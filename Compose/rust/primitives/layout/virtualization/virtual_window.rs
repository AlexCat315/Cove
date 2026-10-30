//! Retained visible-range tracking for virtualized layouts.
use cove_core::{Point, Size};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct VirtualWindow {
    pub offset: Point,
    range: Range<usize>,
    count: usize,
    leading_guard: usize,
    trailing_guard: usize,
    size: Size,
}

impl Default for VirtualWindow {
    fn default() -> Self {
        Self {
            offset: Point::default(),
            range: 0..0,
            count: 0,
            leading_guard: 0,
            trailing_guard: 0,
            size: Size::default(),
        }
    }
}

impl VirtualWindow {
    pub fn preparation(offset: f64, viewport: f64, preload: f32) -> (f64, f64) {
        let fraction = if preload.is_finite() { preload.clamp(0.0, 4.0) } else { 0.3 };
        let inset = viewport.max(0.0) * f64::from(fraction) / 2.0;
        let start = (offset - inset).max(0.0);
        (start, viewport.max(1.0) + offset - start + inset)
    }

    pub fn record(&mut self, range: Range<usize>, visible: Range<usize>, count: usize, size: Size) {
        self.range = range;
        self.count = count;
        self.size = size;
        self.leading_guard = visible.start.saturating_sub(self.range.start) / 2;
        self.trailing_guard = self.range.end.saturating_sub(visible.end) / 2;
    }

    pub fn needs_refill(&self, visible: Range<usize>, size: Size) -> bool {
        if self.size != size { return true; }
        if self.count == 0 || size.height <= 0.0 { return false; }
        if self.range.is_empty() { return true; }
        let first = self.range.start + if self.range.start > 0 { self.leading_guard } else { 0 };
        let last = self.range.end - if self.range.end < self.count { self.trailing_guard } else { 0 };
        visible.start < first || visible.end > last
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualRowKey {
    pub generation: u128,
    pub index: usize,
    pub extent: f32,
    pub width: f32,
}

impl VirtualRowKey {
    pub fn new(generation: u128, index: usize, extent: f32) -> Self {
        Self { generation, index, extent, width: 0.0 }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}
