//! Per-document text transactions. Selection is restored with the text; a new
//! edit after undo discards the red branch. Adjacent typing/deletion coalesces.

use super::text_buffer::TextBuffer;
use cove_core::scene::{ObservableStateRegistrar, StateAccessKey};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub buffer: TextBuffer,
    pub cursor: usize,
    pub anchor: Option<usize>,
}

impl Snapshot {
    pub fn new(buffer: TextBuffer, cursor: usize, anchor: Option<usize>) -> Self {
        Self {
            buffer,
            cursor,
            anchor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditKind {
    Typing,
    Deletion,
    Atomic,
}

#[derive(Clone, Debug)]
struct Transaction {
    before: Snapshot,
    after: Snapshot,
    kind: TextEditKind,
    time: f64,
}

pub struct TextEditHistory {
    registrar: ObservableStateRegistrar,
    undo_stack: VecDeque<Transaction>,
    redo_stack: Vec<Transaction>,
    current_buffer: Option<TextBuffer>,
}

impl Default for TextEditHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEditHistory {
    pub fn new() -> Self {
        Self {
            registrar: ObservableStateRegistrar::new(),
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            current_buffer: None,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.registrar
            .access(StateAccessKey::text("undo"));
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        self.registrar
            .access(StateAccessKey::text("redo"));
        !self.redo_stack.is_empty()
    }

    pub fn current_buffer(&self) -> Option<&TextBuffer> {
        self.current_buffer.as_ref()
    }

    fn notify(&self, undo: bool, redo: bool) {
        if undo != !self.undo_stack.is_empty() {
            self.registrar.invalidate(&StateAccessKey::text("undo"));
        }
        if redo != !self.redo_stack.is_empty() {
            self.registrar.invalidate(&StateAccessKey::text("redo"));
        }
    }

    pub fn synchronize(&mut self, buffer: &TextBuffer) {
        let old_undo = !self.undo_stack.is_empty();
        let old_redo = !self.redo_stack.is_empty();
        if self.current_buffer.as_ref() != Some(buffer) {
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.current_buffer = Some(buffer.clone());
        }
        self.notify(old_undo, old_redo);
    }

    pub fn break_group(&mut self) {
        if let Some(last) = self.undo_stack.back_mut() {
            last.time = f64::NEG_INFINITY;
        }
    }

    pub fn record(&mut self, before: Snapshot, after: Snapshot, kind: TextEditKind, time: f64) {
        let old_undo = !self.undo_stack.is_empty();
        let old_redo = !self.redo_stack.is_empty();
        if before.buffer == after.buffer {
            self.notify(old_undo, old_redo);
            return;
        }
        self.redo_stack.clear();
        let coalesced = self
            .undo_stack
            .back()
            .is_some_and(|last| {
                kind != TextEditKind::Atomic
                    && last.kind == kind
                    && time - last.time < 0.75
                    && last.after == before
                    && before.anchor.is_none()
                    && after.anchor.is_none()
            });
        if coalesced {
            if let Some(last) = self.undo_stack.back_mut() {
                last.after = after.clone();
                last.time = time;
            }
        } else {
            self.undo_stack.push_back(Transaction {
                before,
                after: after.clone(),
                kind,
                time,
            });
            if self.undo_stack.len() > 100 {
                self.undo_stack.pop_front();
            }
        }
        self.current_buffer = Some(after.buffer);
        self.notify(old_undo, old_redo);
    }

    pub fn undo(&mut self) -> Option<Snapshot> {
        let old_undo = !self.undo_stack.is_empty();
        let old_redo = !self.redo_stack.is_empty();
        let Some(edit) = self.undo_stack.pop_back() else {
            self.notify(old_undo, old_redo);
            return None;
        };
        self.current_buffer = Some(edit.before.buffer.clone());
        self.redo_stack.push(edit.clone());
        self.break_group();
        self.notify(old_undo, old_redo);
        Some(edit.before)
    }

    pub fn redo(&mut self) -> Option<Snapshot> {
        let old_undo = !self.undo_stack.is_empty();
        let old_redo = !self.redo_stack.is_empty();
        let Some(mut edit) = self.redo_stack.pop() else {
            self.notify(old_undo, old_redo);
            return None;
        };
        edit.time = f64::NEG_INFINITY;
        self.current_buffer = Some(edit.after.buffer.clone());
        self.undo_stack.push_back(edit.clone());
        self.notify(old_undo, old_redo);
        Some(edit.after)
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}
