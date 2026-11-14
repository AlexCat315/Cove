//! Translation of `Primitives/Data/Table/{VariableRows,TableSortWorker}.swift`.
//!
//! Row geometry for the display order, and the off-thread sort projection.
//!
//! Fixed rows are arithmetic; variable rows use prefix sums, so the viewport,
//! scrolling extent, reveal and paging all agree on one source of truth.
//!
//! Behaviour checklist:
//! - a fixed-height table gives every row the same top, height and total
//! - a variable-height table uses the prefix index for all three
//! - `row_at` finds the containing row and clamps past the end
//! - `visible_range` agrees between both modes
//! - the sort is stable: equal comparisons retain source order in both
//!   directions
//! - the projection maps ids to their display position
//! - a stale sort delivery is rejected by revision

use std::collections::HashMap;
use std::rc::Rc;

use super::super::super::layout::virtualization::VariableRowIndex;
use crate::primitives::data::table::table_column::{
    ComparisonResult, TableSort, TableSortDirection,
};

/// Row geometry for the display order.
#[derive(Clone, Debug)]
pub struct TableRowMetrics {
    pub count: usize,
    /// Height used when the table has no variable-row resolver.
    pub fixed: f32,
    /// Present when rows size to their content.
    pub index: Option<Rc<VariableRowIndex<usize>>>,
}

impl TableRowMetrics {
    pub fn fixed(count: usize, fixed: f32) -> Self {
        Self {
            count,
            fixed,
            index: None,
        }
    }

    pub fn variable(count: usize, fixed: f32, index: VariableRowIndex<usize>) -> Self {
        Self {
            count,
            fixed,
            index: Some(Rc::new(index)),
        }
    }

    pub fn top(&self, row: usize) -> f32 {
        match &self.index {
            Some(index) => index.offset_of(row) as f32,
            None => row as f32 * self.fixed,
        }
    }

    pub fn height(&self, row: usize) -> f32 {
        match &self.index {
            Some(index) => index.height_of(row),
            None => self.fixed,
        }
    }

    pub fn total(&self) -> f32 {
        match &self.index {
            Some(index) => index.total_height() as f32,
            None => self.count as f32 * self.fixed,
        }
    }

    /// Rows intersecting the viewport, with `overscan` extra on each side.
    pub fn visible_range(
        &self,
        offset: f32,
        viewport: f32,
        overscan: usize,
    ) -> std::ops::Range<usize> {
        match &self.index {
            Some(index) => {
                // Clamp first so the offset cannot scroll past the content.
                let clamped = offset.clamp(0.0, (self.total() - viewport).max(0.0));
                expand(
                    index.visible_range(f64::from(clamped), f64::from(viewport)),
                    overscan,
                    self.count,
                )
            }
            None => {
                let stride = self.fixed.max(1.0);
                let first = (offset / stride).floor().max(0.0) as usize;
                let last = ((offset + viewport) / stride).ceil().max(0.0) as usize;
                expand(first..last, overscan, self.count)
            }
        }
    }

    /// The row containing a content offset.
    pub fn row_at(&self, offset: f32) -> usize {
        if self.count == 0 {
            return 0;
        }
        match &self.index {
            Some(index) => index.row_at(f64::from(offset.max(0.0))).min(self.count - 1),
            None => ((offset / self.fixed.max(1.0)) as usize).min(self.count - 1),
        }
    }
}

fn expand(range: std::ops::Range<usize>, overscan: usize, count: usize) -> std::ops::Range<usize> {
    let start = range.start.saturating_sub(overscan).min(count);
    let end = (range.end + overscan).min(count);
    start..end.max(start)
}

/// Cache key for the variable-row index: rebuild once per model revision, row
/// count, minimum height or text scale — never while scrolling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowHeightKey {
    pub revision: u64,
    pub count: usize,
    pub minimum: f32,
    pub scale: f32,
}

/// Whether the cached index still applies.
pub fn row_height_key_matches(current: Option<RowHeightKey>, next: RowHeightKey) -> bool {
    current == Some(next)
}

/// Builds the heights for a variable-row table.
///
/// Every row is at least `minimum` tall, and the caller's height is scaled by
/// the text scaler before that floor is applied.
pub fn variable_row_heights(
    count: usize,
    minimum: f32,
    scale: f32,
    row_height: impl Fn(usize) -> f32,
) -> Vec<f32> {
    (0..count)
        .map(|row| minimum.max(row_height(row) * scale))
        .collect()
}

// MARK: - Sort projection

/// The display order produced by a sort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableProjection<Id: Clone + Eq + std::hash::Hash> {
    /// Source indices in display order.
    pub order: Vec<usize>,
    /// Display position of each id.
    pub positions: HashMap<Id, usize>,
}

/// One sort request, ready to run off-thread.
pub struct TableSortRequest<Record> {
    pub records: Vec<Record>,
    pub ids: Vec<usize>,
    pub sort: TableSort,
    pub compare: Rc<dyn Fn(&Record, &Record) -> ComparisonResult>,
}

impl<Record> TableSortRequest<Record> {
    /// Sorts the display order.
    ///
    /// Equal comparisons fall back to the source index in **both** directions,
    /// so a descending sort does not reverse ties — that is what makes the order
    /// stable rather than merely deterministic.
    pub fn build(&self) -> TableProjection<usize> {
        let wanted = match self.sort.direction {
            TableSortDirection::Ascending => ComparisonResult::Ascending,
            TableSortDirection::Descending => ComparisonResult::Descending,
        };
        let mut order: Vec<usize> = (0..self.records.len()).collect();
        order.sort_by(|&left, &right| {
            let result = (self.compare)(&self.records[left], &self.records[right]);
            if result == ComparisonResult::Same {
                left.cmp(&right)
            } else if result == wanted {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        let positions = order
            .iter()
            .enumerate()
            .map(|(position, source)| (self.ids[*source], position))
            .collect();
        TableProjection { order, positions }
    }
}

/// Rejects a projection built for an out-of-date row revision or sort command.
pub fn accepts_projection(
    delivered_revision: u64,
    current_revision: u64,
    delivered_sort: &TableSort,
    current_sort: &TableSort,
) -> bool {
    delivered_revision == current_revision && delivered_sort == current_sort
}
