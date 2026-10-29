//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Grid/NamedColumns.swift.
//!
//! Named columns are resolved through the shared column-visibility plan
//! defined by the Columns primitives (`Primitives/Columns/Priority.swift`).
//! That module has no Rust counterpart yet, so the plan is ported here with
//! identical priority, override and width-budget semantics.
use std::collections::HashSet;

use super::columns::{ColumnVisibility, GridColumn, GridTrackResolver};

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnVisibilityTrack {
    pub id: String,
    pub title: String,
    pub visibility: ColumnVisibility,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnVisibilityPlan {
    pub indices: Vec<usize>,
    pub required_ids: HashSet<String>,
}
impl ColumnVisibilityPlan {
    pub fn resolve(columns: &[ColumnVisibilityTrack], overrides: &std::collections::HashMap<String, bool>,
                   available_width: f32, spacing: f32, required_ids: &HashSet<String>) -> Self {
        let protected: HashSet<usize> = columns.iter().enumerate()
            .filter(|(_, column)| column.visibility == ColumnVisibility::Required || required_ids.contains(&column.id))
            .map(|(index, _)| index)
            .collect();
        let mut required = protected.clone();
        required.extend(columns.iter().enumerate()
            .filter(|(_, column)| overrides.get(&column.id) == Some(&true))
            .map(|(index, _)| index));
        let mut shown: HashSet<usize> = columns.iter().enumerate()
            .filter(|(index, column)| required.contains(index) || overrides.get(&column.id) != Some(&false))
            .map(|(index, _)| index)
            .collect();
        let mut candidates: Vec<usize> = columns.iter().enumerate()
            .filter(|(index, column)| !required.contains(index) && !overrides.contains_key(&column.id))
            .map(|(index, _)| index)
            .collect();
        candidates.sort_by(|&lhs, &rhs| {
            let left = columns[lhs].visibility.priority();
            let right = columns[rhs].visibility.priority();
            if left == right { rhs.cmp(&lhs) } else { left.cmp(&right) }
        });
        if required.is_empty() {
            let anchor = shown.iter().copied().min_by(|&lhs, &rhs| {
                let left = columns[lhs].visibility.priority();
                let right = columns[rhs].visibility.priority();
                if left == right { lhs.cmp(&rhs) } else { right.cmp(&left) }
            }).or_else(|| if columns.is_empty() { None } else { Some(0) });
            if let Some(anchor) = anchor {
                required.insert(anchor);
                shown.insert(anchor);
            }
        }
        let mut width: f32 = shown.iter().map(|&index| columns[index].width).sum::<f32>()
            + spacing * shown.len().saturating_sub(1) as f32;
        for index in candidates {
            if width > available_width.max(0.0) && !required.contains(&index) {
                shown.remove(&index);
                width -= columns[index].width + if shown.is_empty() { 0.0 } else { spacing };
            }
        }
        Self {
            indices: (0..columns.len()).filter(|index| shown.contains(index)).collect(),
            required_ids: protected.iter().map(|&index| columns[index].id.clone()).collect(),
        }
    }
}

pub struct GridNamedColumns;
impl GridNamedColumns {
    pub fn tracks(columns: &[GridColumn]) -> Option<Vec<ColumnVisibilityTrack>> {
        let tracks: Vec<ColumnVisibilityTrack> = columns.iter().filter_map(|column| {
            let GridColumn::Named { id, title, sizing, visibility } = column else { return None };
            assert!(!id.is_empty() && !title.is_empty(), "Named Grid columns require an ID and title");
            match sizing.as_ref() {
                GridColumn::Fixed(_) | GridColumn::Flexible { .. } => {}
                GridColumn::Adaptive { .. } | GridColumn::Named { .. } =>
                    panic!("Named Grid columns use fixed or flexible sizing"),
            }
            Some(ColumnVisibilityTrack {
                id: id.clone(),
                title: title.clone(),
                visibility: *visibility,
                width: GridTrackResolver::minimum_width(&[sizing.as_ref().clone()], 0.0),
            })
        }).collect();
        if tracks.is_empty() {
            return None;
        }
        assert_eq!(tracks.len(), columns.len(), "Use named columns for every field in a named Grid");
        let unique: HashSet<&String> = tracks.iter().map(|track| &track.id).collect();
        assert_eq!(unique.len(), tracks.len(), "Grid column IDs must be unique");
        Some(tracks)
    }

    pub fn minimum_width(columns: &[GridColumn], overrides: &std::collections::HashMap<String, bool>, spacing: f32) -> f32 {
        let Some(tracks) = Self::tracks(columns) else {
            return GridTrackResolver::minimum_width(columns, spacing);
        };
        let required = ColumnVisibilityPlan::resolve(&tracks, overrides, 0.0, spacing, &HashSet::new());
        required.indices.iter().map(|&index| tracks[index].width).sum::<f32>()
            + spacing * required.indices.len().saturating_sub(1) as f32
    }
}
