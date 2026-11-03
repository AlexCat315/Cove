//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Grid.swift.
use crate::Scope;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    Alignment, AnyView, BoxView, HorizontalAlignment, Layout, LayoutContainer, LayoutSubview,
    PrimitiveView, ProposedViewSize, VerticalAlignment, View, ViewKind,
};
use cove_core::scene::{Direction, LayoutNode, Node, Point, Rect, Size};
use cove_core::{Size as CoreSize, State};

use super::columns::{GridColumn, GridTrackResolver};
use super::named_columns::{
    ColumnVisibilityPlan, ColumnVisibilityTrack, GridColumnError, GridNamedColumns,
};
use super::span::GridSpanKey;

/// A general row-major grid built on the public measurement and placement
/// contract. Row heights come from the tallest measured child in each row.
/// Children may cover several cells with `.gridCellSpan(columns:rows:)`.
#[derive(Clone, Debug)]
pub struct GridLayout {
    columns: Vec<GridColumn>,
    named: Option<Vec<ColumnVisibilityTrack>>,
    pub column_spacing: f32,
    pub row_spacing: f32,
    pub alignment: Alignment,
    pub column_visibility: HashMap<String, bool>,
}
impl GridLayout {
    pub fn try_new(columns: Vec<GridColumn>) -> Result<Self, GridColumnError> {
        let named = GridNamedColumns::tracks(&columns)?;
        Ok(Self {
            columns,
            named,
            ..Default::default()
        })
    }

    fn alignment_components(&self) -> (HorizontalAlignment, VerticalAlignment) {
        self.alignment.components()
    }

    fn horizontal_offset(alignment: HorizontalAlignment, available: f32, rtl: bool) -> f32 {
        match alignment {
            HorizontalAlignment::Leading => {
                if rtl {
                    available
                } else {
                    0.0
                }
            }
            HorizontalAlignment::Center => available / 2.0,
            HorizontalAlignment::Trailing => {
                if rtl {
                    0.0
                } else {
                    available
                }
            }
        }
    }

    fn vertical_offset(alignment: VerticalAlignment, available: f32) -> f32 {
        match alignment {
            VerticalAlignment::Top => 0.0,
            VerticalAlignment::Center => available / 2.0,
            VerticalAlignment::Bottom => available,
        }
    }

    /// Taffy owns track sizing, spanning and row placement. Proxy leaves bridge
    /// the public custom-layout measurement contract without reparenting views.
    fn arrangement(
        &self,
        proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) -> NativeArrangement {
        use cove_core::scene::{Display, GridPlacement, GridStyle, Gutter, MeasureMode};
        if self.columns.is_empty() || subviews.is_empty() {
            return NativeArrangement {
                frames: vec![None; subviews.len()],
                size: Size::default(),
            };
        }
        let gap = GridTrackResolver::finite_nonnegative(self.column_spacing);
        let named = &self.named;
        let visible: Vec<usize> = named
            .as_ref()
            .map(|tracks| {
                ColumnVisibilityPlan::resolve(
                    tracks,
                    &self.column_visibility,
                    proposal.width.unwrap_or(f32::INFINITY),
                    gap,
                    &Default::default(),
                )
                .indices
            })
            .unwrap_or_else(|| (0..self.columns.len()).collect());
        if visible.is_empty() {
            return NativeArrangement {
                frames: vec![None; subviews.len()],
                size: Size::default(),
            };
        }
        let columns: Vec<_> = visible
            .iter()
            .map(|&i| self.columns[i].sizing().clone())
            .collect();
        let direction = subviews
            .first()
            .map_or(Direction::Ltr, |v| v.layout_direction);
        let root = LayoutNode::new();
        root.set_display(Display::Grid);
        root.set_gap(gap, Gutter::Column);
        root.set_gap(
            GridTrackResolver::finite_nonnegative(self.row_spacing),
            Gutter::Row,
        );
        let tracks = native_columns(&columns, proposal.width, gap);
        let column_count = tracks.len();
        root.set_grid_style(GridStyle {
            columns: tracks,
            ..Default::default()
        });
        // Stretch proxies to obtain the cell width, while the public alignment
        // still controls each measured view inside that cell.
        let mut proxies = Vec::with_capacity(subviews.len());
        for (index, subview) in subviews.iter().enumerate() {
            let proxy = LayoutNode::new();
            let mut grid = GridStyle::default();
            let is_visible = if named.is_some() {
                let source = index % self.columns.len();
                if let Some(column) = visible.iter().position(|&value| value == source) {
                    grid.column = [
                        GridPlacement::Line(((column + 1) as i16).into()),
                        GridPlacement::Span(1),
                    ];
                    grid.row = [
                        GridPlacement::Line(((index / self.columns.len() + 1) as i16).into()),
                        GridPlacement::Span(1),
                    ];
                    true
                } else {
                    false
                }
            } else {
                let span = subview.value::<GridSpanKey>();
                grid.column = [
                    GridPlacement::Auto,
                    GridPlacement::Span(
                        span.columns.max(1).min(column_count).min(u16::MAX as usize) as u16,
                    ),
                ];
                grid.row = [
                    GridPlacement::Auto,
                    GridPlacement::Span(span.rows.max(1).min(u16::MAX as usize) as u16),
                ];
                true
            };
            proxy.set_grid_style(grid);
            if !is_visible {
                proxy.set_display(Display::None);
            }
            let view = subview.clone();
            proxy.set_measure_func(Some(Rc::new(move |width, mode, _, _| {
                if mode == MeasureMode::Undefined {
                    return view.size_that_fits(ProposedViewSize::new(None, None));
                }
                cell_size(&view, width)
            })));
            root.add_child(&proxy);
            proxies.push((proxy, is_visible));
        }
        root.calculate_layout(proposal.width, None, direction);
        let frames: Vec<_> = proxies
            .iter()
            .map(|(node, visible)| visible.then(|| node.frame()))
            .collect();
        // CSS tracks may overflow a constrained root. Report the occupied
        // extent to Cove's enclosing ScrollView so required columns stay reachable.
        let left = frames
            .iter()
            .flatten()
            .map(|frame| frame.x)
            .fold(0.0, f32::min);
        let right = frames
            .iter()
            .flatten()
            .map(|frame| frame.max_x())
            .fold(root.frame().width, f32::max);
        NativeArrangement {
            frames,
            size: Size {
                width: right - left,
                height: root.frame().height,
            },
        }
    }
}
impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            named: None,
            column_spacing: 0.0,
            row_spacing: 0.0,
            alignment: Alignment::TopLeading,
            column_visibility: HashMap::new(),
        }
    }
}
impl Layout for GridLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.arrangement(proposal, subviews).size
    }
    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let plan = self.arrangement(ProposedViewSize::new(Some(bounds.width), None), subviews);
        let rtl = subviews
            .first()
            .is_some_and(|view| view.layout_direction == Direction::Rtl);
        let (horizontal, vertical) = self.alignment_components();
        for (view, frame) in subviews.iter().zip(plan.frames) {
            let Some(frame) = frame else {
                view.place(Point::new(0.0, 0.0), ProposedViewSize::ZERO, false);
                continue;
            };
            let size = cell_size(view, frame.width);
            view.place(
                Point::new(
                    bounds.x
                        + frame.x
                        + Self::horizontal_offset(horizontal, frame.width - size.width, rtl),
                    bounds.y
                        + frame.y
                        + Self::vertical_offset(vertical, frame.height - size.height),
                ),
                ProposedViewSize::new(Some(frame.width), Some(size.height)),
                true,
            );
        }
    }
}
struct NativeArrangement {
    frames: Vec<Option<Rect>>,
    size: Size,
}

/// A grid cell can shrink below its track: the probe lays children out at
/// their intrinsic width. Wrapped text then needs more lines than the row was
/// measured against the track width, so the row reserves too little height and
/// the last line is clipped. Re-measure at the width the cell will actually
/// occupy so the reported width and height agree.
fn cell_size(view: &LayoutSubview, track_width: f32) -> Size {
    let fitted = view.size_that_fits(ProposedViewSize::new(Some(track_width), None));
    if fitted.width + 0.01 >= track_width {
        return fitted;
    }
    view.size_that_fits(ProposedViewSize::new(Some(fitted.width), None))
}

fn native_track(column: &GridColumn) -> cove_core::scene::GridTrack {
    use cove_core::scene::{GridBreadth, GridTrack, LayoutStyleValue as Value};
    match column.sizing() {
        GridColumn::Fixed(value) => {
            GridTrack::length(Value::Points(GridTrackResolver::finite_nonnegative(*value)))
        }
        GridColumn::Flexible { minimum, maximum } | GridColumn::Adaptive { minimum, maximum } => {
            let minimum = GridTrackResolver::finite_nonnegative(*minimum);
            let max = if maximum.is_finite() {
                GridBreadth::Length(Value::Points(maximum.max(minimum)))
            } else {
                GridBreadth::Fraction(1.0)
            };
            GridTrack::minmax(GridBreadth::Length(Value::Points(minimum)), max)
        }
        GridColumn::Named { .. } => unreachable!("sizing unwraps named columns"),
    }
}

/// Adaptive columns are a Cove policy: allocate each group with native Grid,
/// then expand its slots. Taffy solves the final tracks and child placement.
/// This also supports multiple adaptive groups (CSS permits one auto-repeat).
fn native_columns(
    columns: &[GridColumn],
    width: Option<f32>,
    gap: f32,
) -> Vec<cove_core::scene::GridTemplateTrack> {
    use cove_core::scene::{Display, GridStyle, Gutter};
    let initial: Vec<_> = columns
        .iter()
        .map(|column| {
            let group = match column.sizing() {
                GridColumn::Adaptive { minimum, .. } => {
                    GridColumn::flexible(*minimum, f32::INFINITY)
                }
                value => value.clone(),
            };
            native_track(&group).into()
        })
        .collect();
    if !columns
        .iter()
        .any(|column| matches!(column.sizing(), GridColumn::Adaptive { .. }))
    {
        return initial;
    }
    let root = LayoutNode::new();
    root.set_display(Display::Grid);
    root.set_gap(gap, Gutter::Column);
    root.set_grid_style(GridStyle {
        columns: initial.clone(),
        ..Default::default()
    });
    let cells: Vec<_> = columns
        .iter()
        .map(|_| {
            let node = LayoutNode::new();
            root.add_child(&node);
            node
        })
        .collect();
    root.calculate_layout(width, None, Direction::Ltr);
    columns
        .iter()
        .zip(cells)
        .flat_map(|(column, cell)| {
            let count = if let GridColumn::Adaptive { minimum, .. } = column.sizing() {
                let minimum = GridTrackResolver::finite_nonnegative(*minimum).max(1.0);
                ((cell.frame().width + gap) / (minimum + gap))
                    .floor()
                    .max(1.0)
                    .min(32767.0) as usize
            } else {
                1
            };
            std::iter::repeat_n(native_track(column).into(), count)
        })
        .collect()
}

/// Places each direct child in one cell. Use `LazyGrid` for large collections.
pub struct Grid {
    pub layout: GridLayout,
    pub content: AnyView,
    /// Overrides supplied by a picker or host. Swift uses `Binding`; the Rust
    /// image has no binding primitive yet, so shared state is explicit.
    pub column_visibility: Option<Rc<State<HashMap<String, bool>>>>,
    pub is_enabled: bool,
    pub local_visibility: State<HashMap<String, bool>>,
}
impl Grid {
    pub fn try_new(
        columns: Vec<GridColumn>,
        content: impl View + 'static,
    ) -> Result<Self, GridColumnError> {
        Ok(Self {
            layout: GridLayout::try_new(columns)?,
            content: AnyView::new(content),
            column_visibility: None,
            is_enabled: true,
            local_visibility: State::new(HashMap::new()),
        })
    }

    pub fn spacing(mut self, column: f32, row: f32) -> Self {
        self.layout.column_spacing = column;
        self.layout.row_spacing = row;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.layout.alignment = alignment;
        self
    }

    pub fn column_visibility(
        mut self,
        visibility: Option<Rc<State<HashMap<String, bool>>>>,
    ) -> Self {
        self.column_visibility = visibility;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
}

/// The grid's own tracks re-expressed through the shared columns contract, so
/// the picker and the layout read one visibility type.
fn shared_tracks(
    tracks: &[ColumnVisibilityTrack],
) -> Vec<crate::primitives::columns::ColumnVisibilityTrack> {
    tracks
        .iter()
        .map(|track| {
            crate::primitives::columns::ColumnVisibilityTrack::new(
                track.id.clone(),
                track.title.clone(),
                match track.visibility {
                    super::columns::ColumnVisibility::Required => {
                        crate::primitives::columns::ColumnVisibility::Required
                    }
                    super::columns::ColumnVisibility::Automatic { priority } => {
                        crate::primitives::columns::ColumnVisibility::Automatic { priority }
                    }
                },
                track.width,
            )
        })
        .collect()
}

impl View for Grid {
    fn body(&self, _cx: &Scope) -> AnyView {
        let layout = self.layout.clone();
        let content = self.content.clone();
        let is_enabled = self.is_enabled;
        let overrides = self.column_visibility.clone();
        let local = self.local_visibility.clone();
        let reader = crate::adaptive::ContainerSizeReader::new(move |size: CoreSize| {
            let values = overrides
                .as_ref()
                .map(|state| state.get())
                .unwrap_or_else(|| local.get());
            let mut algorithm = layout.clone();
            algorithm.column_visibility = values.clone();
            let grid = LayoutContainer::<GridLayout, AnyView>::with_erased_content(
                algorithm,
                content.clone(),
            );
            let scroll = crate::ScrollView::new(crate::ScrollAxis::Horizontal, AnyView::new(grid));
            let spacing = GridTrackResolver::finite_nonnegative(layout.column_spacing);
            // Named columns publish a picker above the grid, so hiding a
            // column is reachable without the host wiring its own menu.
            let picker = GridNamedColumns::tracks(&layout.columns)
                .ok()
                .flatten()
                .map(|tracks| {
                    let shared = shared_tracks(&tracks);
                    let plan = crate::primitives::columns::ColumnVisibilityPlan::resolve(
                        &shared,
                        &values,
                        size.width,
                        spacing,
                        &Default::default(),
                    );
                    let picker = crate::primitives::columns::ColumnVisibilityPicker::new(
                        shared,
                        plan,
                        overrides
                            .as_ref()
                            .map(|state| state.binding())
                            .unwrap_or_else(|| local.binding()),
                        is_enabled,
                    );
                    AnyView::new(picker)
                });
            let mut children: Vec<AnyView> = Vec::new();
            if let Some(picker) = picker {
                children.push(picker);
            }
            children.push(AnyView::new(scroll));
            let body = AnyView::new(BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                AnyView::new(crate::TupleView::new(children)),
            ));
            AnyView::new(GridInteractionHost {
                is_enabled,
                content: body,
            })
        });
        AnyView::new(reader)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        crate::dynamic_fields!(self; local_visibility)
    }
}

/// Disables hit testing and drops capture/focus inside a disabled grid.
pub(crate) struct GridInteractionHost {
    pub is_enabled: bool,
    pub content: AnyView,
}
impl PrimitiveView for GridInteractionHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_is_interaction_enabled(self.is_enabled);
        if self.is_enabled {
            return;
        }
        let Some(chain) = cove_core::scene::input::FocusChainHolder::current() else {
            return;
        };
        if let Some(target) = cove_core::scene::input::PointerCaptureHolder::current()
            .and_then(|capture| capture.target())
            && chain.contains(&target, node)
            && let Some(capture) = cove_core::scene::input::PointerCaptureHolder::current()
        {
            capture.release();
        }
        if let Some(target) = chain.focused()
            && chain.contains(&target, node)
        {
            chain.clear();
        }
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl View for GridInteractionHost {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Named columns publish their visibility plan to a picker. The picker
/// primitive (`Primitives/Columns/Picker.swift`) is outside this port; the
/// plan and minimum width are still available here.
pub fn named_column_plan(
    tracks: &[ColumnVisibilityTrack],
    overrides: &HashMap<String, bool>,
    available_width: f32,
    spacing: f32,
) -> ColumnVisibilityPlan {
    ColumnVisibilityPlan::resolve(
        tracks,
        overrides,
        available_width,
        spacing,
        &Default::default(),
    )
}
