//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/RowHeights.swift.
use std::ops::Range;

use super::columns::GridTrackResolver;

/// Authored row heights for `LazyGrid`. Construct variable heights when the
/// data changes and reuse the value while scrolling; prefix sums are built
/// once, and viewport lookup is logarithmic in the number of rows.
/// Supply unscaled logical heights; LazyGrid applies the active TextScaler.
#[derive(Clone, Debug, PartialEq)]
pub struct GridRowHeights {
    storage: GridRowHeightStorage,
    scale: f32,
}
#[derive(Clone, Debug, PartialEq)]
enum GridRowHeightStorage {
    Fixed(f32),
    Variable(Vec<f32>),
}
impl GridRowHeights {
    /// Every row has this height. Fixed rows need no per-row allocation.
    pub fn fixed(height: f32) -> Self {
        Self { storage: GridRowHeightStorage::Fixed(Self::normalized_height(height)), scale: 1.0 }
    }

    /// One known height per row, including the last partial row. Heights must
    /// be updated together with the data or column count. Unknown content
    /// heights require an eager `Grid` measurement instead.
    pub fn variable(heights: &[f32]) -> Self {
        let mut offsets = Vec::with_capacity(heights.len() + 1);
        offsets.push(0.0);
        for height in heights {
            offsets.push(offsets[offsets.len() - 1] + Self::normalized_height(*height));
        }
        Self { storage: GridRowHeightStorage::Variable(offsets), scale: 1.0 }
    }

    pub fn validate(&self, row_count: usize) {
        if let GridRowHeightStorage::Variable(offsets) = &self.storage {
            assert_eq!(offsets.len(), row_count + 1,
                "LazyGrid requires one variable height per row, including the last partial row");
        }
    }

    /// Reuse authored prefix sums when the text scale changes or rows scroll.
    pub fn scaled(&self, factor: f32) -> Self {
        let mut result = self.clone();
        result.scale *= factor;
        result
    }

    pub fn height(&self, row: usize) -> f32 {
        match &self.storage {
            GridRowHeightStorage::Fixed(height) => height * self.scale,
            GridRowHeightStorage::Variable(offsets) => (offsets[row + 1] - offsets[row]) * self.scale,
        }
    }

    pub fn offset(&self, row: usize, spacing: f32) -> f32 {
        let gap = GridTrackResolver::finite_nonnegative(spacing);
        match &self.storage {
            GridRowHeightStorage::Fixed(height) => row as f32 * (height * self.scale + gap),
            GridRowHeightStorage::Variable(offsets) => offsets[row] * self.scale + row as f32 * gap,
        }
    }

    pub fn total_height(&self, row_count: usize, spacing: f32) -> f32 {
        if row_count == 0 {
            return 0.0;
        }
        self.offset(row_count - 1, spacing) + self.height(row_count - 1)
    }

    pub fn visible_rows(&self, count: usize, offset: f32, viewport_height: f32,
                        spacing: f32, overscan: usize) -> Range<usize> {
        if count == 0 {
            return 0..0;
        }
        let gap = GridTrackResolver::finite_nonnegative(spacing);
        let viewport = 1.0f32.max(GridTrackResolver::finite_nonnegative(viewport_height));
        let total = self.total_height(count, gap);
        let start = GridTrackResolver::finite_nonnegative(offset)
            .min((total - viewport).max(0.0));
        let end = start + viewport;
        let (lower, upper) = match &self.storage {
            GridRowHeightStorage::Fixed(height) => {
                let stride = height * self.scale + gap;
                (Self::bounded_index((start + gap) / stride, count),
                 Self::bounded_index((end / stride).ceil(), count))
            }
            GridRowHeightStorage::Variable(_) => {
                let lower = self.first_index(count, |row| self.offset(row, gap) + self.height(row) > start);
                let upper = self.first_index(count, |row| self.offset(row, gap) >= end);
                (lower, upper)
            }
        };
        let extra = overscan;
        let lower_start = lower.saturating_sub(lower.min(extra));
        let upper_end = count.min(upper.max(lower) + (count - upper.max(lower)).min(extra));
        lower_start..upper_end
    }

    fn first_index(&self, count: usize, predicate: impl Fn(usize) -> bool) -> usize {
        let mut lower = 0usize;
        let mut upper = count;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if predicate(middle) {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        lower
    }

    fn bounded_index(value: f32, count: usize) -> usize {
        if !(value > 0.0) {
            return 0;
        }
        if value < count as f32 {
            value as usize
        } else {
            count
        }
    }

    fn normalized_height(height: f32) -> f32 {
        1.0f32.max(GridTrackResolver::finite_nonnegative(height))
    }
}
