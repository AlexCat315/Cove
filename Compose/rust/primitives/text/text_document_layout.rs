//! Bounded line shaping and an incremental height index for one surface. The
//! cached strings contain individual lines, never the complete document.

use super::text_buffer::TextBuffer;
use super::text_field::text_visual_line_index::TextVisualLineIndex;
use cove_core::{Font, PositionedGlyph, TextAlignment, TextLayoutResult, TextLine};
use cove_text::TextEnvironment;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TextLineGeometry {
    pub font: Font,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub atlas: u64,
    pub width: f32,
    pub secure: bool,
}

pub struct TextDocumentLayout {
    buffer: TextBuffer,
    geometry: TextLineGeometry,
    index: TextVisualLineIndex,
    lines: HashMap<usize, MeasuredTextLine>,
    access_order: HashMap<usize, u64>,
    clock: u64,
    pub shaped_line_count: usize,
    pub shaped_utf8_count: usize,
    hidden_lines: Vec<std::ops::Range<usize>>,
}

impl TextDocumentLayout {
    /// Unwrapped lines longer than this are shaped in segments.
    pub const SEGMENT_BYTES: usize = 8_192;
    pub const RETAINED_LINES: usize = 256;

    pub fn new(buffer: TextBuffer, geometry: TextLineGeometry) -> Self {
        let index = TextVisualLineIndex::new(buffer.line_count());
        Self {
            buffer,
            geometry,
            index,
            lines: HashMap::new(),
            access_order: HashMap::new(),
            clock: 0,
            shaped_line_count: 0,
            shaped_utf8_count: 0,
            hidden_lines: Vec::new(),
        }
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn geometry(&self) -> &TextLineGeometry {
        &self.geometry
    }

    pub fn index(&self) -> &TextVisualLineIndex {
        &self.index
    }

    pub fn update(&mut self, next: TextBuffer, next_geometry: TextLineGeometry) {
        if self.geometry != next_geometry {
            self.geometry = next_geometry;
            self.buffer = next;
            self.index = TextVisualLineIndex::new(self.buffer.line_count());
            self.lines.clear();
            self.access_order.clear();
            self.hidden_lines.clear();
            return;
        }
        if next == self.buffer {
            return;
        }
        if let Some(delta) = next.edit_delta(&self.buffer) {
            let first = self
                .buffer
                .line_index_for_character_index(delta.character_range.start);
            let last = self
                .buffer
                .line_index_for_character_index(delta.character_range.end);
            let new_last = next.line_index_for_character_index(
                next.character_index_for_utf8_offset(delta.new_end_utf8_offset),
            );
            let shift = next.line_count() as isize - self.buffer.line_count() as isize;
            self.index.replace(first..(last + 1), new_last - first + 1);
            // At most 256 retained entries are rebased; work is independent of
            // document length. The persistent row index splices in O(log n).
            let mut kept: HashMap<usize, MeasuredTextLine> = HashMap::new();
            let mut order: HashMap<usize, u64> = HashMap::new();
            for (line, entry) in &self.lines {
                if *line < first || *line > last {
                    let key = if *line > last {
                        (*line as isize + shift) as usize
                    } else {
                        *line
                    };
                    kept.insert(key, entry.clone());
                    if let Some(access) = self.access_order.get(line) {
                        order.insert(key, *access);
                    }
                }
            }
            self.lines = kept;
            self.access_order = order;
        }
        self.buffer = next;
        if !self.hidden_lines.is_empty() {
            self.hidden_lines.clear();
            self.index = TextVisualLineIndex::new(self.buffer.line_count());
            for (line, entry) in &self.lines {
                self.index.measure(*line, entry.rows.len());
            }
        }
    }

    pub fn measured_line(&mut self, line: usize, env: &TextEnvironment) -> MeasuredTextLine {
        let row = (self.buffer.line_count() - 1).min(line);
        self.clock = self.clock.wrapping_add(1);
        self.access_order.insert(row, self.clock);
        if let Some(entry) = self.lines.get(&row) {
            return entry.clone();
        }
        let source = self.buffer.substring(self.buffer.line_range(row));
        let text = if self.geometry.secure {
            "\u{2022}".repeat(source.chars().count())
        } else {
            source.clone()
        };
        let result = if !self.geometry.width.is_finite() && text.len() > Self::SEGMENT_BYTES {
            self.segmented_layout(&text, env)
        } else {
            (*env.cached_layout(
                &text,
                Some(&self.geometry.font),
                Some(self.geometry.line_height),
                self.geometry.width,
                TextAlignment::Leading,
                self.geometry.letter_spacing,
            ))
            .clone()
        };
        let entry = MeasuredTextLine::new(&source, &text, &result, self.geometry.line_height);
        self.lines.insert(row, entry.clone());
        self.index.measure(row, entry.rows.len());
        self.shaped_line_count += 1;
        self.shaped_utf8_count += text.len();
        if self.lines.len() > Self::RETAINED_LINES {
            if let Some((oldest, _)) = self
                .access_order
                .iter()
                .min_by_key(|(_, access)| **access)
                .map(|(line, access)| (*line, *access))
            {
                self.lines.remove(&oldest);
                self.access_order.remove(&oldest);
            }
        }
        entry
    }

    /// Shapes a very long unwrapped line in bounded segments placed end to end,
    /// preferring to cut after a space so shaping context rarely spans a cut.
    /// Each segment hits the shared layout cache on its own, so an edit near the
    /// end of a minified file reshapes only the segments it touches.
    fn segmented_layout(&self, text: &str, env: &TextEnvironment) -> TextLayoutResult {
        let mut glyphs: Vec<PositionedGlyph> = Vec::new();
        let mut x: f32 = 0.0;
        let mut byte = 0usize;
        let mut baseline: Option<f32> = None;
        let mut top: f32 = 0.0;
        let mut start = 0usize;
        let mut cursor = 0usize;
        let mut last_space: Option<usize> = None;
        let mut pending = 0usize;

        let mut flush = |piece: &str,
                         glyphs: &mut Vec<PositionedGlyph>,
                         x: &mut f32,
                         byte: &mut usize,
                         baseline: &mut Option<f32>,
                         top: &mut f32| {
            let layout = env.cached_layout(
                piece,
                Some(&self.geometry.font),
                Some(self.geometry.line_height),
                f32::INFINITY,
                TextAlignment::Leading,
                self.geometry.letter_spacing,
            );
            if let Some(line) = layout.lines.first() {
                if baseline.is_none() {
                    *baseline = Some(line.baseline_y);
                    *top = line.top_y;
                }
                if !glyphs.is_empty() {
                    *x += self.geometry.letter_spacing;
                }
                for glyph in &line.glyphs {
                    glyphs.push(PositionedGlyph {
                        glyph_id: glyph.glyph_id,
                        font_id: glyph.font_id,
                        cluster: glyph.cluster + *byte as u32,
                        x: glyph.x + *x,
                        y: glyph.y,
                        atlas_info: glyph.atlas_info,
                    });
                }
                *x += line.width;
            }
            *byte += piece.len();
        };

        while cursor < text.len() {
            let character = text[cursor..].chars().next().unwrap_or('\0');
            let next = cursor + character.len_utf8();
            pending += character.len_utf8();
            if character == ' ' {
                last_space = Some(next);
            }
            if pending >= Self::SEGMENT_BYTES / 2 {
                let cut = last_space.unwrap_or(next);
                flush(
                    &text[start..cut],
                    &mut glyphs,
                    &mut x,
                    &mut byte,
                    &mut baseline,
                    &mut top,
                );
                start = cut;
                last_space = None;
                pending = next - cut;
            }
            cursor = next;
        }
        if start < text.len() {
            flush(
                &text[start..],
                &mut glyphs,
                &mut x,
                &mut byte,
                &mut baseline,
                &mut top,
            );
        }
        let line = TextLine {
            glyphs,
            baseline_y: baseline.unwrap_or(self.geometry.line_height * 0.8),
            top_y: top,
            width: x,
            start_cluster: 0,
            end_cluster: byte as u32,
        };
        TextLayoutResult {
            lines: vec![line],
            total_width: x,
            total_height: self.geometry.line_height,
        }
    }

    pub fn set_hidden_lines(&mut self, ranges: Vec<std::ops::Range<usize>>) {
        if self.hidden_lines == ranges {
            return;
        }
        self.hidden_lines = ranges;
        self.index = TextVisualLineIndex::new(self.buffer.line_count());
        for (line, entry) in &self.lines {
            self.index.measure(*line, entry.rows.len());
        }
        for range in &self.hidden_lines {
            self.index.hide(range.clone());
        }
    }

    /// Only logical lines intersecting this row window are shaped. Learning a
    /// wrapped line's height can only reduce the remaining work in the window.
    pub fn visible_layout(
        &mut self,
        first_row: usize,
        row_count: usize,
        env: &TextEnvironment,
    ) -> TextLayoutResult {
        let first = self.index.location_at_row(first_row).0;
        let mut result: Vec<TextLine> = Vec::new();
        let mut line = first;
        let mut width: f32 = 0.0;
        let end_row = first_row.max(0) + row_count.max(1);
        while line < self.buffer.line_count() && self.index.first_row_of_line(line) < end_row {
            if let Some(hidden) = self
                .hidden_lines
                .iter()
                .find(|range| range.contains(&line))
                .cloned()
            {
                line = hidden.end;
                continue;
            }
            let entry = self.measured_line(line, env);
            let first_visual_row = self.index.first_row_of_line(line);
            let range = self.buffer.line_range(line);
            let byte_start = self.buffer.utf8_offset_for_character_index(range.start);
            for (subrow, local) in entry.rows.iter().enumerate() {
                let visual_row = first_visual_row + subrow;
                if visual_row < first_row || visual_row >= end_row {
                    continue;
                }
                let y = first_visual_row as f32 * self.geometry.line_height;
                let global = |offset: u32| -> u32 {
                    if self.geometry.secure {
                        let character = entry.display_character_index_for_byte(offset as usize);
                        return (self
                            .buffer
                            .utf8_offset_for_character_index(range.start + character))
                            as u32;
                    }
                    (byte_start + offset as usize) as u32
                };
                let glyphs: Vec<PositionedGlyph> = local
                    .glyphs
                    .iter()
                    .map(|glyph| PositionedGlyph {
                        glyph_id: glyph.glyph_id,
                        font_id: glyph.font_id,
                        cluster: global(glyph.cluster),
                        x: glyph.x,
                        y: glyph.y + y,
                        atlas_info: glyph.atlas_info,
                    })
                    .collect();
                result.push(TextLine {
                    glyphs,
                    baseline_y: local.baseline_y + y,
                    top_y: local.top_y + y,
                    width: local.width,
                    start_cluster: global(local.start_cluster),
                    end_cluster: global(local.end_cluster),
                });
                width = width.max(local.width);
            }
            line += 1;
        }
        TextLayoutResult {
            lines: result,
            total_width: width,
            total_height: self.index.row_count() as f32 * self.geometry.line_height,
        }
    }

    pub fn caret(
        &mut self,
        cursor: usize,
        line_end_affinity: bool,
        env: &TextEnvironment,
    ) -> (f32, usize) {
        let position = self.buffer.line_and_column(cursor);
        if let Some(hidden) = self
            .hidden_lines
            .iter()
            .find(|range| range.contains(&position.0))
            .cloned()
        {
            return self.caret(
                self.buffer.line_range(hidden.start.saturating_sub(1)).end,
                line_end_affinity,
                env,
            );
        }
        let line = self.measured_line(position.0, env);
        let local = line.caret(position.1, line_end_affinity);
        (local.0, self.index.first_row_of_line(position.0) + local.1)
    }

    pub fn character(&mut self, row: usize, x: f32, env: &TextEnvironment) -> usize {
        let mut location = self.index.location_at_row(row);
        let line = self.measured_line(location.0, env);
        location = self.index.location_at_row(row);
        // Measuring can reveal more subrows of the same logical line.
        let column = line.character(location.1, x);
        self.buffer.character_index_for_line(location.0, column)
    }
}

/// A bounded line and its glyph-cluster caret map. Multiple glyphs in an emoji
/// or combining cluster cannot create extra cursor stops. Ligatures interpolate
/// across their constituent character boundaries.
#[derive(Clone)]
pub struct MeasuredTextLine {
    pub rows: Vec<TextLine>,
    boundaries: Vec<usize>,
    carets: Vec<Vec<TextCharacterCaret>>,
}

impl MeasuredTextLine {
    pub fn new(source: &str, display: &str, layout: &TextLayoutResult, line_height: f32) -> Self {
        let _ = source;
        let mut boundaries = vec![0usize];
        let mut offset = 0usize;
        for character in display.chars() {
            offset += character.len_utf8();
            boundaries.push(offset);
        }
        let rows = if layout.lines.is_empty() {
            vec![TextLine {
                glyphs: Vec::new(),
                baseline_y: line_height * 0.8,
                top_y: 0.0,
                width: 0.0,
                start_cluster: 0,
                end_cluster: 0,
            }]
        } else {
            layout.lines.clone()
        };
        let carets: Vec<Vec<TextCharacterCaret>> = rows
            .iter()
            .map(|row| {
                let mut clusters: Vec<(usize, f32)> = Vec::new();
                for glyph in &row.glyphs {
                    if clusters.last().map(|(byte, _)| *byte) != Some(glyph.cluster as usize) {
                        clusters.push((glyph.cluster as usize, glyph.x));
                    }
                }
                clusters.push((row.end_cluster as usize, row.width));
                let mut positions: Vec<TextCharacterCaret> = Vec::new();
                let mut cluster = 0usize;
                for (character, byte) in boundaries.iter().enumerate() {
                    if *byte < row.start_cluster as usize || *byte > row.end_cluster as usize {
                        continue;
                    }
                    while cluster + 1 < clusters.len() && clusters[cluster + 1].0 <= *byte {
                        cluster += 1;
                    }
                    let current = clusters[cluster];
                    let mut x = current.1;
                    if cluster + 1 < clusters.len() && *byte > current.0 {
                        // Boundaries are sorted and indexed by character, so the
                        // ligature span is found by binary search, not a scan.
                        let next = clusters[cluster + 1];
                        let lower = Self::first_index(&boundaries, current.0);
                        let upper = Self::first_index(&boundaries, next.0 + 1);
                        let fraction = (character as f32 - lower as f32)
                            / ((upper - lower).saturating_sub(1).max(1)) as f32;
                        x += (next.1 - x) * fraction;
                    }
                    positions.push(TextCharacterCaret { character, x });
                }
                if positions.is_empty() {
                    positions.push(TextCharacterCaret {
                        character: 0,
                        x: 0.0,
                    });
                }
                positions
            })
            .collect();
        Self {
            rows,
            boundaries,
            carets,
        }
    }

    pub fn first_index(boundaries: &[usize], at_least: usize) -> usize {
        let mut lower = 0usize;
        let mut upper = boundaries.len();
        while lower < upper {
            let middle = (lower + upper) / 2;
            if boundaries[middle] < at_least {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        lower
    }

    pub fn display_character_index_for_byte(&self, byte: usize) -> usize {
        let mut lower = 0usize;
        let mut upper = self.boundaries.len() - 1;
        while lower < upper {
            let middle = (lower + upper) / 2;
            if self.boundaries[middle] < byte {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        lower
    }

    pub fn caret(&self, character: usize, line_end_affinity: bool) -> (f32, usize) {
        let cursor = (self.boundaries.len() - 1).min(character);
        for (row, positions) in self.carets.iter().enumerate() {
            if let Some(position) = positions
                .iter()
                .find(|position| position.character == cursor)
            {
                if cursor < positions.last().map_or(0, |last| last.character)
                    || row == self.carets.len() - 1
                    || line_end_affinity
                {
                    return (position.x, row);
                }
            }
        }
        // A wrapping separator can be omitted by the shaper; attach it to the
        // next row, preserving a deterministic stop rather than inventing one.
        for (row, positions) in self.carets.iter().enumerate() {
            if let Some(position) = positions
                .iter()
                .find(|position| position.character >= cursor)
            {
                return (position.x, row);
            }
        }
        (
            self.rows.last().map_or(0.0, |row| row.width),
            self.rows.len().saturating_sub(1),
        )
    }

    pub fn character(&self, row: usize, x: f32) -> usize {
        let positions = &self.carets[(self.carets.len() - 1).min(row)];
        for index in 0..positions.len().saturating_sub(1) {
            if x < (positions[index].x + positions[index + 1].x) / 2.0 {
                return positions[index].character;
            }
        }
        positions.last().map_or(0, |position| position.character)
    }
}

#[derive(Clone, Copy, Debug)]
struct TextCharacterCaret {
    character: usize,
    x: f32,
}
