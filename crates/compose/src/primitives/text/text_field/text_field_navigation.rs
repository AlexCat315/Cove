//! Translation of Primitives/Text/TextField/TextFieldNavigation.swift.
//!
//! Line commands follow the displayed row. Without a shaping environment, the
//! rope still provides a deterministic logical-line boundary.
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_document_layout::TextDocumentLayout;
use cove_core::scene::Point;
use cove_text::TextEnvironment;

/// A caret move caused by a line, page or document command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaretMove {
    pub target: usize,
    pub extend_selection: bool,
    pub line_end_affinity: bool,
    pub preferred_caret_x: Option<u32>,
}

/// Logical or displayed line boundary for the caret.
pub fn line_boundary(
    buffer: &TextBuffer,
    caret: usize,
    ending: bool,
    is_vertical: bool,
    wraps_lines: bool,
    document: Option<&mut TextDocumentLayout>,
    env: &TextEnvironment,
    line_end_affinity: bool,
) -> usize {
    if !is_vertical {
        return if ending { buffer.character_count() } else { 0 };
    }
    if wraps_lines && let Some(document) = document {
        let (_, row) = document.caret(caret, line_end_affinity, env);
        let rows = document.index().row_count();
        return document.character(
            row.min(rows.saturating_sub(1)),
            if ending { f32::INFINITY } else { 0.0 },
            env,
        );
    }
    let range = buffer.line_range(buffer.line_index_for_character_index(caret));
    if ending { range.end } else { range.start }
}

/// Vertical caret move that keeps the preferred x across short rows.
pub fn move_cursor_vertically(
    buffer: &TextBuffer,
    caret: usize,
    line_delta: isize,
    preferred_caret_x: Option<f32>,
    line_end_affinity: bool,
    document: Option<&mut TextDocumentLayout>,
    env: Option<&TextEnvironment>,
) -> CaretMove {
    if line_delta == 0 {
        return CaretMove {
            target: caret,
            ..Default::default()
        };
    }
    match (document, env) {
        (Some(document), Some(env)) => {
            let (x, row) = document.caret(caret, line_end_affinity, env);
            let rows = document.index().row_count();
            let target_row =
                (row as isize + line_delta).clamp(0, rows.saturating_sub(1) as isize) as usize;
            let desired_x = preferred_caret_x.unwrap_or(x);
            let target = document.character(target_row, desired_x, env);
            let (_, landed_row) = document.caret(target, true, env);
            CaretMove {
                target,
                extend_selection: false,
                line_end_affinity: landed_row == target_row,
                preferred_caret_x: Some(desired_x.to_bits()),
            }
        }
        _ => {
            let (line, column) = buffer.line_and_column(caret);
            let row = (line as isize + line_delta)
                .clamp(0, buffer.line_count().saturating_sub(1) as isize)
                as usize;
            // Without shaping there is no pixel column to preserve, so the
            // preferred column travels in `preferred_caret_x` as a character
            // index: a tall line visited first keeps the column of a short line
            // on the way back.
            let desired = preferred_caret_x
                .map(|x| x.max(0.0) as usize)
                .unwrap_or(column);
            let range = buffer.line_range(row);
            let target = buffer.character_index_for_line(row, desired.min(range.end - range.start));
            CaretMove {
                target,
                extend_selection: false,
                line_end_affinity: false,
                preferred_caret_x: Some((desired as f32).to_bits()),
            }
        }
    }
}

/// Page move: one viewport of rows, leaving one row of context.
pub fn move_cursor_by_page(
    buffer: &TextBuffer,
    caret: usize,
    direction: isize,
    frame_height: f32,
    line_height: f32,
    vertical_inset: f32,
    preferred_caret_x: Option<f32>,
    document: Option<&mut TextDocumentLayout>,
    env: Option<&TextEnvironment>,
) -> Option<CaretMove> {
    let height = line_height.max(1.0);
    let visible = height.max(frame_height - vertical_inset * 2.0);
    let rows = ((visible / height).floor() as isize - 1).max(1);
    let moved = move_cursor_vertically(
        buffer,
        caret,
        direction * rows,
        preferred_caret_x,
        false,
        document,
        env,
    );
    Some(moved)
}

/// How one character participates in word motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharKind {
    Word,
    Space,
    Other,
}

/// Translation of `TextField.wordKind(_:)`: letters, digits and `_` form a word
/// run; whitespace is skipped first; everything else is punctuation.
pub fn char_kind(character: &str) -> CharKind {
    let scalar = character.chars().next();
    if scalar.is_some_and(|value| value.is_alphanumeric() || value == '_') {
        CharKind::Word
    } else if scalar.is_some_and(char::is_whitespace) {
        CharKind::Space
    } else {
        CharKind::Other
    }
}

/// Option+Left / Option+Backspace target: skip the whitespace immediately
/// before the caret, then the run of same-kind characters before it.
pub fn word_boundary_before(buffer: &TextBuffer, offset: usize) -> usize {
    let count = buffer.character_count();
    let mut cursor = offset.min(count);
    while cursor > 0
        && buffer
            .character_at(cursor - 1)
            .is_some_and(|c| is_space(&c))
    {
        cursor -= 1;
    }
    let Some(previous) = cursor
        .checked_sub(1)
        .and_then(|index| buffer.character_at(index))
    else {
        return 0;
    };
    let kind = char_kind(&previous);
    while cursor > 0
        && buffer
            .character_at(cursor - 1)
            .is_some_and(|c| char_kind(&c) == kind)
    {
        cursor -= 1;
    }
    cursor
}

/// Option+Right / Option+Delete target: skip the whitespace ahead, then the run
/// of same-kind characters after it.
pub fn word_boundary_after(buffer: &TextBuffer, offset: usize) -> usize {
    let count = buffer.character_count();
    let mut cursor = offset.min(count);
    while cursor < count && buffer.character_at(cursor).is_some_and(|c| is_space(&c)) {
        cursor += 1;
    }
    let Some(next) = buffer.character_at(cursor) else {
        return count;
    };
    let kind = char_kind(&next);
    while cursor < count
        && buffer
            .character_at(cursor)
            .is_some_and(|c| char_kind(&c) == kind)
    {
        cursor += 1;
    }
    cursor
}

fn is_space(character: &str) -> bool {
    char_kind(character) == CharKind::Space
}

/// The visible window a text origin maps to, used by hit testing.
pub fn character_index_at_point(
    document: &mut TextDocumentLayout,
    point: Point,
    draw_origin: Point,
    text_origin_y_offset: f32,
    leading_inset: f32,
    scroll: (f32, f32),
    line_height: f32,
    env: &TextEnvironment,
) -> usize {
    let x = point.x - draw_origin.x - leading_inset + scroll.0;
    let y = point.y - draw_origin.y - text_origin_y_offset + scroll.1;
    let row = ((y.max(0.0) / line_height.max(1.0)).floor()) as usize;
    document.character(row, x, env)
}
