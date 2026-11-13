//! Authored CSS Grid: a container declares tracks, gaps and named areas; each
//! child places itself on explicit lines.
//!
//! This is the single grid semantics of the authored layer. Geometry is Taffy's
//! grid algorithm, reached through the same [`GridStyle`] the core layout engine
//! uses; there is no second track solver here. Table-shaped containers that need
//! column policies (fixed/flexible/adaptive/named-plus-visibility) or row
//! virtualization stay in [`crate::primitives::layout::grid::GridLayout`], which
//! compiles its policy down to `GridStyle` and lets Taffy place the cells.
//!
//! ```
//! use cove_composition::{AnyView, EmptyView, TupleView};
//! use cove_core::scene::{Align, FlexDirection, GridAreaTemplate, GridTrack, Justify, Wrap};
//! use cove_ui_compose::prelude::*;
//!
//! let areas = GridAreaTemplate::new(&["header header", "main side"]).unwrap();
//! let shell = GridSpec::areas(areas)
//!     .columns([GridTrack::fr(1.0).into(), GridTrack::points(240.0).into()])
//!     .rows([GridTrack::AUTO.into(), GridTrack::fr(1.0).into()])
//!     .gap(12.0);
//! let main = shell.cell("main").expect("main is declared");
//!
//! let body = BoxView::new(
//!     FlexDirection::Column,
//!     Align::Stretch,
//!     Justify::FlexStart,
//!     Wrap::NoWrap,
//!     0.0,
//!     AnyView::new(TupleView::new(vec![
//!         AnyView::new(AnyView::new(EmptyView).grid_span(2, 1)),
//!         AnyView::new(AnyView::new(EmptyView).grid_cell(main)),
//!     ])),
//! );
//! let _ = body.grid(shell);
//! ```

use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::scene::{
    Align, Display, GridAreaTemplate, GridAutoFlow, GridPlacement, GridTemplateTrack, GridTrack,
    Gutter, LayoutNode, Node,
};

/// A child's placement inside its parent's grid.
#[derive(Clone, Debug, PartialEq)]
pub struct GridCell {
    /// Start line and end line/span on the column axis.
    pub column: [GridPlacement<String>; 2],
    /// Start line and end line/span on the row axis.
    pub row: [GridPlacement<String>; 2],
}

impl GridCell {
    /// One cell at the given 1-based lines.
    pub fn at(column: i16, row: i16) -> Self {
        Self::lines((column, column + 1), (row, row + 1))
    }

    /// An auto-placed item spanning `columns` tracks and `rows` tracks.
    pub fn span(columns: u16, rows: u16) -> Self {
        Self {
            column: [GridPlacement::Auto, GridPlacement::Span(columns.max(1))],
            row: [GridPlacement::Auto, GridPlacement::Span(rows.max(1))],
        }
    }

    /// An item starting at the given 1-based lines and spanning `columns`
    /// tracks and `rows` tracks.
    pub fn spanning(column: i16, row: i16, columns: u16, rows: u16) -> Self {
        Self::lines(
            (column, column + columns.max(1) as i16),
            (row, row + rows.max(1) as i16),
        )
    }

    /// Explicit `(start, end)` line pairs, both 1-based.
    pub fn lines(column: (i16, i16), row: (i16, i16)) -> Self {
        Self {
            column: [
                GridPlacement::Line(column.0.into()),
                GridPlacement::Line(column.1.into()),
            ],
            row: [
                GridPlacement::Line(row.0.into()),
                GridPlacement::Line(row.1.into()),
            ],
        }
    }

    /// Placement by named lines, as authored in [`GridSpec::column_names`].
    ///
    /// Both names resolve to the first line carrying them, which is the CSS
    /// meaning of `grid-column-start: left` / `grid-column-end: split`.
    pub fn named(column: (&str, &str), row: (&str, &str)) -> Self {
        Self {
            column: [
                GridPlacement::NamedLine(column.0.to_string(), 1),
                GridPlacement::NamedLine(column.1.to_string(), 1),
            ],
            row: [
                GridPlacement::NamedLine(row.0.to_string(), 1),
                GridPlacement::NamedLine(row.1.to_string(), 1),
            ],
        }
    }

    /// Placement of the named area in `template`; `None` when it has no such
    /// area. Lines come from the template, so the placement stays explicit and
    /// independent of how Taffy resolves area names internally.
    pub fn area(template: &GridAreaTemplate, name: &str) -> Option<Self> {
        let span = template.area(name)?;
        Some(Self {
            column: [
                GridPlacement::Line(span.column_start.into()),
                GridPlacement::Line(span.column_end.into()),
            ],
            row: [
                GridPlacement::Line(span.row_start.into()),
                GridPlacement::Line(span.row_end.into()),
            ],
        })
    }
}

/// The container side of authored CSS Grid. Every field maps one-to-one onto a
/// CSS `grid-*` property, so the authored layer never invents a second
/// vocabulary for the same concept.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GridSpec {
    pub columns: Vec<GridTemplateTrack>,
    pub rows: Vec<GridTemplateTrack>,
    pub auto_columns: Vec<GridTrack>,
    pub auto_rows: Vec<GridTrack>,
    pub flow: GridAutoFlow,
    pub areas: Option<GridAreaTemplate>,
    /// Default horizontal alignment of every item in its cell.
    pub justify_items: Option<Align>,
    /// Default vertical alignment of every item in its cell.
    pub align_items: Option<Align>,
    pub column_gap: Option<f32>,
    pub row_gap: Option<f32>,
    /// Names for each column line (`grid-template-columns` line names); one set
    /// per line, so an N-track template has N + 1 sets.
    pub column_names: Vec<Vec<String>>,
    /// Names for each row line.
    pub row_names: Vec<Vec<String>>,
}

impl GridSpec {
    /// A container with explicit column tracks.
    pub fn new(columns: impl IntoIterator<Item = GridTemplateTrack>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            ..Self::default()
        }
    }

    /// A container sized by the area template alone; add tracks to control the
    /// track sizes.
    pub fn areas(template: GridAreaTemplate) -> Self {
        Self {
            areas: Some(template),
            ..Self::default()
        }
    }

    pub fn columns(mut self, tracks: impl IntoIterator<Item = GridTemplateTrack>) -> Self {
        self.columns = tracks.into_iter().collect();
        self
    }

    pub fn rows(mut self, tracks: impl IntoIterator<Item = GridTemplateTrack>) -> Self {
        self.rows = tracks.into_iter().collect();
        self
    }

    pub fn auto_rows(mut self, tracks: impl IntoIterator<Item = GridTrack>) -> Self {
        self.auto_rows = tracks.into_iter().collect();
        self
    }

    pub fn auto_columns(mut self, tracks: impl IntoIterator<Item = GridTrack>) -> Self {
        self.auto_columns = tracks.into_iter().collect();
        self
    }

    pub fn flow(mut self, flow: GridAutoFlow) -> Self {
        self.flow = flow;
        self
    }

    pub fn gap(mut self, value: f32) -> Self {
        self.column_gap = Some(value);
        self.row_gap = Some(value);
        self
    }

    pub fn column_gap(mut self, value: f32) -> Self {
        self.column_gap = Some(value);
        self
    }

    pub fn row_gap(mut self, value: f32) -> Self {
        self.row_gap = Some(value);
        self
    }

    pub fn column_names(mut self, names: impl IntoIterator<Item = Vec<String>>) -> Self {
        self.column_names = names.into_iter().collect();
        self
    }

    pub fn row_names(mut self, names: impl IntoIterator<Item = Vec<String>>) -> Self {
        self.row_names = names.into_iter().collect();
        self
    }

    pub fn justify_items(mut self, align: Align) -> Self {
        self.justify_items = Some(align);
        self
    }

    pub fn align_items(mut self, align: Align) -> Self {
        self.align_items = Some(align);
        self
    }

    /// Placement of a name declared in [`Self::areas`].
    pub fn cell(&self, name: &str) -> Option<GridCell> {
        GridCell::area(self.areas.as_ref()?, name)
    }
}

pub struct GridModifier {
    pub spec: GridSpec,
}

impl GridModifier {
    pub fn new(spec: GridSpec) -> Self {
        Self { spec }
    }

    fn apply(&self, layout: &LayoutNode) {
        layout.set_display(Display::Grid);
        if let Some(gap) = self.spec.column_gap {
            layout.set_gap(gap, Gutter::Column);
        }
        if let Some(gap) = self.spec.row_gap {
            layout.set_gap(gap, Gutter::Row);
        }
        if let Some(align) = self.spec.align_items {
            layout.set_align_items(align);
        }
        // A container can also be placed inside its parent's grid, so keep the
        // placement fields this spec does not own.
        let mut grid = layout.style().grid;
        grid.columns.clone_from(&self.spec.columns);
        grid.rows.clone_from(&self.spec.rows);
        grid.auto_columns.clone_from(&self.spec.auto_columns);
        grid.auto_rows.clone_from(&self.spec.auto_rows);
        grid.auto_flow = self.spec.flow;
        grid.areas.clone_from(&self.spec.areas);
        grid.column_names.clone_from(&self.spec.column_names);
        grid.row_names.clone_from(&self.spec.row_names);
        if self.spec.justify_items.is_some() {
            grid.justify_items = self.spec.justify_items;
        }
        layout.set_grid_style(grid);
    }
}

impl ViewModifier for GridModifier {
    fn apply_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            self.apply(&layout);
        }
    }
}

pub struct GridCellModifier(pub GridCell);

impl ViewModifier for GridCellModifier {
    fn apply_node(&self, node: &Node) {
        let Some(layout) = node.layout_node() else {
            return;
        };
        // Read-modify-write so stacking `.grid_cell(...)` with a per-item
        // alignment keeps both; replacing the whole style would silently drop
        // whichever modifier ran first.
        let mut grid = layout.style().grid;
        grid.column.clone_from(&self.0.column);
        grid.row.clone_from(&self.0.row);
        layout.set_grid_style(grid);
    }
}

/// Per-item alignment inside its cell (CSS `justify-self` / `align-self`).
pub struct GridSelfAlignmentModifier {
    pub justify_self: Option<Align>,
    pub align_self: Option<Align>,
}

impl ViewModifier for GridSelfAlignmentModifier {
    fn apply_node(&self, node: &Node) {
        let Some(layout) = node.layout_node() else {
            return;
        };
        let mut grid = layout.style().grid;
        if self.justify_self.is_some() {
            grid.justify_self = self.justify_self;
        }
        layout.set_grid_style(grid);
        if let Some(align) = self.align_self {
            layout.set_align_self(align);
        }
    }
}

pub trait GridExt: View + Sized + 'static {
    /// Makes this view a CSS Grid container.
    fn grid(self, spec: GridSpec) -> ModifiedContent {
        self.modifier(GridModifier::new(spec))
    }

    /// Places this view on explicit grid lines, or auto-places it with a span.
    fn grid_cell(self, cell: GridCell) -> ModifiedContent {
        self.modifier(GridCellModifier(cell))
    }

    /// Auto-placed shorthand for [`GridCell::span`].
    fn grid_span(self, columns: u16, rows: u16) -> ModifiedContent {
        self.grid_cell(GridCell::span(columns, rows))
    }

    /// Horizontal placement of this item inside its cell (`justify-self`).
    fn grid_justify_self(self, align: Align) -> ModifiedContent {
        self.modifier(GridSelfAlignmentModifier {
            justify_self: Some(align),
            align_self: None,
        })
    }

    /// Vertical placement of this item inside its cell (`align-self`).
    fn grid_align_self(self, align: Align) -> ModifiedContent {
        self.modifier(GridSelfAlignmentModifier {
            justify_self: None,
            align_self: Some(align),
        })
    }
}
impl<T: View + Sized + 'static> GridExt for T {}
