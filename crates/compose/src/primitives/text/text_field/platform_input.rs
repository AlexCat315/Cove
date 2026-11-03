//! Translation of Primitives/Text/TextField/PlatformInput.swift.
//!
//! Builds the `TextInputClient` that lets a platform input method drive the field:
//! snapshot in UTF-16 units, selection by UTF-16 range, and delete-surrounding.
//! While an IME composition is live the snapshot reports the preview buffer with
//! the marked range, and `select` moves the composition window instead of
//! replacing text.
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

use crate::primitives::text::text_buffer::TextBuffer;
use cove_core::scene::TextInputClient;

use super::text_field::FieldState;
use super::text_field_options::{TextFieldAxis, TextFieldBehavior};
use crate::primitives::text::text_edit_history::TextEditKind;

/// UTF-16 offset of a character index, clamped so a stale index cannot run past
/// the buffer.
pub fn utf16_offset(buffer: &TextBuffer, character: usize) -> i32 {
    i32::try_from(buffer.utf16_offset_for_character_index(character)).unwrap_or(i32::MAX)
}

/// Grapheme index at a user-perceived character boundary for a UTF-16 offset,
/// matching Swift's `character(_:roundUp:)` walk over `String.enumerated()`.
/// Native selections must not split a grapheme.
pub fn character_index(buffer: &TextBuffer, offset: i32, round_up: bool) -> usize {
    let target = offset.max(0) as usize;
    let byte = buffer.utf8_offset_for_utf16_offset(target);
    let index = buffer.character_index_for_utf8_offset(byte);
    if !round_up && buffer.utf16_offset_for_character_index(index) > target {
        index.saturating_sub(1)
    } else {
        index
    }
}

/// Build the platform input client for one field and its editing state.
pub fn platform_input_client(
    state: Rc<std::cell::RefCell<FieldState>>,
    behavior: TextFieldBehavior,
    axis: TextFieldAxis,
) -> TextInputClient {
    let snapshot_state = state.clone();
    let select_state = state.clone();
    let delete_state = state;
    TextInputClient::new(
        move || {
            let state = snapshot_state.borrow();
            let buffer = &state.buffer;
            let composition = state.composition.clone();
            let (low, high) = state
                .selection_range()
                .unwrap_or((state.selection.cursor_index, state.selection.cursor_index));
            let mut snapshot = cove_core::scene::TextInputSnapshot::new(
                buffer.string_value(),
                utf16_offset(buffer, low),
                utf16_offset(buffer, high),
            );
            snapshot.is_secure = behavior.secure;
            snapshot.is_read_only = behavior.read_only;
            snapshot.is_multiline = axis == TextFieldAxis::Vertical;
            if composition.is_active() {
                // The preview replaces the selected range with the marked text, so
                // the platform sees what it is composing over.
                let preview = buffer.replace(low..high, &composition.text);
                let base = utf16_offset(buffer, low);
                snapshot.text = preview.string_value();
                let caret = base
                    + composition
                        .text
                        .graphemes(true)
                        .take(composition.start + composition.length)
                        .map(|grapheme| grapheme.encode_utf16().count())
                        .sum::<usize>() as i32;
                snapshot.selection_start = caret;
                snapshot.selection_end = caret;
                snapshot.composition_start = base;
                snapshot.composition_end = base + composition.text.encode_utf16().count() as i32;
            }
            snapshot
        },
        move |start: i32, end: i32| {
            let mut state = select_state.borrow_mut();
            if state.composition.is_active() {
                // A selection inside the marked range moves the composition
                // window instead of replacing the text, which is what IMEs expect
                // when they re-place the caret mid-composition.
                let buffer = state.buffer.clone();
                let (low, _) = state
                    .selection_range()
                    .unwrap_or((state.selection.cursor_index, state.selection.cursor_index));
                let base = utf16_offset(&buffer, low);
                let pending = state.composition.text.clone();
                let limit = base + pending.encode_utf16().count() as i32;
                if start >= base && end >= base && start <= limit && end <= limit {
                    let position = |units: i32| -> usize {
                        let mut consumed = 0i32;
                        for (index, grapheme) in pending.graphemes(true).enumerate() {
                            consumed += grapheme.encode_utf16().count() as i32;
                            if units < consumed {
                                return index;
                            }
                        }
                        pending.graphemes(true).count()
                    };
                    let lower = position(start.min(end) - base);
                    state.composition.start = lower;
                    state.composition.length = position(start.max(end) - base) - lower;
                    return;
                }
                // Outside the marked range the composition commits first, as
                // one undo step and clipped to the authored length limit.
                super::text_field_editing::insert_replacing_selection_limited(
                    &mut state,
                    &pending,
                    behavior.max_length,
                );
                state.clear_composition();
            }
            let buffer = state.buffer.clone();
            let low_index = character_index(&buffer, start.max(0), false);
            let high_index = character_index(&buffer, end.max(0), false);
            state.selection.anchor = (low_index != high_index).then_some(low_index);
            state.selection.cursor_index = high_index;
            state.record_activity();
        },
        move |before: i32, after: i32| {
            if behavior.read_only || behavior.disabled {
                return;
            }
            let mut state = delete_state.borrow_mut();
            state.clear_composition();
            let buffer = state.buffer.clone();
            let (low, high) = state
                .selection_range()
                .unwrap_or((state.selection.cursor_index, state.selection.cursor_index));
            let start_units = utf16_offset(&buffer, low) - before.max(0);
            let end_units = utf16_offset(&buffer, high) + after.max(0);
            let start_index = character_index(&buffer, start_units.max(0), false);
            let end_index = character_index(&buffer, end_units, true).max(start_index);
            if end_index <= start_index {
                return;
            }
            state.begin_edit(&buffer, TextEditKind::Deletion);
            state.buffer = buffer.delete(start_index..end_index);
            state.selection.anchor = None;
            state.selection.cursor_index = start_index;
            state.record_activity();
            state.end_edit();
        },
    )
}
