//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Grid.swift.
use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    Alignment, AnyView, BoxView, HorizontalAlignment, Layout, LayoutContainer, LayoutSubview,
    PrimitiveView, ProposedViewSize, VerticalAlignment, View, ViewKind,
};
use cove_core::scene::{Direction, LayoutNode, Node, Point, Rect, Size};
use cove_core::{Size as CoreSize, State};

use super::columns::{GridColumn, GridTrackResolver, GridTracks};
use super::named_columns::{ColumnVisibilityPlan, ColumnVisibilityTrack, GridNamedColumns};
use super::span::{GridSpan, GridSpanKey, GridSpanPlacement};

/// A general row-major grid built on the public measurement and placement
/// contract. Row heights come from the tallest measured child in each row.
/// Children may cover several cells with `.gridCellSpan(columns:rows:)`.
#[derive(Clone, Debug)]
pub struct GridLayout {
    pub columns: Vec<GridColumn>,
    pub column_spacing: f32,
    pub row_spacing: f32,
    pub alignment: Alignment,
    pub column_visibility: HashMap<String, bool>,
}
impl GridLayout {
    pub fn new(
        columns: Vec<GridColumn>,
        column_spacing: f32,
        row_spacing: f32,
        alignment: Alignment,
        column_visibility: HashMap<String, bool>,
    ) -> Self {
        let _ = GridNamedColumns::tracks(&columns);
        Self {
            columns,
            column_spacing,
            row_spacing,
            alignment,
            column_visibility,
        }
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

    fn arrangement(
        &self,
        proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) -> GridArrangement {
        if self.columns.is_empty() || subviews.is_empty() {
            return GridArrangement::empty();
        }
        let gap = GridTrackResolver::finite_nonnegative(self.column_spacing);
        let named = GridNamedColumns::tracks(&self.columns);
        let plan = named.as_ref().map(|tracks| {
            ColumnVisibilityPlan::resolve(
                tracks,
                &self.column_visibility,
                proposal.width.unwrap_or(f32::INFINITY),
                gap,
                &Default::default(),
            )
        });
        let mapping: Option<HashMap<usize, usize>> = plan.as_ref().map(|plan| {
            plan.indices
                .iter()
                .enumerate()
                .map(|(offset, &source)| (source, offset))
                .collect()
        });
        let presented_column = |source: usize| -> Option<usize> {
            mapping
                .as_ref()
                .map_or(Some(source), |mapping| mapping.get(&source).copied())
        };
        let visible_columns: Vec<GridColumn> = plan
            .as_ref()
            .map(|plan| {
                plan.indices
                    .iter()
                    .map(|&index| self.columns[index].clone())
                    .collect()
            })
            .unwrap_or_else(|| self.columns.clone());
        let mut intrinsic_widths: Vec<f32> = Vec::new();
        if proposal.width.is_none() {
            intrinsic_widths = vec![0.0_f32; visible_columns.len()];
            for index in 0..subviews.len() {
                let column = index % self.columns.len();
                if let Some(presented) = presented_column(column) {
                    intrinsic_widths[presented] = intrinsic_widths[presented].max(
                        subviews[index]
                            .size_that_fits(ProposedViewSize::UNSPECIFIED)
                            .width,
                    );
                }
            }
        }
        let widths =
            GridTrackResolver::widths(&visible_columns, proposal.width, gap, &intrinsic_widths);
        let source_column_count = if named.is_none() {
            widths.count
        } else {
            self.columns.len()
        };
        let row_count = subviews.len() / source_column_count
            + usize::from(!subviews.len().is_multiple_of(source_column_count));
        let mut heights = vec![0.0f32; row_count];
        let mut active_rows = vec![false; row_count];
        let mut sizes = Vec::with_capacity(subviews.len());
        for index in 0..subviews.len() {
            let source = index % source_column_count;
            let presented = presented_column(source);
            let size = presented
                .map(|presented| {
                    subviews[index]
                        .size_that_fits(ProposedViewSize::new(Some(widths.get(presented)), None))
                })
                .unwrap_or_default();
            sizes.push(size);
            let row = index / source_column_count;
            if presented.is_some() {
                active_rows[row] = true;
            }
            heights[row] = heights[row].max(size.height);
        }
        let active_count = active_rows.iter().filter(|active| **active).count();
        let size = Size {
            width: widths.total_width,
            height: heights.iter().sum::<f32>()
                + GridTrackResolver::finite_nonnegative(self.row_spacing)
                    * active_count.saturating_sub(1) as f32,
        };
        GridArrangement {
            widths,
            heights,
            item_sizes: sizes,
            source_column_count,
            mapping,
            active_rows,
            size,
        }
    }

    // MARK: - Spanning cells

    /// Spans apply to anonymous columns only; `None` keeps the one-cell path.
    fn spans(&self, subviews: &[LayoutSubview]) -> Option<Vec<GridSpan>> {
        if self.columns.is_empty() || GridNamedColumns::tracks(&self.columns).is_some() {
            return None;
        }
        let spans: Vec<GridSpan> = subviews
            .iter()
            .map(|subview| subview.value::<GridSpanKey>())
            .collect();
        if spans.iter().any(|span| *span != GridSpan::SINGLE) {
            Some(spans)
        } else {
            None
        }
    }

    fn span_arrangement(
        &self,
        proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
        spans: &[GridSpan],
    ) -> SpanArrangement {
        let gap = GridTrackResolver::finite_nonnegative(self.column_spacing);
        let row_gap = GridTrackResolver::finite_nonnegative(self.row_spacing);
        let mut intrinsic = Vec::new();
        if proposal.width.is_none() {
            let draft = GridSpanPlacement::new(spans, self.columns.len());
            intrinsic = vec![0.0_f32; self.columns.len()];
            for (index, cell) in draft.cells.iter().enumerate() {
                if cell.span.columns == 1 {
                    intrinsic[cell.column] = intrinsic[cell.column].max(
                        subviews[index]
                            .size_that_fits(ProposedViewSize::UNSPECIFIED)
                            .width,
                    );
                }
            }
        }
        let widths = GridTrackResolver::widths(&self.columns, proposal.width, gap, &intrinsic);
        let placement = GridSpanPlacement::new(spans, widths.count);
        let mut sizes = Vec::with_capacity(subviews.len());
        for (index, cell) in placement.cells.iter().enumerate() {
            let width = (cell.column..(cell.column + cell.span.columns))
                .map(|column| widths.get(column))
                .sum::<f32>()
                + gap * (cell.span.columns - 1) as f32;
            sizes.push(subviews[index].size_that_fits(ProposedViewSize::new(Some(width), None)));
        }
        let heights = placement.row_heights(
            &sizes.iter().map(|size| size.height).collect::<Vec<_>>(),
            row_gap,
        );
        let height = heights.iter().sum::<f32>() + row_gap * heights.len().saturating_sub(1) as f32;
        let total_width = widths.total_width;
        SpanArrangement {
            widths,
            placement,
            heights,
            item_sizes: sizes,
            size: Size {
                width: total_width,
                height,
            },
        }
    }

    fn place_spanning(&self, bounds: Rect, subviews: &[LayoutSubview], spans: &[GridSpan]) {
        let plan = self.span_arrangement(
            ProposedViewSize::new(Some(bounds.width), None),
            subviews,
            spans,
        );
        let gap = GridTrackResolver::finite_nonnegative(self.column_spacing);
        let row_gap = GridTrackResolver::finite_nonnegative(self.row_spacing);
        let rtl = subviews
            .first()
            .map(|subview| subview.layout_direction == Direction::Rtl)
            .unwrap_or(false);
        let (horizontal, vertical) = self.alignment_components();
        let mut row_tops = Vec::with_capacity(plan.heights.len());
        let mut y = bounds.y;
        for height in &plan.heights {
            row_tops.push(y);
            y += height + row_gap;
        }
        for (index, cell) in plan.placement.cells.iter().enumerate() {
            let cell_width = plan.width_of(cell, gap);
            let cell_height = (cell.row..(cell.row + cell.span.rows))
                .map(|row| plan.heights[row])
                .sum::<f32>()
                + row_gap * (cell.span.rows - 1) as f32;
            let offset = plan.x_of_column(cell.column, gap);
            let item = plan.item_sizes[index];
            let cell_x = if rtl {
                bounds.max_x() - offset - cell_width
            } else {
                bounds.x + offset
            };
            subviews[index].place(
                Point::new(
                    cell_x + Self::horizontal_offset(horizontal, cell_width - item.width, rtl),
                    row_tops[cell.row] + Self::vertical_offset(vertical, cell_height - item.height),
                ),
                ProposedViewSize::new(Some(cell_width), Some(item.height)),
                true,
            );
        }
    }
}
impl Default for GridLayout {
    fn default() -> Self {
        Self::new(vec![], 0.0, 0.0, Alignment::TopLeading, HashMap::new())
    }
}
impl Layout for GridLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        if let Some(spans) = self.spans(subviews) {
            return self.span_arrangement(proposal, subviews, &spans).size;
        }
        self.arrangement(proposal, subviews).size
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        if let Some(spans) = self.spans(subviews) {
            self.place_spanning(bounds, subviews, &spans);
            return;
        }
        let arrangement =
            self.arrangement(ProposedViewSize::new(Some(bounds.width), None), subviews);
        if arrangement.widths.is_empty() {
            for child in subviews {
                child.place(Point::new(0.0, 0.0), ProposedViewSize::ZERO, false);
            }
            return;
        }
        let gap = GridTrackResolver::finite_nonnegative(self.column_spacing);
        let row_gap = GridTrackResolver::finite_nonnegative(self.row_spacing);
        let rtl = subviews
            .first()
            .map(|subview| subview.layout_direction == Direction::Rtl)
            .unwrap_or(false);
        let (horizontal, vertical) = self.alignment_components();
        let mut y = bounds.y;
        let mut row = 0usize;
        let mut x = 0.0f32;
        for index in 0..subviews.len() {
            let column = index % arrangement.source_column_count;
            if column == 0 {
                x = 0.0;
            }
            if let Some(presented) = arrangement.presented_column(column) {
                let cell_width = arrangement.widths.get(presented);
                let item_size = arrangement.item_sizes[index];
                let horizontal_offset =
                    Self::horizontal_offset(horizontal, cell_width - item_size.width, rtl);
                let vertical_offset =
                    Self::vertical_offset(vertical, arrangement.heights[row] - item_size.height);
                let cell_x = if rtl {
                    bounds.max_x() - x - cell_width
                } else {
                    bounds.x + x
                };
                subviews[index].place(
                    Point::new(cell_x + horizontal_offset, y + vertical_offset),
                    ProposedViewSize::new(Some(cell_width), Some(item_size.height)),
                    true,
                );
                x += cell_width + gap;
            } else {
                subviews[index].place(Point::new(0.0, 0.0), ProposedViewSize::ZERO, false);
            }
            if column + 1 == arrangement.source_column_count || index + 1 == subviews.len() {
                if arrangement.active_rows.get(row).copied().unwrap_or(false) {
                    y += arrangement.heights[row] + row_gap;
                }
                row += 1;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct GridArrangement {
    widths: GridTracks,
    heights: Vec<f32>,
    item_sizes: Vec<Size>,
    source_column_count: usize,
    mapping: Option<HashMap<usize, usize>>,
    active_rows: Vec<bool>,
    size: Size,
}
impl GridArrangement {
    fn presented_column(&self, source: usize) -> Option<usize> {
        self.mapping
            .as_ref()
            .map_or(Some(source), |mapping| mapping.get(&source).copied())
    }
    fn empty() -> Self {
        Self {
            widths: GridTracks::new(vec![], 0.0),
            heights: vec![],
            item_sizes: vec![],
            source_column_count: 1,
            mapping: None,
            active_rows: vec![],
            size: Size::default(),
        }
    }
}

struct SpanArrangement {
    widths: GridTracks,
    placement: GridSpanPlacement,
    heights: Vec<f32>,
    item_sizes: Vec<Size>,
    size: Size,
}
impl SpanArrangement {
    fn x_of_column(&self, column: usize, gap: f32) -> f32 {
        (0..column).map(|index| self.widths.get(index) + gap).sum()
    }
    fn width_of(&self, cell: &super::span::GridSpanCell, gap: f32) -> f32 {
        (cell.column..(cell.column + cell.span.columns))
            .map(|index| self.widths.get(index))
            .sum::<f32>()
            + gap * (cell.span.columns - 1) as f32
    }
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
    pub fn new(
        columns: Vec<GridColumn>,
        column_spacing: f32,
        row_spacing: f32,
        alignment: Alignment,
        column_visibility: Option<Rc<State<HashMap<String, bool>>>>,
        is_enabled: bool,
        content: AnyView,
    ) -> Self {
        Self {
            layout: GridLayout::new(
                columns,
                column_spacing,
                row_spacing,
                alignment,
                HashMap::new(),
            ),
            content,
            column_visibility,
            is_enabled,
            local_visibility: State::new(HashMap::new()),
        }
    }
}
impl View for Grid {
    fn body(&self) -> AnyView {
        let columns = self.layout.columns.clone();
        let column_spacing = self.layout.column_spacing;
        let row_spacing = self.layout.row_spacing;
        let alignment = self.layout.alignment;
        let content = self.content.clone();
        let is_enabled = self.is_enabled;
        let overrides = self.column_visibility.clone();
        let local = self.local_visibility.clone();
        let reader = crate::adaptive::ContainerSizeReader::new(move |_size: CoreSize| {
            let values = overrides
                .as_ref()
                .map(|state| state.get())
                .unwrap_or_else(|| local.get());
            let mut algorithm = GridLayout::new(
                columns.clone(),
                column_spacing,
                row_spacing,
                alignment,
                values,
            );
            algorithm.column_visibility = algorithm.column_visibility.clone();
            let grid = LayoutContainer::<GridLayout, AnyView>::with_erased_content(
                algorithm,
                content.clone(),
            );
            let scroll = crate::ScrollView::new(crate::ScrollAxis::Horizontal, AnyView::new(grid));
            let body = AnyView::new(BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                AnyView::new(scroll),
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
}

/// Disables hit testing and drops capture/focus inside a disabled grid.
pub struct GridInteractionHost {
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
