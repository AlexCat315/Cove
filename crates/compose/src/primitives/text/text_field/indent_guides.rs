//! Translation of Primitives/Text/TextField/IndentGuides.swift.
//!
//! Guides occupy the leading whitespace of a scope, never its first token. Only
//! visible rows and their bounded indentation prefixes are inspected.
use std::collections::HashMap;

use crate::primitives::text::text_buffer::TextBuffer;
use cove_core::TextLayoutResult;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndentGuideSegment {
    pub x: f32,
    pub top: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Indent {
    depth: usize,
    blank: bool,
}

/// Guide segments for the visible rows of a document.
pub struct TextIndentGuides;

impl TextIndentGuides {
    pub fn segments(
        layout: &TextLayoutResult,
        source: &TextBuffer,
        tab_width: usize,
        space_width: f32,
        line_height: f32,
        visible_x: (f32, f32),
    ) -> Vec<IndentGuideSegment> {
        if tab_width == 0
            || !space_width.is_finite()
            || space_width <= 0.0
            || !line_height.is_finite()
            || line_height <= 0.0
            || !visible_x.0.is_finite()
            || !visible_x.1.is_finite()
            || visible_x.1 <= visible_x.0
        {
            return Vec::new();
        }
        let step = tab_width as f32 * space_width;
        let first_level = ((visible_x.0 / step).clamp(0.0, 4096.0)) as usize;
        let last_level = first_level.max(((visible_x.1 / step).ceil()).clamp(0.0, 4096.0) as usize);
        let mut cache: HashMap<usize, Indent> = HashMap::new();
        let mut runs: HashMap<usize, IndentGuideSegment> = HashMap::new();
        let mut result: Vec<IndentGuideSegment> = Vec::new();
        let mut prior_depth = 0;
        if let Some(first) = layout.lines.first() {
            let logical = source.line_index_for_character_index(
                source.character_index_for_utf8_offset(first.start_cluster as usize),
            );
            // Recover context at the top of a scrolled blank run without
            // scanning arbitrarily many lines in a large document.
            for index in (logical.saturating_sub(64)..logical).rev() {
                let value = Self::indentation(source, index, tab_width);
                if !value.blank {
                    prior_depth = value.depth;
                    break;
                }
            }
        }
        for line in &layout.lines {
            let logical = source.line_index_for_character_index(
                source.character_index_for_utf8_offset(line.start_cluster as usize),
            );
            let start = source.utf8_offset_for_character_index(source.line_range(logical).start);
            let value = *cache
                .entry(logical)
                .or_insert_with(|| Self::indentation(source, logical, tab_width));
            if !value.blank {
                prior_depth = value.depth;
            }
            // Soft wraps restart at the text origin. A scope guide there would
            // cross the continued word, so only its authored row draws.
            let depth = if line.start_cluster as usize == start {
                prior_depth
            } else {
                0
            };
            let end_level = last_level.min(depth).max(first_level);
            let stale: Vec<usize> = runs
                .keys()
                .copied()
                .filter(|level| !(first_level..end_level).contains(level))
                .collect();
            for level in stale {
                if let Some(completed) = runs.remove(&level) {
                    result.push(completed);
                }
            }
            for level in first_level..end_level {
                let entry = runs.get(&level).copied();
                match entry {
                    Some(run) if (run.top + run.height - line.top_y).abs() < 0.5 => {
                        runs.insert(
                            level,
                            IndentGuideSegment {
                                x: run.x,
                                top: run.top,
                                height: line.top_y + line_height - run.top,
                            },
                        );
                    }
                    _ => {
                        if let Some(completed) = entry {
                            result.push(completed);
                        }
                        runs.insert(
                            level,
                            IndentGuideSegment {
                                x: level as f32 * step + 0.5,
                                top: line.top_y,
                                height: line_height,
                            },
                        );
                    }
                }
            }
        }
        result.extend(runs.into_values());
        result.sort_by(|lhs, rhs| {
            if lhs.x == rhs.x {
                lhs.top.total_cmp(&rhs.top)
            } else {
                lhs.x.total_cmp(&rhs.x)
            }
        });
        result
    }

    fn indentation(source: &TextBuffer, line: usize, tab_width: usize) -> Indent {
        let range = source.line_range(line);
        let start = source.utf8_offset_for_character_index(range.start);
        let end = source.utf8_offset_for_character_index(range.end);
        // Extremely long lines remain owned by the document layout. Guide
        // decoration reads at most 4 KiB of their indentation prefix.
        let mut columns = 0usize;
        let mut offset = start;
        let limit = end.min(start + 4096);
        while offset < limit {
            let chunk_end = limit.min(offset + 64);
            for byte in source.utf8_bytes(offset..chunk_end) {
                match byte {
                    32 => columns += 1,
                    9 => columns += tab_width - columns % tab_width,
                    10 | 13 => {
                        return Indent {
                            depth: 0,
                            blank: true,
                        };
                    }
                    _ => {
                        return Indent {
                            depth: columns / tab_width,
                            blank: false,
                        };
                    }
                }
            }
            offset = chunk_end;
        }
        Indent {
            depth: columns / tab_width,
            blank: offset == end,
        }
    }
}
