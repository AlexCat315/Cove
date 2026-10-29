//! A provider reply belongs to one document revision. Retain it only while
//! typing or deleting at the end of the same identifier; every edit range is
//! moved through UTF-8 so grapheme joins cannot leave stale character indexes.
//!
//! The completion request/item models live in `Primitives/Controls/
//! CompletionPopover.swift`; their Rust counterparts are not present yet, so
//! the fields this rebase consumes are mirrored here.

use super::text_buffer::{TextBuffer, TextEditDelta};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextInsertFormat {
    #[default]
    PlainText,
    Snippet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextCompletionRequest {
    pub buffer: TextBuffer,
    pub caret_index: usize,
    pub prefix: String,
    pub prefix_range: Range<usize>,
}

impl TextCompletionRequest {
    pub fn new(buffer: TextBuffer, caret_index: usize) -> Self {
        let caret_index = caret_index.min(buffer.character_count());
        let mut start = caret_index;
        while start > 0 {
            let Some(character) = buffer.character_at(start - 1) else {
                break;
            };
            if character.is_alphanumeric() || character == '_' {
                start -= 1;
            } else {
                break;
            }
        }
        let prefix_range = start..caret_index;
        let prefix = buffer.substring(prefix_range.clone());
        Self {
            buffer,
            caret_index,
            prefix,
            prefix_range,
        }
    }

    pub fn utf8_offset(&self) -> usize {
        self.buffer
            .utf8_offset_for_character_index(self.caret_index)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextCompletionEdit {
    pub range: Range<usize>,
    pub text: String,
}

impl TextCompletionEdit {
    pub fn new(range: Range<usize>, text: impl Into<String>) -> Self {
        Self {
            range,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextCompletionItem {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: String,
    pub filter_text: Option<String>,
    pub replacement: Option<Range<usize>>,
    pub additional_edits: Vec<TextCompletionEdit>,
    pub insert_text_format: TextInsertFormat,
}

impl TextCompletionItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, insert_text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            documentation: None,
            insert_text: insert_text.into(),
            filter_text: None,
            replacement: None,
            additional_edits: Vec::new(),
            insert_text_format: TextInsertFormat::PlainText,
        }
    }
}

pub struct TextCompletionRebase;

impl TextCompletionRebase {
    pub fn candidates(
        items: &[TextCompletionItem],
        old: &TextCompletionRequest,
        next: &TextCompletionRequest,
    ) -> Vec<TextCompletionItem> {
        let Some(delta) = next.buffer.edit_delta(&old.buffer) else {
            return Vec::new();
        };
        if old.prefix_range.start != next.prefix_range.start
            || delta.character_range.start < old.prefix_range.start
            || delta.character_range.end > old.prefix_range.end
            || delta.old_end_utf8_offset != old.utf8_offset()
            || delta.new_end_utf8_offset != next.utf8_offset()
        {
            return Vec::new();
        }
        let mut result = Vec::new();
        for item in items {
            let mut item = item.clone();
            if let Some(range) = item.replacement.clone() {
                if range.start > old.prefix_range.start || range.end < old.caret_index {
                    continue;
                }
                let Some(moved) =
                    Self::move_range(&range, old, next, &delta, true)
                else {
                    continue;
                };
                item.replacement = Some(moved);
            }
            let mut edits = Vec::new();
            let mut failed = false;
            for edit in &item.additional_edits {
                let Some(range) =
                    Self::move_range(&edit.range, old, next, &delta, false)
                else {
                    failed = true;
                    break;
                };
                edits.push(TextCompletionEdit::new(range, edit.text.clone()));
            }
            if failed {
                continue;
            }
            item.additional_edits = edits;
            result.push(item);
        }
        result
    }

    fn move_range(
        range: &Range<usize>,
        old: &TextCompletionRequest,
        next: &TextCompletionRequest,
        delta: &TextEditDelta,
        includes_change: bool,
    ) -> Option<Range<usize>> {
        if range.end > old.buffer.character_count() {
            return None;
        }
        let mut start = old.buffer.utf8_offset_for_character_index(range.start) as i64;
        let mut end = old.buffer.utf8_offset_for_character_index(range.end) as i64;
        let shift = delta.new_end_utf8_offset as i64 - delta.old_end_utf8_offset as i64;
        if includes_change {
            end += shift;
        } else if end <= delta.start_utf8_offset as i64 {
            // Edits before the prefix keep their anchor.
        } else if start >= delta.old_end_utf8_offset as i64 {
            start += shift;
            end += shift;
        } else {
            return None;
        }
        if start < 0 || end < start || end > next.buffer.utf8_length() as i64 {
            return None;
        }
        let (start, end) = (start as usize, end as usize);
        let lower = next.buffer.character_index_for_utf8_offset(start);
        let upper = next.buffer.character_index_for_utf8_offset(end);
        if next.buffer.utf8_offset_for_character_index(lower) != start
            || next.buffer.utf8_offset_for_character_index(upper) != end
        {
            return None;
        }
        Some(lower..upper)
    }
}
