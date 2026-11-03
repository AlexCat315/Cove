//! Translation of Primitives/Text/TextField/TextFieldEditing.swift.
//!
//! Buffer edits and caret movement over `FieldState`. Every mutation funnels
//! through `record_caret_activity`, which clears the line-end affinity, requests a
//! reveal and reports the new caret — mirroring the Swift `TextField` extensions
//! so the call sites read the same.
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_edit_history::TextEditKind;

use super::text_field::FieldState;
use super::text_field_observation::TextFieldCaretState;

/// Monotonic seconds since the first edit, the unit `TextEditHistory` coalesces
/// adjacent transactions in.
pub fn edit_clock() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// Delete the active selection, if any. Returns `true` when something was
/// deleted, so the caller can skip its own delete-one logic.
pub fn delete_selection(state: &mut FieldState) -> bool {
    let Some((low, high)) = state.selection_range() else {
        return false;
    };
    let current = state.buffer.clone();
    state.begin_edit(&current, TextEditKind::Deletion);
    state.buffer = state.buffer.delete(low..high);
    state.selection.cursor_index = low;
    state.selection.anchor = None;
    state.clear_composition();
    state.selection.set_preferred_caret_x(None);
    record_caret_activity(state);
    state.end_edit();
    true
}

/// Replace the active selection (or insert at the caret) with `incoming`.
pub fn insert_replacing_selection(state: &mut FieldState, incoming: &str) {
    insert_replacing_selection_limited(state, incoming, None);
}

/// `insert_replacing_selection` with an authored `max_length`, in characters.
///
/// The limit applies to the text that remains after the selection is removed,
/// so replacing a selection can never push the buffer past it; the insertion is
/// truncated at the remaining capacity and an empty remainder is a no-op.
pub fn insert_replacing_selection_limited(
    state: &mut FieldState,
    incoming: &str,
    max_length: Option<usize>,
) {
    if incoming.is_empty() {
        return;
    }
    let selection = state.selection_range();
    let cursor = selection.map_or(state.selection.cursor_index, |(low, _)| low);
    let removed = selection.map_or(0, |(low, high)| high - low);
    // A single non-newline character with no selection is typing: it coalesces
    // with the neighbouring keystrokes into one undo step.
    let typing = incoming.chars().count() == 1
        && !incoming.contains('\n')
        && selection.is_none()
        && !state.composition.is_active();
    let current = state.buffer.clone();
    state.begin_edit(
        &current,
        if typing {
            TextEditKind::Typing
        } else {
            TextEditKind::Atomic
        },
    );
    let capacity = max_length.map(|limit| limit.saturating_sub(state.buffer.character_count() - removed));
    let insertion: String = match capacity {
        Some(capacity) => incoming.chars().take(capacity).collect(),
        None => incoming.to_string(),
    };
    if insertion.is_empty() {
        state.end_edit();
        return;
    }
    let start_byte = state.buffer.utf8_offset_for_character_index(cursor);
    state.buffer = state.buffer.replace(cursor..(cursor + removed), &insertion);
    state.clear_composition();
    // A combining scalar or regional indicator joins the neighbouring grapheme.
    // The inserted byte endpoint is still the caret the user expects.
    state.selection.cursor_index = state
        .buffer
        .character_index_for_utf8_offset(start_byte + insertion.len())
        .min(state.buffer.character_count());
    state.selection.anchor = None;
    state.selection.set_preferred_caret_x(None);
    record_caret_activity(state);
    state.end_edit();
}

/// Move the caret, optionally growing a selection, and remember the preferred
/// caret x for vertical moves.
pub fn move_cursor(
    state: &mut FieldState,
    target: usize,
    extend_selection: bool,
    preferred_x: Option<f32>,
) {
    let count = state.buffer.character_count();
    let bounded = target.min(count);
    if extend_selection {
        if state.selection.anchor.is_none() {
            state.selection.anchor = Some(state.selection.cursor_index);
        }
    } else {
        state.selection.anchor = None;
    }
    state.selection.cursor_index = bounded;
    state.selection.set_preferred_caret_x(preferred_x);
    record_caret_activity(state);
}

/// Clamp persisted indices into the current text and refresh the cached buffer.
///
/// `FieldState` outlives recompose while the bound text can be rewritten
/// underneath it (entity switch, formatter, programmatic set); a stale cursor
/// past the new end must never reach index math.
pub fn normalize_indices(state: &mut FieldState) {
    let current = state.buffer.clone();
    if !state.buffer.has_same_contents(&current) {
        state.selection.line_end_affinity = false;
    }
    let count = state.buffer.character_count();
    state.selection.cursor_index = state.selection.cursor_index.min(count);
    if let Some(anchor) = state.selection.anchor {
        let bounded = anchor.min(count);
        state.selection.anchor = if bounded == state.selection.cursor_index {
            None
        } else {
            Some(bounded)
        };
    }
}

/// Every caret/selection mutation funnels through here: it resets the blink
/// phase, requests a reveal and reports the new caret.
pub fn record_caret_activity(state: &mut FieldState) {
    state.record_activity();
}

/// The caret/selection snapshot for reporting.
pub fn caret_state(state: &FieldState) -> TextFieldCaretState {
    TextFieldCaretState::new(state.selection.cursor_index, state.selection_range())
}

/// The whole buffer as a string, clamped to the character count.
pub fn buffer_text(buffer: &TextBuffer) -> String {
    buffer.substring(0..buffer.character_count())
}
