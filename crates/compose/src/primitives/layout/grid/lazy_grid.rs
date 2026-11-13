//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/LazyGrid.swift.
use crate::Scope;
use std::ops::Range;
use std::rc::Rc;

use crate::modifiers::layout::GridExt;
use crate::{AnyView, BoxView, LayoutNode, PrimitiveView, View, ViewKind};
use cove_core::State;
use cove_core::scene::Point;

use super::columns::GridColumn;
use super::grid::Grid;
use super::row_heights::GridRowHeights;

/// Row packing shared by the eager and lazy grids. Uniform rows are
/// arithmetic; spans are packed once per update from small integers, never
/// from views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LazyGridRows {
    pub columns: usize,
    pub count: usize,
    item_count: usize,
    row_starts: Option<Vec<usize>>,
    item_columns: Option<Vec<usize>>,
    spans: Option<Vec<usize>>,
}
impl LazyGridRows {
    pub fn new(item_count: usize, columns: usize, raw_spans: Option<&[usize]>) -> Self {
        let columns = columns.max(1);
        let Some(raw_spans) = raw_spans else {
            return Self {
                columns,
                count: item_count / columns + usize::from(!item_count.is_multiple_of(columns)),
                item_count,
                row_starts: None,
                item_columns: None,
                spans: None,
            };
        };
        let spans: Vec<usize> = raw_spans
            .iter()
            .map(|span| columns.min((*span).max(1)))
            .collect();
        let mut starts = Vec::new();
        let mut positions = Vec::with_capacity(spans.len());
        let mut column = columns;
        for (index, span) in spans.iter().enumerate() {
            if column + span > columns {
                starts.push(index);
                column = 0;
            }
            positions.push(column);
            column += span;
        }
        let count = starts.len();
        starts.push(spans.len());
        Self {
            columns,
            count,
            item_count,
            row_starts: Some(starts),
            item_columns: Some(positions),
            spans: Some(spans),
        }
    }

    pub fn items(&self, row: usize) -> Range<usize> {
        match &self.row_starts {
            Some(starts) => starts[row]..starts[row + 1],
            None => {
                let start = row * self.columns;
                start..self.item_count.min(start + self.columns)
            }
        }
    }

    pub fn column_of_item(&self, index: usize) -> usize {
        self.item_columns
            .as_ref()
            .map_or(index % self.columns, |columns| columns[index])
    }

    pub fn span_of_item(&self, index: usize) -> usize {
        self.spans.as_ref().map_or(1, |spans| spans[index])
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LazyGridError {
    #[error("a grid requires at least one column")]
    NoColumns,
    #[error("the grid requires a height for each of its {rows} rows")]
    RowHeightCount { rows: usize },
}

/// Layout inputs used to pack and validate all rows together.
pub struct LazyGridOptions<T> {
    pub columns: usize,
    pub row_heights: GridRowHeights,
    pub column_spacing: f32,
    pub row_spacing: f32,
    pub overscan: usize,
    pub column_width: Option<f32>,
    pub column_span: Option<Rc<dyn Fn(&T) -> usize>>,
}

impl<T> LazyGridOptions<T> {
    pub fn new(columns: usize, row_heights: GridRowHeights) -> Self {
        Self {
            columns,
            row_heights,
            column_spacing: 0.0,
            row_spacing: 0.0,
            overscan: 0,
            column_width: None,
            column_span: None,
        }
    }
}

/// A vertically scrolling, equal-column grid that constructs only visible
/// rows plus overscan. Row heights are explicit so scrolling never measures
/// the full collection.
pub struct LazyGrid<T> {
    data: Vec<T>,
    id: Rc<dyn Fn(&T) -> String>,
    columns: usize,
    row_heights: GridRowHeights,
    column_spacing: f32,
    row_spacing: f32,
    overscan: usize,
    scroll_offset: Option<Rc<State<Point>>>,
    column_width: Option<f32>,
    rows: Rc<LazyGridRows>,
    content: Rc<dyn Fn(&T) -> AnyView>,
    visible_rows: State<Range<usize>>,
    visible_columns: State<Range<usize>>,
}
impl<T: Clone + 'static> LazyGrid<T> {
    pub fn try_new(
        data: Vec<T>,
        id: impl Fn(&T) -> String + 'static,
        options: LazyGridOptions<T>,
        content: impl Fn(&T) -> AnyView + 'static,
    ) -> Result<Self, LazyGridError> {
        let LazyGridOptions {
            columns,
            row_heights,
            column_spacing,
            row_spacing,
            overscan,
            column_width,
            column_span,
        } = options;
        if columns == 0 {
            return Err(LazyGridError::NoColumns);
        }
        let spans: Option<Vec<usize>> = column_span
            .as_ref()
            .map(|span| data.iter().map(|item| (span)(item)).collect());
        let rows = LazyGridRows::new(data.len(), columns, spans.as_deref());
        if !row_heights.matches_count(rows.count) {
            return Err(LazyGridError::RowHeightCount { rows: rows.count });
        }
        Ok(Self {
            data,
            id: Rc::new(id),
            columns,
            row_heights,
            column_spacing: super::columns::GridTrackResolver::finite_nonnegative(column_spacing),
            row_spacing: super::columns::GridTrackResolver::finite_nonnegative(row_spacing),
            overscan,
            scroll_offset: None,
            column_width: column_width.filter(|width| width.is_finite() && *width > 0.0),
            rows: Rc::new(rows),
            content: Rc::new(content),
            visible_rows: State::new(0..1),
            visible_columns: State::new(0..1),
        })
    }

    pub fn scroll_offset(mut self, offset: Rc<State<Point>>) -> Self {
        self.scroll_offset = Some(offset);
        self
    }

    pub fn visible_columns(
        count: usize,
        width: f32,
        spacing: f32,
        offset: f32,
        viewport: f32,
        overscan: usize,
    ) -> Range<usize> {
        if count == 0 || width <= 0.0 {
            return 0..0;
        }
        let stride = width + spacing;
        let first = ((offset.max(0.0) / stride).floor() as usize).saturating_sub(overscan);
        let last = (((offset.max(0.0) + viewport.max(0.0)) / stride).ceil() as usize + overscan)
            .min(count);
        first..first.max(last)
    }
}
impl<T: Clone + 'static> View for LazyGrid<T> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let grid = self.scaled_grid(1.0);
        AnyView::new(crate::theme::TextScalerReader::new(move |scaler| {
            grid.scaled_grid(scaler.factor())
        }))
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
impl<T: Clone + 'static> LazyGrid<T> {
    fn scaled_grid(&self, factor: f32) -> LazyGridView<T> {
        LazyGridView {
            data: self.data.clone(),
            id: self.id.clone(),
            columns: self.columns,
            row_heights: self.row_heights.scaled(factor),
            column_spacing: self.column_spacing,
            row_spacing: self.row_spacing * factor,
            overscan: self.overscan,
            scroll_offset: self.scroll_offset.clone(),
            column_width: self.column_width,
            rows: self.rows.clone(),
            content: self.content.clone(),
            visible_rows: self.visible_rows.clone(),
            visible_columns: self.visible_columns.clone(),
        }
    }
}

/// Holds the scaled row metrics and window state that the public `LazyGrid`
/// rebuilds when the text scale changes.
pub struct LazyGridView<T> {
    data: Vec<T>,
    id: Rc<dyn Fn(&T) -> String>,
    columns: usize,
    row_heights: GridRowHeights,
    column_spacing: f32,
    row_spacing: f32,
    overscan: usize,
    scroll_offset: Option<Rc<State<Point>>>,
    column_width: Option<f32>,
    rows: Rc<LazyGridRows>,
    content: Rc<dyn Fn(&T) -> AnyView>,
    visible_rows: State<Range<usize>>,
    visible_columns: State<Range<usize>>,
}
impl<T: Clone + 'static> LazyGridView<T> {
    /// Re-applies a text-scale factor to the row metrics. The view may have
    /// been built at factor 1, so the base metrics are still unscaled.
    fn scaled_grid(&self, factor: f32) -> LazyGridView<T> {
        LazyGridView {
            data: self.data.clone(),
            id: self.id.clone(),
            columns: self.columns,
            row_heights: self.row_heights.scaled(factor),
            column_spacing: self.column_spacing,
            row_spacing: self.row_spacing * factor,
            overscan: self.overscan,
            scroll_offset: self.scroll_offset.clone(),
            column_width: self.column_width,
            rows: self.rows.clone(),
            content: self.content.clone(),
            visible_rows: self.visible_rows.clone(),
            visible_columns: self.visible_columns.clone(),
        }
    }
}
impl<T: Clone + 'static> View for LazyGridView<T> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows = self.rows.clone();
        let row_count = rows.count;
        let visible = self.visible_rows.get();
        let lower = row_count.min(visible.start);
        let upper = lower.max(row_count.min(visible.end));
        let total = self.row_heights.total_height(row_count, self.row_spacing);
        let first_offset = if lower < row_count {
            self.row_heights.offset(lower, self.row_spacing)
        } else {
            total
        };
        let last_offset = if upper < row_count {
            self.row_heights.offset(upper, self.row_spacing)
        } else {
            total
        };
        let content_width = self.column_width.map(|width| {
            self.columns as f32 * width + self.column_spacing * (self.columns - 1) as f32
        });

        let mut children: Vec<AnyView> = Vec::new();
        children.push(AnyView::new(LazyGridSpacer {
            height: first_offset,
        }));
        for row in lower..upper {
            let height = self.row_heights.height(row);
            let gap = if row + 1 < rows.count {
                self.row_spacing
            } else {
                0.0
            };
            children.push(AnyView::new(LazyGridRow {
                cells: self.cells(row, &rows, height),
                height: height + gap,
            }));
        }
        children.push(AnyView::new(LazyGridSpacer {
            height: (total - last_offset).max(0.0),
        }));

        let column = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            0.0,
            AnyView::new(crate::TupleView::new(children)),
        ));
        let content = LazyGridContent {
            width: content_width.unwrap_or(0.0),
            height: total,
            has_fixed_width: content_width.is_some(),
            content: column,
        };
        let axis = if self.column_width.is_none() {
            crate::ScrollAxis::Vertical
        } else {
            crate::ScrollAxis::Both
        };
        let mut scroll = crate::ScrollView::new(axis, AnyView::new(content))
            .scrollbar_gutter(crate::ScrollbarGutter::Stable);
        if let Some(offset) = &self.scroll_offset {
            // `State` is `Arc`-backed and thread-safe; the surrounding `Rc`
            // handle is UI-thread-only, so clone the `State` out of it rather
            // than capturing the `Rc` in the binding.
            scroll = scroll.scroll_offset((**offset).binding());
        }
        let visible_rows = self.visible_rows.clone();
        let visible_columns = self.visible_columns.clone();
        let row_heights = self.row_heights.clone();
        let row_spacing = self.row_spacing;
        let overscan = self.overscan;
        let columns = self.columns;
        let column_width = self.column_width;
        let column_spacing = self.column_spacing;
        scroll = scroll.on_geometry_change(move |geometry: crate::ScrollGeometry| {
            let next = row_heights.visible_rows(
                row_count,
                geometry.offset.y,
                geometry.viewport_size.height,
                row_spacing,
                overscan,
            );
            if visible_rows.get() != next {
                visible_rows.set(next);
            }
            if let Some(column_width) = column_width {
                let next_columns = LazyGrid::<T>::visible_columns(
                    columns,
                    column_width,
                    column_spacing,
                    geometry.offset.x,
                    geometry.viewport_size.width,
                    overscan,
                );
                if visible_columns.get() != next_columns {
                    visible_columns.set(next_columns);
                }
            }
        });
        AnyView::new(scroll)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// `body` reads `visible_rows` from `State`, so the
    /// scope must declare those dependencies. Without this the writes never
    /// subscribe and the view keeps its first layout forever.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state(
            "visible_rows",
            &self.visible_rows,
        )]
    }
}
impl<T: Clone + 'static> LazyGridView<T> {
    fn cells(&self, row: usize, rows: &LazyGridRows, height: f32) -> AnyView {
        let items = rows.items(row);
        if let Some(column_width) = self.column_width {
            let window = self.visible_columns.get();
            let window = window.start.min(self.columns)..window.end.min(self.columns);
            let mut children = Vec::new();
            for index in items {
                let start = rows.column_of_item(index);
                let span = rows.span_of_item(index);
                if start < window.end && start + span > window.start {
                    let element = &self.data[index];
                    let x = start as f32 * (column_width + self.column_spacing);
                    let width =
                        span as f32 * column_width + (span - 1) as f32 * self.column_spacing;
                    children.push(AnyView::new(LazyGridCell {
                        x,
                        width,
                        height,
                        key: (self.id)(element),
                        content: (self.content)(element),
                    }));
                }
            }
            return AnyView::new(LazyGridCellRow { children });
        }
        let columns: Vec<GridColumn> = (0..self.columns)
            .map(|_| GridColumn::flexible(0.0, f32::INFINITY))
            .collect();
        let mut cells = Vec::new();
        for index in items {
            let element = &self.data[index];
            let content = AnyView::new(LazyGridFixedHeight {
                height,
                content: (self.content)(element),
            });
            if rows.span_of_item(index) > 1 {
                cells.push(AnyView::new(
                    content.grid_span(1, rows.span_of_item(index) as u16),
                ));
            } else {
                cells.push(content);
            }
        }
        AnyView::new(
            Grid::try_new(columns, AnyView::new(crate::TupleView::new(cells)))
                .expect("valid grid columns")
                .spacing(self.column_spacing, 0.0),
        )
    }
}

/// Applies the row height to one lazy grid cell.
struct LazyGridFixedHeight {
    height: f32,
    content: AnyView,
}
impl PrimitiveView for LazyGridFixedHeight {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &cove_core::scene::Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(self.height));
        layout.set_width_percent(100.0);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.height));
        layout.set_width_percent(100.0);
    }
    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl View for LazyGridFixedHeight {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// A fixed-height spacer used for the unconstructed head and tail.
struct LazyGridSpacer {
    height: f32,
}
impl PrimitiveView for LazyGridSpacer {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &cove_core::scene::Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(self.height));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.height));
    }
    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![]
    }
}
impl View for LazyGridSpacer {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct LazyGridRow {
    cells: AnyView,
    height: f32,
}
impl PrimitiveView for LazyGridRow {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &cove_core::scene::Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(self.height));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.height));
    }
    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.cells.clone()]
    }
}
impl View for LazyGridRow {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct LazyGridContent {
    content: AnyView,
    width: f32,
    height: f32,
    has_fixed_width: bool,
}
impl PrimitiveView for LazyGridContent {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &cove_core::scene::Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(self.height));
        if self.has_fixed_width {
            layout.set_width(Some(self.width));
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.height));
        if self.has_fixed_width {
            layout.set_width(Some(self.width));
        }
    }
    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl View for LazyGridContent {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct LazyGridCellRow {
    children: Vec<AnyView>,
}
impl View for LazyGridCellRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            0.0,
            AnyView::new(crate::TupleView::new(self.children.clone())),
        ))
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct LazyGridCell {
    x: f32,
    width: f32,
    height: f32,
    key: String,
    content: AnyView,
}
impl PrimitiveView for LazyGridCell {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &cove_core::scene::Node) {
        // The item identity survives windowing: a cell that scrolls out and
        // back keeps the same name for the inspector and for tests.
        node.set_attachment(
            cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME,
            self.key.clone(),
        );
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_position_type(cove_core::scene::PositionType::Absolute);
        layout.set_position(self.x, cove_core::scene::Edge::Left);
        layout.set_position(0.0, cove_core::scene::Edge::Top);
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(cove_core::scene::PositionType::Absolute);
        layout.set_position(self.x, cove_core::scene::Edge::Left);
        layout.set_position(0.0, cove_core::scene::Edge::Top);
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
    }
    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl View for LazyGridCell {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
