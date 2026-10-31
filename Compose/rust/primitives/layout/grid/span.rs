//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Span.swift.
use crate::{LayoutValueExt, LayoutValueKey, View};

/// Cells a grid child covers. Spans are clamped to the resolved column count;
/// named field columns ignore spans because hiding a field column would
/// otherwise move every spanning cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSpan {
    pub columns: usize,
    pub rows: usize,
}
impl GridSpan {
    pub const SINGLE: Self = Self {
        columns: 1,
        rows: 1,
    };
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns: columns.max(1),
            rows: rows.max(1),
        }
    }
}
impl Default for GridSpan {
    fn default() -> Self {
        Self::SINGLE
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSpanKey;
impl LayoutValueKey for GridSpanKey {
    type Value = GridSpan;
    fn default_value() -> Self::Value {
        GridSpan::SINGLE
    }
}

/// Let this child cover several `Grid` columns and/or rows. Placement is
/// row-major: a cell that does not fit the remaining columns of a row starts
/// the next free row.
pub trait GridCellSpanExt: View + Sized + 'static {
    fn grid_cell_span(self, columns: usize, rows: usize) -> crate::ModifiedContent {
        self.layout_value::<GridSpanKey>(GridSpan::new(columns, rows))
    }
}
impl<T: View + Sized + 'static> GridCellSpanExt for T {}

/// Row-major auto placement with an occupancy table, as in CSS grid's sparse
/// flow. Heights grow so every multi-row cell fits its spanned rows.
#[derive(Clone, Debug, PartialEq)]
pub struct GridSpanPlacement {
    pub cells: Vec<GridSpanCell>,
    pub row_count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSpanCell {
    pub row: usize,
    pub column: usize,
    pub span: GridSpan,
}
impl GridSpanPlacement {
    pub fn new(spans: &[GridSpan], column_count: usize) -> Self {
        let columns = column_count.max(1);
        let mut occupied: Vec<Vec<bool>> = Vec::new();
        let mut cells = Vec::with_capacity(spans.len());
        let mut row = 0usize;
        let mut column = 0usize;
        let is_free = |occupied: &Vec<Vec<bool>>, row: usize, column: usize, span: GridSpan| {
            if column + span.columns > columns {
                return false;
            }
            for r in row..(row + span.rows) {
                if r < occupied.len() {
                    for c in column..(column + span.columns) {
                        if occupied[r][c] {
                            return false;
                        }
                    }
                }
            }
            true
        };
        for raw in spans {
            let span = GridSpan::new(columns.min(raw.columns), raw.rows);
            while !is_free(&occupied, row, column, span) {
                column += 1;
                if column + span.columns > columns {
                    column = 0;
                    row += 1;
                }
            }
            while occupied.len() < row + span.rows {
                occupied.push(vec![false; columns]);
            }
            for r in row..(row + span.rows) {
                for c in column..(column + span.columns) {
                    occupied[r][c] = true;
                }
            }
            cells.push(GridSpanCell { row, column, span });
            column += span.columns;
            if column >= columns {
                column = 0;
                row += 1;
            }
        }
        Self {
            cells,
            row_count: occupied.len(),
        }
    }

    /// Single-row cells set row heights first; a taller multi-row cell then
    /// adds its deficit to the last row it spans.
    pub fn row_heights(&self, item_heights: &[f32], row_spacing: f32) -> Vec<f32> {
        let mut heights = vec![0.0f32; self.row_count];
        for (index, cell) in self.cells.iter().enumerate() {
            if cell.span.rows == 1 {
                heights[cell.row] = heights[cell.row].max(item_heights[index]);
            }
        }
        for (index, cell) in self.cells.iter().enumerate() {
            if cell.span.rows > 1 {
                let rows = cell.row..(cell.row + cell.span.rows);
                let available: f32 = rows.clone().map(|row| heights[row]).sum::<f32>()
                    + row_spacing * (cell.span.rows - 1) as f32;
                if item_heights[index] > available {
                    let last = rows.end - 1;
                    heights[last] += item_heights[index] - available;
                }
            }
        }
        heights
    }
}
