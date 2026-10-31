//! Size of one grid cell for a monospaced font.
//!
//! Measured with the host's shaper so painting and hit testing agree with the
//! rasterised glyphs.

use cove_core::Font;
use cove_text::TextEnvironment;

#[derive(Clone, Debug, PartialEq)]
pub struct CellMetrics {
    pub font: Font,
    pub width: f32,
    pub height: f32,
}

impl CellMetrics {
    pub fn new(font: Font, environment: &TextEnvironment) -> Self {
        // Ten ems averages out per-glyph rounding, as the Swift version does.
        let sample = environment.cached_layout(
            "MMMMMMMMMM",
            Some(&font),
            None,
            f32::INFINITY,
            cove_core::TextAlignment::Leading,
            0.0,
        );
        let advance = sample.total_width / 10.0;
        let width = if advance.is_finite() && advance > 0.0 {
            advance
        } else {
            font.size() * 0.6
        };
        let line_height = environment.resolved_line_height(&font, None);
        let height = if line_height.is_finite() && line_height > 0.0 {
            line_height.ceil()
        } else {
            (font.size() * 1.3).ceil()
        };
        Self {
            font,
            width,
            height,
        }
    }

    /// Whole cells that fit in `width` x `height` points (at least 1 x 1).
    pub fn grid(&self, width: f32, height: f32) -> (usize, usize) {
        let columns = (width / self.width).floor();
        let rows = (height / self.height).floor();
        let columns = if columns.is_finite() {
            columns.max(1.0) as usize
        } else {
            1
        };
        let rows = if rows.is_finite() {
            rows.max(1.0) as usize
        } else {
            1
        };
        (columns, rows)
    }

    /// Highlight a link under the pointer, underlined like one.
    pub fn line_step(&self) -> f32 {
        self.height
    }
}
