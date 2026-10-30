//! Translation of Primitives/Layout/Virtualization/RowIndex.swift.
//!
//! Cumulative offsets for variable-height rows, with prefix sums so a range
//! query never walks the whole collection.
use std::ops::Range;

/// Cumulative row offsets for a variable-height list.
#[derive(Clone, Debug, Default)]
pub struct VariableRowIndex<Id: Ord + Clone> {
    ids: Vec<Id>,
    heights: Vec<f32>,
    /// `prefix[i]` is the offset of row `i`; `prefix[len]` is the total extent.
    prefix: Vec<f64>,
    spacing: f32,
}

impl<Id: Ord + Clone> VariableRowIndex<Id> {
    pub fn new() -> Self {
        Self { ids: Vec::new(), heights: Vec::new(), prefix: Vec::new(), spacing: 0.0 }
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn ids(&self) -> &[Id] {
        &self.ids
    }

    pub fn total_height(&self) -> f64 {
        self.prefix.last().copied().unwrap_or(0.0)
    }

    pub fn height_of(&self, row: usize) -> f32 {
        self.heights.get(row).copied().unwrap_or(0.0)
    }

    pub fn offset_of(&self, row: usize) -> f64 {
        self.prefix.get(row).copied().unwrap_or_else(|| self.total_height())
    }

    /// Replace the whole index. `spacing` sits between adjacent rows.
    pub fn replace(&mut self, ids: &[Id], heights: &[f32], spacing: f32) {
        self.spacing = if spacing.is_finite() { spacing.max(0.0) } else { 0.0 };
        self.ids = ids.to_vec();
        self.heights = heights.iter().map(|height| if height.is_finite() { height.max(0.0) } else { 0.0 }).collect();
        self.prefix = Vec::with_capacity(self.heights.len() + 1);
        let mut cursor = 0.0_f64;
        for (index, height) in self.heights.iter().enumerate() {
            self.prefix.push(cursor);
            cursor += f64::from(*height);
            if index + 1 < self.heights.len() {
                cursor += f64::from(self.spacing);
            }
        }
        self.prefix.push(cursor);
    }

    /// Clamp `offset` so the last row stays reachable.
    pub fn clamp(&self, offset: f64, viewport: f64) -> f64 {
        let maximum = (self.total_height() - viewport.max(0.0)).max(0.0);
        offset.clamp(0.0, maximum)
    }

    /// Rows intersecting `[offset, offset + viewport)`.
    pub fn visible_range(&self, offset: f64, viewport: f64) -> Range<usize> {
        if self.ids.is_empty() {
            return 0..0;
        }
        let start = self.row_at(offset);
        let end = self.row_at(offset + viewport.max(0.0)).min(self.ids.len() - 1) + 1;
        start..end.max(start)
    }

    /// Row containing `offset`, clamped to the last row.
    pub fn row_at(&self, offset: f64) -> usize {
        if self.ids.is_empty() {
            return 0;
        }
        let offset = offset.max(0.0);
        let mut lower = 0usize;
        let mut upper = self.ids.len();
        while lower + 1 < upper {
            let middle = (lower + upper) / 2;
            if self.offset_of(middle) <= offset {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        lower
    }

    /// Row id at the top of the viewport, used as the scroll anchor.
    pub fn anchor_at(&self, offset: f64) -> Option<Id> {
        if self.ids.is_empty() {
            return None;
        }
        self.ids.get(self.row_at(offset)).cloned()
    }

    /// Offset that brings `anchor` back to the top of the viewport.
    pub fn offset_for(&self, anchor: &Id, viewport: f64) -> f64 {
        match self.ids.iter().position(|id| id == anchor) {
            Some(row) => self.clamp(self.offset_of(row), viewport),
            None => 0.0,
        }
    }
}
