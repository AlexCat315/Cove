//! Index conversions and range materialization for the rope.

use super::rope_storage::{RopeChunk, RopeHandle, RopeLeaf, RopeNode, RopeStorage};
use std::sync::Arc;

impl RopeNode {
    pub fn utf8_offset_for_character_index(&self, index: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => leaf.character_boundaries[index],
            RopeStorage::Branch(branch) => {
                if index < branch.left.metrics.character_count {
                    branch.left.utf8_offset_for_character_index(index)
                } else {
                    branch.left.metrics.utf8_length
                        + branch
                            .right
                            .utf8_offset_for_character_index(index - branch.left.metrics.character_count)
                }
            }
        }
    }

    pub fn character_index_for_utf8_offset(&self, offset: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => lower_bound(&leaf.character_boundaries, offset),
            RopeStorage::Branch(branch) => {
                if offset < branch.left.metrics.utf8_length {
                    branch.left.character_index_for_utf8_offset(offset)
                } else {
                    branch.left.metrics.character_count
                        + branch
                            .right
                            .character_index_for_utf8_offset(offset - branch.left.metrics.utf8_length)
                }
            }
        }
    }

    pub fn utf16_offset_for_character_index(&self, index: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => {
                let boundary = leaf.character_boundaries[index];
                let prefix =
                    std::str::from_utf8(&leaf.bytes[..boundary]).unwrap_or_default();
                prefix.encode_utf16().count()
            }
            RopeStorage::Branch(branch) => {
                if index < branch.left.metrics.character_count {
                    branch.left.utf16_offset_for_character_index(index)
                } else {
                    branch.left.metrics.utf16_length
                        + branch
                            .right
                            .utf16_offset_for_character_index(index - branch.left.metrics.character_count)
                }
            }
        }
    }

    pub fn newline_character_index(&self, ordinal: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => leaf.newline_characters[ordinal],
            RopeStorage::Branch(branch) => {
                if ordinal < branch.left.metrics.newline_count {
                    branch.left.newline_character_index(ordinal)
                } else {
                    branch.left.metrics.character_count
                        + branch
                            .right
                            .newline_character_index(ordinal - branch.left.metrics.newline_count)
                }
            }
        }
    }

    pub fn newlines_before_character(&self, index: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => lower_bound(&leaf.newline_characters, index),
            RopeStorage::Branch(branch) => {
                if index < branch.left.metrics.character_count {
                    branch.left.newlines_before_character(index)
                } else {
                    branch.left.metrics.newline_count
                        + branch
                            .right
                            .newlines_before_character(index - branch.left.metrics.character_count)
                }
            }
        }
    }

    pub fn line_feeds_before_byte(&self, offset: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => lower_bound(&leaf.line_feed_bytes, offset),
            RopeStorage::Branch(branch) => {
                if offset < branch.left.metrics.utf8_length {
                    branch.left.line_feeds_before_byte(offset)
                } else {
                    branch.left.metrics.line_feed_count
                        + branch
                            .right
                            .line_feeds_before_byte(offset - branch.left.metrics.utf8_length)
                }
            }
        }
    }

    pub fn line_feed_byte_index(&self, ordinal: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => leaf.line_feed_bytes[ordinal],
            RopeStorage::Branch(branch) => {
                if ordinal < branch.left.metrics.line_feed_count {
                    branch.left.line_feed_byte_index(ordinal)
                } else {
                    branch.left.metrics.utf8_length
                        + branch
                            .right
                            .line_feed_byte_index(ordinal - branch.left.metrics.line_feed_count)
                }
            }
        }
    }

    pub fn utf8_offset_for_utf16_offset(&self, offset: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => {
                let mut byte = 0usize;
                let mut code_unit = 0usize;
                for scalar in leaf.text.chars() {
                    if code_unit >= offset {
                        return byte;
                    }
                    byte += scalar.len_utf8();
                    code_unit += scalar.len_utf16();
                }
                byte
            }
            RopeStorage::Branch(branch) => {
                if offset < branch.left.metrics.utf16_length {
                    branch.left.utf8_offset_for_utf16_offset(offset)
                } else {
                    branch.left.metrics.utf8_length
                        + branch
                            .right
                            .utf8_offset_for_utf16_offset(offset - branch.left.metrics.utf16_length)
                }
            }
        }
    }

    pub fn utf16_offset_for_utf8_offset(&self, offset: usize) -> usize {
        match &self.storage {
            RopeStorage::Leaf(leaf) => {
                let mut byte = 0usize;
                let mut code_unit = 0usize;
                for scalar in leaf.text.chars() {
                    if byte >= offset {
                        return code_unit;
                    }
                    byte += scalar.len_utf8();
                    code_unit += scalar.len_utf16();
                }
                code_unit
            }
            RopeStorage::Branch(branch) => {
                if offset < branch.left.metrics.utf8_length {
                    branch.left.utf16_offset_for_utf8_offset(offset)
                } else {
                    branch.left.metrics.utf16_length
                        + branch
                            .right
                            .utf16_offset_for_utf8_offset(offset - branch.left.metrics.utf8_length)
                }
            }
        }
    }

    pub fn leaves_in(&self, range: std::ops::Range<usize>) -> Vec<Arc<RopeLeaf>> {
        if range.is_empty() {
            return Vec::new();
        }
        match &self.storage {
            RopeStorage::Leaf(leaf) => vec![leaf.clone()],
            RopeStorage::Branch(branch) => {
                let split = branch.left.metrics.utf8_length;
                let mut result: Vec<Arc<RopeLeaf>> = Vec::new();
                if range.start < split {
                    result.extend(
                        branch
                            .left
                            .leaves_in(range.start..split.min(range.end)),
                    );
                }
                if range.end > split {
                    result.extend(
                        branch
                            .right
                            .leaves_in(range.start.saturating_sub(split)..(range.end - split)),
                    );
                }
                result
            }
        }
    }

    pub fn append_bytes_in(&self, range: std::ops::Range<usize>, result: &mut Vec<u8>) {
        if range.is_empty() {
            return;
        }
        match &self.storage {
            RopeStorage::Leaf(leaf) => result.extend_from_slice(&leaf.bytes[range]),
            RopeStorage::Branch(branch) => {
                let split = branch.left.metrics.utf8_length;
                if range.start < split {
                    branch
                        .left
                        .append_bytes_in(range.start..split.min(range.end), result);
                }
                if range.end > split {
                    branch.right.append_bytes_in(
                        range.start.saturating_sub(split)..(range.end - split),
                        result,
                    );
                }
            }
        }
    }

    pub fn chunk_at(&self, offset: usize, base: usize) -> RopeChunk {
        match &self.storage {
            RopeStorage::Leaf(leaf) => RopeChunk {
                leaf: leaf.clone(),
                utf8_offset: base,
                start_index: offset,
            },
            RopeStorage::Branch(branch) => {
                let split = branch.left.metrics.utf8_length;
                if offset < split {
                    branch.left.chunk_at(offset, base)
                } else {
                    branch.right.chunk_at(offset - split, base + split)
                }
            }
        }
    }

    /// Collects the entire subtree text; used by differences and tests.
    pub fn collect_text(&self, result: &mut String) {
        match &self.storage {
            RopeStorage::Leaf(leaf) => result.push_str(&leaf.text),
            RopeStorage::Branch(branch) => {
                branch.left.collect_text(result);
                branch.right.collect_text(result);
            }
        }
    }

    pub fn to_string_value(&self) -> String {
        let mut text = String::with_capacity(self.metrics.utf8_length);
        self.collect_text(&mut text);
        text
    }
}

pub fn lower_bound(array: &[usize], target: usize) -> usize {
    let mut lower = 0usize;
    let mut upper = array.len();
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if array[middle] < target {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    lower
}

/// Convenience for callers that hold a handle.
pub fn leaves_in(root: &RopeHandle, range: std::ops::Range<usize>) -> Vec<Arc<RopeLeaf>> {
    root.leaves_in(range)
}
