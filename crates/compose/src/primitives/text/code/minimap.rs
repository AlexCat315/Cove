//! Translation of Primitives/Text/Code/Minimap.swift.
//!
//! A scaled overview of a code field: every visual row as a two-point strip of
//! syntax-coloured bars, with the visible region marked. Taller documents scroll
//! the overview in step with the text, as editors commonly do.
use std::ops::Range;

use cove_core::{Color, Rect};

/// Reserved width on the trailing edge of a vertical field.
pub const MINIMAP_WIDTH: f32 = 84.0;
/// Height of one document row in the overview.
pub const MINIMAP_ROW_HEIGHT: f32 = 2.0;
/// Width of one character column in the overview.
pub const MINIMAP_COLUMN_WIDTH: f32 = 1.0;
const INSET: f32 = 2.0;

/// Where the last frame drew the overview, for pointer hits and drags.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapGeometry {
    pub rect: Rect,
    /// Overview points scrolled past its top.
    pub scroll: f32,
    pub line_height: f32,
}

impl MinimapGeometry {
    /// The overview for `rows` document rows, scrolled in step with the text.
    pub fn new(rect: Rect, rows: usize, offset: f32, max_offset: f32, line_height: f32) -> Self {
        let overflow = (rows as f32 * MINIMAP_ROW_HEIGHT - rect.height).max(0.0);
        let progress = if max_offset > 0.0 {
            (offset / max_offset).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            rect,
            scroll: overflow * progress,
            line_height,
        }
    }

    /// Document rows that intersect the overview, with one row of slack.
    pub fn visible_rows(&self, rows: usize) -> Range<usize> {
        let first = (self.scroll / MINIMAP_ROW_HEIGHT).max(0.0) as usize;
        let last = (first + (self.rect.height / MINIMAP_ROW_HEIGHT) as usize + 2).min(rows);
        first.min(last)..last
    }

    /// Character columns one strip can hold.
    pub fn columns(&self) -> usize {
        ((self.rect.width / MINIMAP_COLUMN_WIDTH) as usize).saturating_sub(4)
    }

    /// The strip for `run` character columns on `row`.
    pub fn strip(&self, row: usize, run: Range<usize>) -> Rect {
        Rect::new(
            self.rect.x + INSET + run.start as f32 * MINIMAP_COLUMN_WIDTH,
            self.rect.y + row as f32 * MINIMAP_ROW_HEIGHT - self.scroll,
            run.len() as f32 * MINIMAP_COLUMN_WIDTH,
            MINIMAP_ROW_HEIGHT - 0.5,
        )
    }

    /// The visible region, as one band over the rows it covers.
    pub fn visible_band(&self, offset: f32, viewport_height: f32) -> Rect {
        let line_height = self.line_height.max(1.0);
        Rect::new(
            self.rect.x,
            self.rect.y + offset / line_height * MINIMAP_ROW_HEIGHT - self.scroll,
            self.rect.width,
            (viewport_height / line_height * MINIMAP_ROW_HEIGHT).max(MINIMAP_ROW_HEIGHT * 2.0),
        )
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.height
    }

    /// The document offset that centres the visible region on overview point
    /// `y`, clamped to the scrollable range.
    pub fn offset_at(&self, y: f32, viewport_height: f32, max_offset: f32) -> f32 {
        let row = (y - self.rect.y + self.scroll) / MINIMAP_ROW_HEIGHT;
        (row * self.line_height - viewport_height / 2.0).clamp(0.0, max_offset.max(0.0))
    }
}

/// Splits one line's UTF-8 bytes into character-column runs of ink. Spaces and
/// tabs separate runs; continuation bytes belong to the character before them.
///
/// Each run carries its columns and the byte offset of its first character, so
/// the caller can ask the syntax colouring for that position.
pub fn ink_runs(bytes: &[u8], tab_width: usize, columns: usize) -> Vec<(Range<usize>, usize)> {
    let tab_width = tab_width.max(1);
    let mut runs = Vec::new();
    let mut column = 0;
    let mut start: Option<(usize, usize)> = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte & 0xC0 == 0x80 {
            continue;
        }
        if byte == b' ' || byte == b'\t' {
            if let Some((first, offset)) = start.take() {
                runs.push((first..column, offset));
            }
            column += if byte == b'\t' {
                tab_width - column % tab_width
            } else {
                1
            };
        } else {
            if start.is_none() {
                start = Some((column, index));
            }
            column += 1;
        }
        if column >= columns {
            break;
        }
    }
    if let Some((first, offset)) = start {
        runs.push((first..column, offset));
    }
    runs
}

/// Bar colour for one strip.
pub fn strip_color(color: Color, opacity: f32) -> Color {
    color.multiplied_alpha(opacity * 0.65)
}
