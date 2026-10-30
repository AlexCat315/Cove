//! Translation of `Primitives/Data/DataTable/ColumnOrder.swift`.
//!
//! Display order of authored columns. Unknown IDs are ignored and columns
//! missing from a saved order keep their authored position after the ordered
//! ones, so adding a column never requires migrating a saved order.
//!
//! Behaviour checklist:
//! - an empty order applies nothing
//! - ordered columns come first in order, unordered ones after, in authored order
//! - unknown IDs in the order are ignored
//! - `move` inserts after the target when moving right, before it when moving left
//! - `move` is a no-op for an unknown source or target, or for source == target
//! - `target` reports the presented column whose span contains the dragged
//!   column's centre

use std::collections::HashMap;

use super::data_table_geometry::DataTableColumnGeometry;

/// Authorable column ordering for a data table.
pub enum DataTableColumnOrder {}

impl DataTableColumnOrder {
    /// Reorders `ids` (authored order) by `order` (saved display order).
    pub fn apply(order: &[String], ids: &[String]) -> Vec<String> {
        if order.is_empty() {
            return ids.to_vec();
        }
        // First occurrence wins, so a duplicated saved entry cannot shift the
        // ranking of the columns after it.
        let mut rank: HashMap<&str, usize> = HashMap::new();
        for (index, id) in order.iter().enumerate() {
            rank.entry(id.as_str()).or_insert(index);
        }
        let mut indexed: Vec<(usize, &String)> = ids.iter().enumerate().collect();
        indexed.sort_by(|(left_index, left), (right_index, right)| {
            match (rank.get(left.as_str()), rank.get(right.as_str())) {
                (Some(a), Some(b)) => a.cmp(b),
                // Ordered columns lead.
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                // Both unordered: keep their authored position.
                (None, None) => left_index.cmp(right_index),
            }
        });
        indexed.into_iter().map(|(_, id)| id.clone()).collect()
    }

    /// Moves `source` next to `target` in the full order, keeping hidden columns
    /// in their relative places. Moves left → before the target; right → after.
    pub fn move_id(source: &str, target: &str, order: &[String]) -> Vec<String> {
        if source == target {
            return order.to_vec();
        }
        let (Some(from), Some(to)) = (
            order.iter().position(|id| id == source),
            order.iter().position(|id| id == target),
        ) else {
            return order.to_vec();
        };
        let mut result = order.to_vec();
        result.remove(from);
        let anchor = result
            .iter()
            .position(|id| id == target)
            .expect("the target survives removing the source, which differs from it");
        let insert_at = if to > from { anchor + 1 } else { anchor };
        result.insert(insert_at, source.to_owned());
        result
    }

    /// The presented column whose span contains the dragged column's centre.
    pub fn target(source: usize, offset: f32, geometry: &DataTableColumnGeometry) -> usize {
        let Some(&width) = geometry.widths.get(source) else { return source };
        let starts = geometry.starts();
        let Some(&start) = starts.get(source) else { return source };
        let centre = start + width / 2.0 + offset;
        starts
            .iter()
            .rposition(|entry| *entry <= centre)
            .unwrap_or(0)
    }
}
