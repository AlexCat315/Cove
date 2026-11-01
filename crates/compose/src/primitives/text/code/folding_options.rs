//! Translation of Primitives/Text/Code/FoldingOptions.swift.
//!
//! Language services supply logical regions; the field owns only presentation.
//! Omit collapsed lines for a node-local session, or bind them to caller-owned
//! state.
use std::collections::BTreeSet;

use crate::primitives::text::text_buffer::TextBuffer;
use cove_core::Binding;

/// Retains the header line and hides the following lines through the closing
/// line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextFoldRegion {
    pub first_line: usize,
    pub last_line: usize,
}

impl TextFoldRegion {
    pub fn new(first_line: usize, last_line: usize) -> Self {
        Self {
            first_line,
            last_line,
        }
    }

    pub fn contains(&self, line: usize) -> bool {
        line >= self.first_line && line <= self.last_line
    }
}

pub struct TextFoldingLabels {
    pub collapse: Box<dyn Fn(usize) -> String>,
    pub expand: Box<dyn Fn(usize) -> String>,
    pub summary: Box<dyn Fn(usize) -> String>,
    pub hidden_breakpoints: Box<dyn Fn(usize) -> String>,
}

impl Default for TextFoldingLabels {
    fn default() -> Self {
        Self {
            collapse: Box::new(|line| format!("Collapse code at line {}", line + 1)),
            expand: Box::new(|line| format!("Expand code at line {}", line + 1)),
            summary: Box::new(|count| format!("{count} folded lines")),
            hidden_breakpoints: Box::new(|count| format!("{count} breakpoints in folded code")),
        }
    }
}

/// Authored folding configuration for a code field.
#[derive(Clone, Default)]
pub struct TextEditorFolding {
    pub regions: Vec<TextFoldRegion>,
    /// `false` while an asynchronous language service is producing its
    /// snapshot. Pending ranges must not erase caller-owned collapsed headers on
    /// remount.
    pub is_ready: bool,
    pub collapsed_lines: Option<Binding<BTreeSet<usize>>>,
}

impl TextEditorFolding {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            is_ready: true,
            collapsed_lines: None,
        }
    }
}

/// Line mapping produced by one edit, used to rebase authored line numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextLineRebase {
    first: usize,
    last: usize,
    new_last: usize,
    starts_at_line_start: bool,
}

impl TextLineRebase {
    pub fn new(previous: &TextBuffer, next: &TextBuffer) -> Option<Self> {
        let edit = next.edit_delta(previous)?;
        let first = previous.line_index_for_character_index(edit.character_range.start);
        let last = previous.line_index_for_character_index(edit.character_range.end);
        let new_last = next.line_index_for_character_index(
            next.character_index_for_utf8_offset(edit.new_end_utf8_offset),
        );
        let starts_at_line_start = edit.character_range.start == previous.line_range(first).start;
        Some(Self {
            first,
            last,
            new_last,
            starts_at_line_start,
        })
    }

    /// New index for an old line, or `None` when the line was deleted.
    pub fn line(&self, line: usize) -> Option<usize> {
        if line < self.first {
            return Some(line);
        }
        if line == self.first {
            if !self.starts_at_line_start {
                return Some(line);
            }
            return if self.first == self.last {
                Some(self.new_last)
            } else {
                None
            };
        }
        if line < self.last {
            return None;
        }
        Some(line + self.new_last - self.last)
    }
}
