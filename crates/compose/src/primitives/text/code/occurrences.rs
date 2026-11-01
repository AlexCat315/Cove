//! Translation of Primitives/Text/Code/Occurrences.swift.
//!
//! Soft fills behind every occurrence of the symbol at the caret, drawn under the
//! glyphs on visible lines only.
use cove_core::{Color, Rect, TextLayoutResult};

/// Opacity of the occurrence wash relative to the node.
pub const OCCURRENCE_ALPHA: f32 = 0.1;

/// Draw one rounded wash per visible line segment touched by an occurrence.
pub fn draw_occurrences(
    list: &mut cove_core::DrawList,
    layout: &TextLayoutResult,
    ranges: &[std::ops::Range<usize>],
    text_origin_x: f32,
    text_origin_y: f32,
    line_height: f32,
    base_color: Color,
    opacity: f32,
) {
    if ranges.is_empty() {
        return;
    }
    let color = base_color.multiplied_alpha(opacity * OCCURRENCE_ALPHA);
    for line in &layout.lines {
        let first = line.start_cluster as usize;
        let last = line.end_cluster as usize;
        for range in ranges {
            if range.start >= last || range.end <= first {
                continue;
            }
            // Clamp the byte range to this line, then map to glyph x positions.
            let lower = if range.start <= first {
                0.0
            } else {
                line.glyphs
                    .iter()
                    .rev()
                    .find(|glyph| glyph.cluster as usize <= range.start)
                    .map_or(0.0, |glyph| glyph.x)
            };
            let upper = if range.end >= last {
                line.width
            } else {
                line.glyphs
                    .iter()
                    .find(|glyph| glyph.cluster as usize >= range.end)
                    .map_or(line.width, |glyph| glyph.x)
            };
            if upper <= lower {
                continue;
            }
            let _ = list.add_rounded_rect(
                Rect::new(
                    text_origin_x + lower - 1.0,
                    text_origin_y + line.top_y + 1.0,
                    upper - lower + 2.0,
                    (line_height - 2.0).max(0.0),
                ),
                3.0,
                color,
            );
        }
    }
}
