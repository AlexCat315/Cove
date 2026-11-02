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
    pub fn resolve(
        columns: &[ColumnVisibilityTrack],
        overrides: &std::collections::HashMap<String, bool>,
        available_width: f32,
        spacing: f32,
        required_ids: &HashSet<String>,
    ) -> Self {
        let protected: HashSet<usize> = columns
            .iter()
            .enumerate()
            .filter(|(_, column)| {
                column.visibility == ColumnVisibility::Required || required_ids.contains(&column.id)
            })
            .map(|(index, _)| index)
            .collect();
        let mut required = protected.clone();
        required.extend(
            columns
                .iter()
                .enumerate()
                .filter(|(_, column)| overrides.get(&column.id) == Some(&true))
                .map(|(index, _)| index),
        );
        let mut shown: HashSet<usize> = columns
            .iter()
            .enumerate()
            .filter(|(index, column)| {
                required.contains(index) || overrides.get(&column.id) != Some(&false)
            })
            .map(|(index, _)| index)
            .collect();
        let mut candidates: Vec<usize> = columns
            .iter()
            .enumerate()
            .filter(|(index, column)| {
                !required.contains(index) && !overrides.contains_key(&column.id)
            })
            .map(|(index, _)| index)
            .collect();
        candidates.sort_by(|&lhs, &rhs| {
            let left = columns[lhs].visibility.priority();
            let right = columns[rhs].visibility.priority();
            if left == right {
                rhs.cmp(&lhs)
            } else {
                left.cmp(&right)
            }
        });
        if required.is_empty() {
            let anchor = shown
                .iter()
                .copied()
                .min_by(|&lhs, &rhs| {
                    let left = columns[lhs].visibility.priority();
                    let right = columns[rhs].visibility.priority();
                    if left == right {
                        lhs.cmp(&rhs)
                    } else {
                        right.cmp(&left)
                    }
                })
                .or(if columns.is_empty() { None } else { Some(0) });
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
            indices: (0..columns.len())
                .filter(|index| shown.contains(index))
                .collect(),
            required_ids: protected
                .iter()
                .map(|&index| columns[index].id.clone())
                .collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GridColumnError {
    #[error("named and anonymous columns cannot be mixed")]
    MixedColumns,
    #[error("a named column requires a nonempty ID and title")]
    MissingLabel,
    #[error("a named column requires fixed or flexible sizing")]
    InvalidSizing,
    #[error(transparent)]
    Duplicate(#[from] crate::primitives::DuplicateId),
}

pub struct GridNamedColumns;
impl GridNamedColumns {
    pub fn tracks(
        columns: &[GridColumn],
    ) -> Result<Option<Vec<ColumnVisibilityTrack>>, GridColumnError> {
        if !columns
            .iter()
            .any(|column| matches!(column, GridColumn::Named { .. }))
        {
            return Ok(None);
        }
        let mut tracks = Vec::with_capacity(columns.len());
        for column in columns {
            let GridColumn::Named {
                id,
                title,
                sizing,
                visibility,
            } = column
            else {
                return Err(GridColumnError::MixedColumns);
            };
            if id.is_empty() || title.is_empty() {
                return Err(GridColumnError::MissingLabel);
            }
            if !matches!(
                sizing.as_ref(),
                GridColumn::Fixed(_) | GridColumn::Flexible { .. }
            ) {
                return Err(GridColumnError::InvalidSizing);
            }
            tracks.push(ColumnVisibilityTrack {
                id: id.clone(),
                title: title.clone(),
                visibility: *visibility,
                width: GridTrackResolver::minimum_width(std::slice::from_ref(sizing), 0.0),
            });
        }
        crate::primitives::check_ids(tracks.iter().map(|track| &track.id), "Grid column")?;
        Ok(Some(tracks))
    }

    pub fn minimum_width(
        columns: &[GridColumn],
        overrides: &std::collections::HashMap<String, bool>,
        spacing: f32,
    ) -> Result<f32, GridColumnError> {
        let Some(tracks) = Self::tracks(columns)? else {
            return Ok(GridTrackResolver::minimum_width(columns, spacing));
        };
        let required =
            ColumnVisibilityPlan::resolve(&tracks, overrides, 0.0, spacing, &HashSet::new());
        Ok(required
            .indices
            .iter()
            .map(|&index| tracks[index].width)
            .sum::<f32>()
            + spacing * required.indices.len().saturating_sub(1) as f32)
    }
}
