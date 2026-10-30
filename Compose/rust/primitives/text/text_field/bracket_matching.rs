//! Translation of Primitives/Text/TextField/BracketMatching.swift.
//!
//! Retains only one buffer revision and caret result. Blink and scroll redraws
//! reuse the pair; ordinary text never triggers a lexical-window scan.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{Node, NodeResource};
use crate::primitives::text::text_buffer::TextBuffer;

/// Half of the surrounding lexical window inspected for a match.
const SCAN_RADIUS: usize = 65_536;

/// Bracket pair around the caret, in UTF-8 byte offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BracketPair {
    pub open: usize,
    pub close: usize,
}

/// Cached bracket-pair lookup for one buffer revision and caret position.
#[derive(Default)]
pub struct TextBracketMatching {
    buffer: RefCell<Option<TextBuffer>>,
    caret: RefCell<usize>,
    result: RefCell<Vec<usize>>,
    scanned_utf8_count: RefCell<usize>,
}

impl TextBracketMatching {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// UTF-8 bytes scanned so far; a diagnostics signal for the scan budget.
    pub fn scanned_utf8_count(&self) -> usize {
        *self.scanned_utf8_count.borrow()
    }

    pub fn pair(&self, source: &TextBuffer, caret_index: usize) -> Option<BracketPair> {
        let index = caret_index.min(source.character_count());
        if self
            .buffer
            .borrow()
            .as_ref()
            .is_some_and(|current| current.has_same_contents(source))
            && *self.caret.borrow() == index
        {
            return Self::pair_from(&self.result.borrow());
        }
        *self.buffer.borrow_mut() = Some(source.clone());
        *self.caret.borrow_mut() = index;
        self.result.borrow_mut().clear();

        let byte = source.utf8_offset_for_character_index(index);
        let neighbors = source.utf8_bytes(byte.saturating_sub(1)..(byte + 1).min(source.utf8_length()));
        if !neighbors.iter().any(|value| Self::is_bracket(*value)) {
            return None;
        }
        let start = byte.saturating_sub(SCAN_RADIUS);
        let end = (byte + SCAN_RADIUS).min(source.utf8_length());
        let bytes = source.utf8_bytes(start..end);
        *self.scanned_utf8_count.borrow_mut() += bytes.len();
        let code = Self::code_mask(&bytes);
        let local = byte - start;
        let target = if local < bytes.len() && code[local] && Self::is_bracket(bytes[local]) {
            Some(local)
        } else if local > 0 && code[local - 1] && Self::is_bracket(bytes[local - 1]) {
            Some(local - 1)
        } else {
            None
        };
        let Some(target) = target else { return None };
        let Some(matched) = Self::matching_bracket(&bytes, &code, target) else { return None };
        let mut result = self.result.borrow_mut();
        result.clear();
        result.push(start + target);
        result.push(start + matched);
        Self::pair_from(&result)
    }

    fn pair_from(result: &[usize]) -> Option<BracketPair> {
        match result {
            [open, close, ..] => Some(BracketPair { open: *open, close: *close }),
            _ => None,
        }
    }

    fn is_bracket(byte: u8) -> bool {
        matches!(byte, b'(' | b')' | b'[' | b']' | b'{' | b'}')
    }

    /// `true` where a byte is code rather than comment or string content.
    fn code_mask(bytes: &[u8]) -> Vec<bool> {
        let mut mask = vec![true; bytes.len()];
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
                while index < bytes.len() && bytes[index] != b'\n' {
                    mask[index] = false;
                    index += 1;
                }
                continue;
            }
            if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
                mask[index] = false;
                mask[index + 1] = false;
                index += 2;
                while index < bytes.len() {
                    mask[index] = false;
                    if bytes[index] == b'*' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
                        index += 1;
                        mask[index] = false;
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
            if byte == b'"' {
                mask[index] = false;
                index += 1;
                while index < bytes.len() {
                    mask[index] = false;
                    if bytes[index] == b'\\' {
                        index += 1;
                        if index < bytes.len() {
                            mask[index] = false;
                        }
                    } else if bytes[index] == b'"' {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
            index += 1;
        }
        mask
    }

    fn matching_bracket(bytes: &[u8], code: &[bool], index: usize) -> Option<usize> {
        let byte = *bytes.get(index)?;
        if let Some(close) = Self::closing_for(byte) {
            let mut stack = vec![close];
            let mut cursor = index + 1;
            while cursor < bytes.len() {
                if !code[cursor] {
                    cursor += 1;
                    continue;
                }
                let next = bytes[cursor];
                if let Some(closer) = Self::closing_for(next) {
                    stack.push(closer);
                } else if Self::opening_for(next).is_some() {
                    if stack.last() != Some(&next) {
                        return None;
                    }
                    stack.pop();
                    if stack.is_empty() {
                        return Some(cursor);
                    }
                }
                cursor += 1;
            }
            return None;
        }
        if let Some(open) = Self::opening_for(byte) {
            let mut stack = vec![open];
            let mut cursor = index;
            while cursor > 0 {
                cursor -= 1;
                if !code[cursor] {
                    continue;
                }
                let next = bytes[cursor];
                if let Some(opener) = Self::opening_for(next) {
                    stack.push(opener);
                } else if Self::closing_for(next).is_some() {
                    if stack.last() != Some(&next) {
                        return None;
                    }
                    stack.pop();
                    if stack.is_empty() {
                        return Some(cursor);
                    }
                }
            }
        }
        None
    }

    fn closing_for(byte: u8) -> Option<u8> {
        match byte {
            b'(' => Some(b')'),
            b'[' => Some(b']'),
            b'{' => Some(b'}'),
            _ => None,
        }
    }

    fn opening_for(byte: u8) -> Option<u8> {
        match byte {
            b')' => Some(b'('),
            b']' => Some(b'['),
            b'}' => Some(b'{'),
            _ => None,
        }
    }
}

impl NodeResource for TextBracketMatching {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, _node: &Node) {
        *self.buffer.borrow_mut() = None;
        *self.caret.borrow_mut() = 0;
        self.result.borrow_mut().clear();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
