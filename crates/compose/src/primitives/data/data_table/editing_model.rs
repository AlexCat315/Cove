//! Translation of `Primitives/Data/DataTable/DataTableEditing.swift` (the model
//! half: draft lifecycle, validation and Tab traversal).
//!
//! Behaviour checklist:
//! - beginning an edit on an already-edited cell is a no-op
//! - beginning an edit commits the previous draft first, and aborts if that
//!   commit is rejected
//! - committing validates; a rejected value keeps the draft and records the
//!   error
//! - committing an unchanged value clears the draft without touching the record
//! - Tab moves to the next editable column, skipping non-editable ones, and
//!   Shift+Tab moves backwards
//! - Tab past the last editable column leaves the draft committed

/// One in-progress cell edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataTableCellDraft {
    /// Row identity, not index: the display order can change underneath an edit.
    pub row_id: String,
    pub column_id: String,
    /// The text the editor opened with, used to skip no-op writes.
    pub original: String,
    pub buffer: String,
    /// Validation message while the draft is rejected.
    pub error: Option<String>,
}

/// Outcome of a commit attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    /// Nothing was being edited.
    NoDraft,
    /// The draft was rejected and is still open, with the message recorded.
    Rejected(String),
    /// The draft closed without changing the record.
    Unchanged,
    /// The draft closed and the record should be updated to this value.
    Updated(String),
    /// The row or column disappeared, so the draft was dropped.
    Orphaned,
}

/// State of one editable table.
#[derive(Clone, Debug, Default)]
pub struct DataTableEditingState {
    pub draft: Option<DataTableCellDraft>,
}

impl DataTableEditingState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens an editor, committing (and validating) any previous draft first.
    ///
    /// Returns `false` when the previous draft could not be committed, in which
    /// case the new edit does not start — a rejected cell keeps focus rather than
    /// losing the person's input.
    pub fn begin(
        &mut self,
        row_id: impl Into<String>,
        column_id: impl Into<String>,
        initial: impl Into<String>,
        previous_still_valid: impl FnOnce(&DataTableCellDraft) -> CommitAnswer,
    ) -> bool {
        let (row_id, column_id) = (row_id.into(), column_id.into());
        if let Some(draft) = &self.draft
            && draft.row_id == row_id
            && draft.column_id == column_id
        {
            return false;
        }
        if self.draft.is_some()
            && let CommitOutcome::Rejected(_) = self.commit(previous_still_valid)
        {
            return false;
        }
        let initial = initial.into();
        self.draft = Some(DataTableCellDraft {
            row_id,
            column_id,
            original: initial.clone(),
            buffer: initial,
            error: None,
        });
        true
    }

    /// Validates and closes the draft.
    ///
    /// `answer` is the caller's validation + lookup: it decides whether the row
    /// and column still exist and whether the value is acceptable.
    pub fn commit(
        &mut self,
        answer: impl FnOnce(&DataTableCellDraft) -> CommitAnswer,
    ) -> CommitOutcome {
        let Some(draft) = self.draft.clone() else {
            return CommitOutcome::NoDraft;
        };
        match answer(&draft) {
            CommitAnswer::Orphaned => {
                self.draft = None;
                CommitOutcome::Orphaned
            }
            CommitAnswer::Rejected(message) => {
                if let Some(open) = &mut self.draft {
                    open.error = Some(message.clone());
                }
                CommitOutcome::Rejected(message)
            }
            CommitAnswer::Accept => {
                self.draft = None;
                if draft.buffer == draft.original {
                    CommitOutcome::Unchanged
                } else {
                    CommitOutcome::Updated(draft.buffer)
                }
            }
        }
    }

    /// Whether the draft currently carries a validation error.
    pub fn has_error(&self) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.error.is_some())
    }

    /// The next editable column id from `column_id`, in `direction`, skipping
    /// columns that cannot be edited.
    pub fn next_editable(
        column_id: &str,
        direction: i32,
        columns: &[EditableColumn],
    ) -> Option<String> {
        let start = columns.iter().position(|column| column.id == column_id)?;
        let step: i64 = if direction < 0 { -1 } else { 1 };
        let mut index = start as i64 + step;
        while (0..columns.len() as i64).contains(&index) {
            if columns[index as usize].is_editable {
                return Some(columns[index as usize].id.clone());
            }
            index += step;
        }
        None
    }
}

/// One in-progress cell edit, keyed by the table's own row identity.
///
/// The string-keyed [`DataTableCellDraft`] above is the identity-free form used
/// by the model tests; a mounted table uses this one so a row id never has to
/// round-trip through a `String`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellDraft<Id> {
    pub row_id: Id,
    pub column_id: String,
    pub original: String,
    pub buffer: String,
    pub error: Option<String>,
}

impl<Id: Clone + Eq> CellDraft<Id> {
    /// Whether this draft already owns `(row_id, column_id)`.
    pub fn is_for(&self, row_id: &Id, column_id: &str) -> bool {
        self.row_id == *row_id && self.column_id == column_id
    }

    /// Whether the buffer differs from the text the editor opened with.
    pub fn is_dirty(&self) -> bool {
        self.buffer != self.original
    }
}

/// Id-keyed draft lifecycle for a mounted table.
#[derive(Clone, Debug, Default)]
pub struct CellEditing<Id> {
    pub draft: Option<CellDraft<Id>>,
}

impl<Id: Clone + Eq> CellEditing<Id> {
    pub fn new() -> Self {
        Self { draft: None }
    }

    pub fn is_editing(&self, row_id: &Id, column_id: &str) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.is_for(row_id, column_id))
    }

    pub fn error(&self) -> Option<&str> {
        self.draft.as_ref().and_then(|draft| draft.error.as_deref())
    }

    pub fn cancel(&mut self) {
        self.draft = None;
    }

    /// Opens a draft for one cell, replacing any other.
    pub fn begin(&mut self, row_id: Id, column_id: impl Into<String>, initial: impl Into<String>) {
        let initial = initial.into();
        self.draft = Some(CellDraft {
            row_id,
            column_id: column_id.into(),
            original: initial.clone(),
            buffer: initial,
            error: None,
        });
    }

    pub fn set_buffer(&mut self, value: impl Into<String>) {
        if let Some(draft) = &mut self.draft {
            draft.buffer = value.into();
            draft.error = None;
        }
    }

    /// Rejects the open draft, keeping the buffer the person typed.
    pub fn reject(&mut self, message: impl Into<String>) {
        if let Some(draft) = &mut self.draft {
            draft.error = Some(message.into());
        }
    }

    /// Closes the draft and reports the value to persist, when it changed.
    pub fn accept(&mut self) -> Option<(Id, String, String)> {
        let draft = self.draft.take()?;
        if draft.is_dirty() {
            Some((draft.row_id, draft.column_id, draft.buffer))
        } else {
            None
        }
    }
}

/// One column's editability, for Tab traversal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableColumn {
    pub id: String,
    pub is_editable: bool,
}

/// The caller's answer to a commit request.
pub enum CommitAnswer {
    Accept,
    Rejected(String),
    Orphaned,
}
