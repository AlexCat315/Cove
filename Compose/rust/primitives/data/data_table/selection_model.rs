//! Translation of `Primitives/Data/DataTable/DataTableSelection.swift` and the
//! navigation request from `DataTable.swift`.
//!
//! Selection is rows plus a focused column: the cell cursor is a row and a
//! column, and the two move independently so a range selection can stay on one
//! column while the keyboard walks down.
//!
//! Behaviour checklist:
//! - a plain click replaces the selection and sets the anchor
//! - Shift extends from the anchor to the clicked row
//! - Cmd/Ctrl toggles one row and moves the anchor
//! - single mode always replaces, whatever the modifiers
//! - `retain_existing` drops ids that left the model

use std::collections::BTreeSet;

use cove_platform::KeyModifiers;

use super::model::DataTableModel;

/// Whether a table selects one row or many.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DataTableSelectionMode {
    Single,
    #[default]
    Multiple,
}

/// Rows that are selected, plus the cell cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataTableSelection<Id: Ord + Clone> {
    pub selected_ids: BTreeSet<Id>,
    pub focused_id: Option<Id>,
    pub anchor_id: Option<Id>,
    pub focused_column_id: Option<String>,
}

impl<Id: Ord + Clone> Default for DataTableSelection<Id> {
    fn default() -> Self {
        Self {
            selected_ids: BTreeSet::new(),
            focused_id: None,
            anchor_id: None,
            focused_column_id: None,
        }
    }
}

impl<Id: Ord + Clone> DataTableSelection<Id> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one click at display `index`.
    ///
    /// Shift extends from the anchor, Cmd/Ctrl toggles, and anything else
    /// replaces — but only in multiple mode. Single mode always replaces, so a
    /// modifier cannot leave a table with two selected rows.
    pub fn select<Record: 'static>(
        &mut self,
        index: usize,
        model: &DataTableModel<Record, Id>,
        mode: DataTableSelectionMode,
        modifiers: KeyModifiers,
    ) where
        Id: Eq + std::hash::Hash,
    {
        if index >= model.count() {
            return;
        }
        let Some(id) = model.row_id_at(index) else { return };
        if mode == DataTableSelectionMode::Multiple && modifiers.has_shift() {
            if let Some(anchor) = self.anchor_id.clone()
                && let Some(anchor_index) = model.index_for(&anchor)
            {
                let (low, high) = (anchor_index.min(index), anchor_index.max(index));
                self.selected_ids = (low..=high)
                    .filter_map(|row| model.row_id_at(row))
                    .collect();
            } else {
                self.selected_ids = [id.clone()].into();
                self.anchor_id = Some(id.clone());
            }
        } else if mode == DataTableSelectionMode::Multiple
            && (modifiers.has_gui() || modifiers.has_ctrl())
        {
            if !self.selected_ids.remove(&id) {
                self.selected_ids.insert(id.clone());
            }
            self.anchor_id = Some(id.clone());
        } else {
            self.selected_ids = [id.clone()].into();
            self.anchor_id = Some(id.clone());
        }
        self.focused_id = Some(id);
    }

    /// Drops ids that are no longer in the model.
    pub fn retain_existing<Record: 'static>(&mut self, model: &DataTableModel<Record, Id>)
    where
        Id: Eq + std::hash::Hash,
    {
        self.selected_ids.retain(|id| model.index_for(id).is_some());
        if self.focused_id.as_ref().is_some_and(|id| model.index_for(id).is_none()) {
            self.focused_id = None;
        }
        if self.anchor_id.as_ref().is_some_and(|id| model.index_for(id).is_none()) {
            self.anchor_id = None;
        }
    }

    /// Selects every row in the model.
    pub fn select_all<Record: 'static>(&mut self, model: &DataTableModel<Record, Id>)
    where
        Id: Eq + std::hash::Hash,
    {
        self.selected_ids = (0..model.count()).filter_map(|row| model.row_id_at(row)).collect();
    }

    pub fn clear(&mut self) {
        self.selected_ids.clear();
        self.focused_id = None;
        self.anchor_id = None;
    }

    pub fn contains(&self, id: &Id) -> bool {
        self.selected_ids.contains(id)
    }
}

/// A request to reveal one cell, keyed by a request identity so the same
/// request is not applied twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataTableNavigation<Id> {
    pub request_id: Option<u64>,
    pub row_id: Option<Id>,
    pub column_id: Option<String>,
}

impl<Id> Default for DataTableNavigation<Id> {
    fn default() -> Self {
        Self { request_id: None, row_id: None, column_id: None }
    }
}

impl<Id> DataTableNavigation<Id> {
    pub fn new() -> Self {
        Self::default()
    }
}
