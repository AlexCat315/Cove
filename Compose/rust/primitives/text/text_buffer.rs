//! Immutable text with shared rope subtrees. Copying a buffer or retaining an
//! undo snapshot is O(1). Editing visits the search path and affected
//! graphemes. Character indexes are extended grapheme clusters; byte indexes
//! are UTF-8.

use super::rope::rope_difference;
use super::rope::rope_editing;
use super::rope::rope_lookup::lower_bound;
use super::rope::rope_storage::{RopeChunk, RopeHandle, RopeLeaf, RopeNode, empty_node};
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone)]
struct BufferEdit {
    previous_root: RopeHandle,
    delta: TextEditDelta,
}

#[derive(Clone)]
pub struct TextBuffer {
    root: RopeHandle,
    pub version: u64,
    last_edit: Option<BufferEdit>,
}

impl PartialEq for TextBuffer {
    /// Revision equality is O(1). Independently constructed buffers are
    /// distinct revisions even when their serialized strings have identical
    /// contents.
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root, &other.root)
    }
}

impl std::fmt::Debug for TextBuffer {
    /// Revision identity plus length; the rope contents are not walked, which
    /// keeps debug formatting from materializing the document.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TextBuffer")
            .field("version", &self.version)
            .field("utf8_length", &self.root.metrics.utf8_length)
            .finish()
    }
}

impl TextBuffer {
    pub fn empty() -> Self {
        Self {
            root: empty_node(),
            version: 0,
            last_edit: None,
        }
    }

    fn with_root(root: RopeHandle, version: u64, edit: Option<BufferEdit>) -> Self {
        Self {
            root,
            version,
            last_edit: edit,
        }
    }

    pub fn new(string: &str) -> Self {
        Self::with_root(
            RopeNode::build(string),
            if string.is_empty() { 0 } else { 1 },
            None,
        )
    }

    pub fn utf8_length(&self) -> usize {
        self.root.metrics.utf8_length
    }

    pub fn character_count(&self) -> usize {
        self.root.metrics.character_count
    }

    pub fn utf16_length(&self) -> usize {
        self.root.metrics.utf16_length
    }

    /// An empty document has one line; CRLF counts as one separator.
    pub fn line_count(&self) -> usize {
        self.root.metrics.newline_count + 1
    }

    pub fn is_empty(&self) -> bool {
        self.utf8_length() == 0
    }

    /// O(n). Use at file/service serialization boundaries, outside rendering.
    pub fn string_value(&self) -> String {
        self.root.to_string_value()
    }

    /// Exact UTF-8 content equality, skipping shared subtrees. Use at
    /// edit/save boundaries and cache the result; independently loaded text
    /// may cost O(n).
    pub fn has_same_contents(&self, other: &TextBuffer) -> bool {
        if Arc::ptr_eq(&self.root, &other.root) {
            return true;
        }
        if self.utf8_length() != other.utf8_length() {
            return false;
        }
        rope_difference::common_bytes(&self.root, &other.root, false, self.utf8_length())
            == self.utf8_length()
    }

    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.root) as usize
    }

    pub fn substring(&self, character_range: Range<usize>) -> String {
        let range = self.bounded(character_range, self.character_count());
        let start = self.utf8_offset_for_character_index(range.start);
        let end = self.utf8_offset_for_character_index(range.end);
        let bytes = self.utf8_bytes(start..end);
        String::from_utf8(bytes).unwrap_or_default()
    }

    pub fn character_at(&self, index: usize) -> Option<char> {
        if index >= self.character_count() {
            return None;
        }
        self.substring(index..(index + 1)).chars().next()
    }

    pub fn insert(&self, string: &str, at_character_index: usize) -> TextBuffer {
        let cursor = self.character_count().min(at_character_index);
        self.replace(cursor..cursor, string)
    }

    pub fn delete(&self, character_range: Range<usize>) -> TextBuffer {
        self.replace(character_range, "")
    }

    pub fn replace(&self, character_range: Range<usize>, string: &str) -> TextBuffer {
        let range = self.bounded(character_range, self.character_count());
        let old_text = self.substring(range.clone());
        if old_text == string {
            return self.clone();
        }
        let start = self.utf8_offset_for_character_index(range.start);
        let old_end = self.utf8_offset_for_character_index(range.end);
        let (left, rest) = rope_editing::split(self.root.clone(), range.start);
        let (_, right) = rope_editing::split(rest, range.end - range.start);
        let result = rope_editing::concatenate(
            rope_editing::concatenate(left, RopeNode::build(string)),
            right,
        );
        let delta = TextEditDelta {
            character_range: range,
            start_utf8_offset: start,
            old_end_utf8_offset: old_end,
            new_end_utf8_offset: start + string.len(),
            old_text,
            new_text: string.to_owned(),
        };
        TextBuffer::with_root(
            result,
            self.version.wrapping_add(1),
            Some(BufferEdit {
                previous_root: self.root.clone(),
                delta,
            }),
        )
    }

    /// A single edit returns its recorded delta. Multiple edits, undo and redo
    /// find the changed extent by skipping shared subtrees, without a full
    /// diff.
    pub fn edit_delta(&self, previous: &TextBuffer) -> Option<TextEditDelta> {
        if Arc::ptr_eq(&self.root, &previous.root) {
            return None;
        }
        if let Some(last_edit) = &self.last_edit {
            if Arc::ptr_eq(&last_edit.previous_root, &previous.root) {
                return Some(last_edit.delta.clone());
            }
        }
        let limit = previous.utf8_length().min(self.utf8_length());
        let common_start = rope_difference::common_bytes(&previous.root, &self.root, false, limit);
        let common_end =
            rope_difference::common_bytes(&previous.root, &self.root, true, limit - common_start);
        if common_start == self.utf8_length() && self.utf8_length() == previous.utf8_length() {
            return None;
        }
        let start = previous
            .character_boundary_before_or_at_utf8(common_start)
            .min(self.character_boundary_before_or_at_utf8(common_start));
        let mut trailing = common_end;
        // Both source ranges end on grapheme boundaries. Enlarging one extent
        // also enlarges the other so replaying the delta preserves the suffix.
        loop {
            let old_end = previous.utf8_offset_for_character_index(
                previous.character_index_for_utf8_offset(previous.utf8_length() - trailing),
            );
            let new_end = self.utf8_offset_for_character_index(
                self.character_index_for_utf8_offset(self.utf8_length() - trailing),
            );
            let next = (previous.utf8_length() - old_end).min(self.utf8_length() - new_end);
            if next == trailing {
                break;
            }
            trailing = next;
        }
        let old_end = previous.utf8_length() - trailing;
        let new_end = self.utf8_length() - trailing;
        let range = previous.character_index_for_utf8_offset(start)
            ..previous.character_index_for_utf8_offset(old_end);
        Some(TextEditDelta {
            character_range: range,
            start_utf8_offset: start,
            old_end_utf8_offset: old_end,
            new_end_utf8_offset: new_end,
            old_text: String::from_utf8(previous.utf8_bytes(start..old_end)).unwrap_or_default(),
            new_text: String::from_utf8(self.utf8_bytes(start..new_end)).unwrap_or_default(),
        })
    }

    fn character_boundary_before_or_at_utf8(&self, offset: usize) -> usize {
        let index = self.character_index_for_utf8_offset(offset);
        let boundary = self.utf8_offset_for_character_index(index);
        if boundary > offset {
            self.utf8_offset_for_character_index(index.saturating_sub(1))
        } else {
            boundary
        }
    }

    pub fn utf8_offset_for_character_index(&self, index: usize) -> usize {
        self.root
            .utf8_offset_for_character_index(index.min(self.character_count()))
    }

    /// Offsets inside a grapheme snap forward to its end.
    pub fn character_index_for_utf8_offset(&self, offset: usize) -> usize {
        self.root
            .character_index_for_utf8_offset(offset.min(self.utf8_length()))
    }

    pub fn utf16_offset_for_character_index(&self, index: usize) -> usize {
        self.root
            .utf16_offset_for_character_index(index.min(self.character_count()))
    }

    /// Range excludes the separator; out-of-range lines clamp to the document.
    pub fn line_range(&self, line: usize) -> Range<usize> {
        let row = (self.line_count() - 1).min(line);
        let start = if row == 0 {
            0
        } else {
            self.root.newline_character_index(row - 1) + 1
        };
        let end = if row == self.line_count() - 1 {
            self.character_count()
        } else {
            self.root.newline_character_index(row)
        };
        start..end
    }

    pub fn character_index_for_line(&self, line: usize, character: usize) -> usize {
        let range = self.line_range(line);
        range.start + (range.end - range.start).min(character)
    }

    pub fn line_index_for_character_index(&self, index: usize) -> usize {
        self.root
            .newlines_before_character(index.min(self.character_count()))
    }

    pub fn line_and_column(&self, index: usize) -> (usize, usize) {
        let cursor = index.min(self.character_count());
        let line = self.line_index_for_character_index(index);
        (line, cursor - self.line_range(line).start)
    }

    pub fn point_for_utf8_offset(&self, offset: usize) -> TextBufferPoint {
        let byte = offset.min(self.utf8_length());
        let forward = self.character_index_for_utf8_offset(byte);
        let character = if self.utf8_offset_for_character_index(forward) > byte {
            forward - 1
        } else {
            forward
        };
        let row = self.line_index_for_character_index(character);
        TextBufferPoint {
            row,
            column_utf8: byte - self.utf8_offset_for_character_index(self.line_range(row).start),
        }
    }

    /// Tree-sitter rows count LF bytes; a CRLF edit can end between its two
    /// bytes, independently of the grapheme/visual-line coordinate space.
    pub fn parser_point(&self, offset: usize) -> TextBufferPoint {
        let byte = offset.min(self.utf8_length());
        let row = self.root.line_feeds_before_byte(byte);
        let start = if row == 0 {
            0
        } else {
            self.root.line_feed_byte_index(row - 1) + 1
        };
        TextBufferPoint {
            row,
            column_utf8: byte - start,
        }
    }

    /// UTF-16 code-unit conversion for LSP. Scalar interiors snap forward;
    /// combining-scalar boundaries remain representable within a character.
    pub fn utf8_offset_for_utf16_offset(&self, offset: usize) -> usize {
        self.root
            .utf8_offset_for_utf16_offset(offset.min(self.utf16_length()))
    }

    pub fn utf16_offset_for_utf8_offset(&self, offset: usize) -> usize {
        self.root
            .utf16_offset_for_utf8_offset(offset.min(self.utf8_length()))
    }

    /// Overlapping immutable leaves, including partially covered edge leaves.
    pub fn visible_chunks(&self, range: Range<usize>) -> Vec<Arc<RopeLeaf>> {
        self.root.leaves_in(self.bounded(range, self.utf8_length()))
    }

    pub fn utf8_bytes(&self, range: Range<usize>) -> Vec<u8> {
        let range = self.bounded(range, self.utf8_length());
        let mut bytes = Vec::with_capacity(range.end - range.start);
        self.root.append_bytes_in(range, &mut bytes);
        bytes
    }

    pub fn chunk_at_utf8_offset(&self, offset: usize) -> Option<RopeChunk> {
        if offset >= self.utf8_length() {
            return None;
        }
        Some(self.root.chunk_at(offset, 0))
    }

    pub fn visible_text(&self, first_line: usize, last_line: usize) -> String {
        let first = (self.line_count() - 1).min(first_line);
        let last = (self.line_count() - 1).min(first.max(last_line));
        self.substring(self.line_range(first).start..self.line_range(last).end)
    }

    fn bounded(&self, range: Range<usize>, count: usize) -> Range<usize> {
        let lower = count.min(range.start);
        lower..count.min(lower.max(range.end))
    }

    pub fn lower_bound_boundaries(boundaries: &[usize], target: usize) -> usize {
        lower_bound(boundaries, target)
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<&str> for TextBuffer {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TextBuffer {
    fn from(value: String) -> Self {
        Self::new(&value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextBufferPoint {
    pub row: usize,
    pub column_utf8: usize,
}

/// UTF-8 spans feed Tree-sitter. LSP ranges must be converted to UTF-16
/// positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditDelta {
    pub character_range: Range<usize>,
    pub start_utf8_offset: usize,
    pub old_end_utf8_offset: usize,
    pub new_end_utf8_offset: usize,
    pub old_text: String,
    pub new_text: String,
}

impl TextEditDelta {
    pub fn is_insertion(&self) -> bool {
        self.character_range.is_empty() && !self.new_text.is_empty()
    }

    pub fn is_deletion(&self) -> bool {
        !self.character_range.is_empty() && self.new_text.is_empty()
    }
}
