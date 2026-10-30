//! Prefix geometry for horizontally virtualized data table columns.
use std::ops::Range;

pub struct DataTableColumnGeometry {
    pub widths: Vec<f32>,
    starts: Vec<f32>,
    pub frozen_count: usize,
    pub total_width: f32,
}

impl Clone for DataTableColumnGeometry {
    fn clone(&self) -> Self {
        Self { widths: self.widths.clone(), starts: self.starts.clone(), frozen_count: self.frozen_count, total_width: self.total_width }
    }
}

impl DataTableColumnGeometry {
    pub fn new(widths: Vec<f32>, frozen_count: usize) -> Self {
        let frozen_count = frozen_count.min(widths.len());
        let mut starts = Vec::with_capacity(widths.len());
        let mut total_width = 0.0;
        for width in &widths {
            starts.push(total_width);
            total_width += *width;
        }
        Self { widths, starts, frozen_count, total_width }
    }

    /// Left edge of every column, in header coordinates.
    pub fn starts(&self) -> &[f32] {
        &self.starts
    }

    pub fn frozen_width(&self) -> f32 {
        self.starts.get(self.frozen_count).copied().unwrap_or(self.total_width)
    }

    pub fn moving_width(&self) -> f32 { self.total_width - self.frozen_width() }

    pub fn visible_columns(&self, offset: f32, width: f32) -> Range<usize> {
        if self.frozen_count >= self.widths.len() {
            return self.frozen_count..self.frozen_count;
        }
        let left = offset.max(0.0) + self.frozen_width();
        let right = left + width.max(1.0);
        let insertion = |value: f32| {
            let (mut lower, mut upper) = (self.frozen_count, self.starts.len());
            while lower < upper {
                let middle = (lower + upper) / 2;
                if self.starts[middle] < value { lower = middle + 1; } else { upper = middle; }
            }
            lower
        };
        let lower = self.frozen_count.max(insertion(left).saturating_sub(1));
        lower..self.widths.len().min((lower + 1).max(insertion(right) + 1))
    }
}
