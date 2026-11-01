//! State and menu entries for the column visibility picker.
//! The adaptive menu presentation is pending the Rust menu/overlay controls.
use super::{ColumnVisibilityPlan, ColumnVisibilityTrack};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnPickerEntry {
    Automatic {
        is_enabled: bool,
    },
    Separator,
    Column {
        id: String,
        title: String,
        is_enabled: bool,
        is_selected: bool,
    },
}

/// UI-independent state consumed by the Rust menu host once it is available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnVisibilityPickerModel {
    pub entries: Vec<ColumnPickerEntry>,
    pub summary: String,
    pub is_enabled: bool,
    pub is_expanded: bool,
}

impl ColumnVisibilityPickerModel {
    pub fn new(
        columns: &[ColumnVisibilityTrack],
        plan: &ColumnVisibilityPlan,
        overrides: &HashMap<String, bool>,
        is_enabled: bool,
        is_expanded: bool,
    ) -> Self {
        let visible: HashSet<&str> = plan
            .indices
            .iter()
            .filter_map(|index| columns.get(*index).map(|column| column.id.as_str()))
            .collect();
        let mut entries = vec![ColumnPickerEntry::Automatic {
            is_enabled: !overrides.is_empty(),
        }];
        entries.push(ColumnPickerEntry::Separator);
        entries.extend(columns.iter().map(|column| {
            let selected = visible.contains(column.id.as_str());
            let required =
                plan.required_ids.contains(&column.id) || (visible.len() == 1 && selected);
            ColumnPickerEntry::Column {
                id: column.id.clone(),
                title: column.title.clone(),
                is_enabled: !required,
                is_selected: selected,
            }
        }));
        Self {
            entries,
            summary: format!("{} of {} columns", visible.len(), columns.len()),
            is_enabled,
            is_expanded: is_enabled && is_expanded,
        }
    }

    pub fn reset_to_automatic(overrides: &mut HashMap<String, bool>) -> bool {
        if overrides.is_empty() {
            return false;
        }
        overrides.clear();
        true
    }

    pub fn set_column_visible(id: &str, visible: bool, overrides: &mut HashMap<String, bool>) {
        overrides.insert(id.to_owned(), visible);
    }
}
