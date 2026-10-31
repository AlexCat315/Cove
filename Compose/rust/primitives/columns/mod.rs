//! Multi-column visibility policy shared by table-like views.
use std::collections::{HashMap, HashSet};

pub mod picker;
pub use picker::{ColumnPickerEntry, ColumnVisibilityPickerModel};

/// Required columns are never removed by width fitting or user overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnVisibility {
    Required,
    Automatic { priority: i32 },
}

impl Default for ColumnVisibility {
    fn default() -> Self {
        Self::Automatic { priority: 0 }
    }
}

impl ColumnVisibility {
    pub fn priority(self) -> i32 {
        match self {
            Self::Required => i32::MAX,
            Self::Automatic { priority } => priority,
        }
    }
}

/// The stable identity and width inputs used by the visibility planner.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnVisibilityTrack {
    pub id: String,
    pub title: String,
    pub visibility: ColumnVisibility,
    pub width: f32,
}

impl ColumnVisibilityTrack {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        visibility: ColumnVisibility,
        width: f32,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            visibility,
            width,
        }
    }
}

/// Column indices to render and identities protected by the required policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnVisibilityPlan {
    pub indices: Vec<usize>,
    pub required_ids: HashSet<String>,
}

impl ColumnVisibilityPlan {
    pub fn resolve(
        columns: &[ColumnVisibilityTrack],
        overrides: &HashMap<String, bool>,
        available_width: f32,
        spacing: f32,
        required_ids: &HashSet<String>,
    ) -> Self {
        let protected: HashSet<usize> = columns
            .iter()
            .enumerate()
            .filter_map(|(index, column)| {
                (column.visibility == ColumnVisibility::Required
                    || required_ids.contains(&column.id))
                .then_some(index)
            })
            .collect();
        let mut required = protected.clone();
        let mut shown: HashSet<usize> = columns
            .iter()
            .enumerate()
            .filter_map(|(index, column)| {
                (required.contains(&index) || overrides.get(&column.id) != Some(&false))
                    .then_some(index)
            })
            .collect();
        for (index, column) in columns.iter().enumerate() {
            if overrides.get(&column.id) == Some(&true) {
                required.insert(index);
            }
        }

        let mut candidates: Vec<usize> = columns
            .iter()
            .enumerate()
            .filter_map(|(index, column)| {
                (!required.contains(&index) && !overrides.contains_key(&column.id)).then_some(index)
            })
            .collect();
        candidates.sort_by(|lhs, rhs| {
            columns[*lhs]
                .visibility
                .priority()
                .cmp(&columns[*rhs].visibility.priority())
                .then_with(|| rhs.cmp(lhs))
        });

        if required.is_empty() {
            let anchor = shown
                .iter()
                .copied()
                .min_by(|lhs, rhs| {
                    columns[*rhs]
                        .visibility
                        .priority()
                        .cmp(&columns[*lhs].visibility.priority())
                        .then_with(|| lhs.cmp(rhs))
                })
                .or_else(|| (!columns.is_empty()).then_some(0));
            if let Some(anchor) = anchor {
                required.insert(anchor);
                shown.insert(anchor);
            }
        }

        let mut width: f32 = shown.iter().map(|index| columns[*index].width).sum();
        width += spacing * shown.len().saturating_sub(1) as f32;
        for index in candidates {
            if width <= available_width.max(0.0) {
                break;
            }
            if required.contains(&index) || !shown.remove(&index) {
                continue;
            }
            width -= columns[index].width;
            if !shown.is_empty() {
                width -= spacing;
            }
        }

        Self {
            indices: columns
                .iter()
                .enumerate()
                .filter_map(|(index, _)| shown.contains(&index).then_some(index))
                .collect(),
            required_ids: protected
                .into_iter()
                .map(|index| columns[index].id.clone())
                .collect(),
        }
    }
}
