//! The live projection of a tree drag.
//!
//! While the pointer still holds a row, the tree is painted in the order the
//! drop would produce: the dragged subtree lifts out of the flattened rows and
//! re-enters at the projected slot with its whole depth shifted. The plan, the
//! projected order and the guide flags all derive from the flattened depth
//! sequence alone, so what the drag shows cannot drift from what the drop does.

use std::ops::Range;

use super::TreeDropPosition;

/// A move planned against the flattened row order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeReparentPlan {
    /// First flattened row of the dragged subtree.
    pub source: usize,
    /// Rows in the dragged subtree, including its root.
    pub block_len: usize,
    /// Where the block lands in the order without it.
    pub insert: usize,
    /// Depth the dragged root takes after the move.
    pub new_depth: usize,
}

impl TreeReparentPlan {
    /// Flattened rows that belong to the dragged subtree.
    pub fn block(&self) -> Range<usize> {
        self.source..self.source + self.block_len
    }
}

/// Rows in the subtree rooted at `index`, including the root itself. A
/// collapsed or leaf branch covers one row.
pub fn subtree_len(depths: &[usize], index: usize) -> usize {
    let depth = depths[index];
    let mut end = index + 1;
    while end < depths.len() && depths[end] > depth {
        end += 1;
    }
    end - index
}

/// The sibling above `index`, if the row has one.
pub fn previous_sibling(depths: &[usize], index: usize) -> Option<usize> {
    if index == 0 || index >= depths.len() {
        return None;
    }
    let depth = depths[index];
    let mut candidate = index - 1;
    while depths[candidate] > depth {
        if candidate == 0 {
            return None;
        }
        candidate -= 1;
    }
    (depths[candidate] == depth).then_some(candidate)
}

/// The sibling below `index`, if the row has one.
pub fn next_sibling(depths: &[usize], index: usize) -> Option<usize> {
    let candidate = index + subtree_len(depths, index);
    (candidate < depths.len() && depths[candidate] == depths[index]).then_some(candidate)
}

/// The row one level above `index` that owns it, if the row is not a root.
pub fn parent_index(depths: &[usize], index: usize) -> Option<usize> {
    let depth = *depths.get(index).filter(|&&depth| depth > 0)?;
    let mut candidate = index;
    while candidate > 0 {
        candidate -= 1;
        if depths[candidate] + 1 == depth {
            return Some(candidate);
        }
        if depths[candidate] < depth {
            return None;
        }
    }
    None
}

/// Plans the move the pointer projects, or `None` when it is invalid: a drop
/// onto the dragged subtree or its root, or one that leaves the row where it
/// already is.
pub fn plan_move(
    depths: &[usize],
    source: usize,
    target: usize,
    position: TreeDropPosition,
) -> Option<TreeReparentPlan> {
    if source >= depths.len() || target >= depths.len() || source == target {
        return None;
    }
    let block_len = subtree_len(depths, source);
    if (source..source + block_len).contains(&target) {
        return None;
    }
    let (insert, new_depth) = match position {
        // Before the target: a sibling just above it.
        TreeDropPosition::Before => (target, depths[target]),
        // After the target: a sibling below its whole branch.
        TreeDropPosition::After => (target + subtree_len(depths, target), depths[target]),
        // Inside the target: its last child. A collapsed target has no
        // flattened branch, so the block lands directly under it.
        TreeDropPosition::Inside => (target + subtree_len(depths, target), depths[target] + 1),
    };
    let insert = if insert > source {
        insert - block_len
    } else {
        insert
    };
    if insert == source && new_depth == depths[source] {
        return None;
    }
    Some(TreeReparentPlan {
        source,
        block_len,
        insert,
        new_depth,
    })
}

/// Where every row is painted while a plan is live, indexed by the row's own
/// place in the flattened order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeReparentProjection {
    /// The slot the row is painted at. Its displacement is
    /// `(slot - index) * stride`, so the flattened order — and every hit test
    /// against it — stays the caller's.
    pub slots: Vec<usize>,
    /// The depth the row is painted at.
    pub depths: Vec<usize>,
    /// Whether a following sibling is painted below the row's branch.
    pub has_next_sibling: Vec<bool>,
    /// Which ancestor columns continue past the row.
    pub ancestor_has_next_siblings: Vec<Vec<bool>>,
}

/// The flattened row order after the plan, as indices into the original rows.
fn projected_order(depths: &[usize], plan: &TreeReparentPlan) -> Vec<usize> {
    let block = plan.block();
    let mut out: Vec<usize> = (0..depths.len())
        .filter(|index| !block.contains(index))
        .collect();
    let tail = out.split_off(plan.insert.min(out.len()));
    out.extend(block.clone());
    out.extend(tail);
    out
}

/// The depth each projected row is painted at. The dragged subtree keeps its
/// internal shape and shifts as a whole.
fn projected_depths(depths: &[usize], plan: &TreeReparentPlan) -> Vec<usize> {
    let block = plan.block();
    let base = depths[plan.source];
    projected_order(depths, plan)
        .into_iter()
        .map(|index| {
            if block.contains(&index) {
                depths[index] - base + plan.new_depth
            } else {
                depths[index]
            }
        })
        .collect()
}

/// Resolves the plan into where each row is painted, at what depth, and with
/// which guide flags.
pub fn project(depths: &[usize], plan: &TreeReparentPlan) -> TreeReparentProjection {
    let order = projected_order(depths, plan);
    let painted_depths = projected_depths(depths, plan);
    let (painted_next, painted_ancestors) = guide_flags(&painted_depths);
    let mut projection = TreeReparentProjection {
        slots: vec![0; depths.len()],
        depths: vec![0; depths.len()],
        has_next_sibling: vec![false; depths.len()],
        ancestor_has_next_siblings: vec![Vec::new(); depths.len()],
    };
    for (slot, &row) in order.iter().enumerate() {
        projection.slots[row] = slot;
        projection.depths[row] = painted_depths[slot];
        projection.has_next_sibling[row] = painted_next[slot];
        projection.ancestor_has_next_siblings[row] = painted_ancestors[slot].clone();
    }
    projection
}

/// Guide flags derived from a depth sequence: whether each row has a following
/// sibling, and which ancestor columns continue past it.
///
/// A row's branch ends at the next row that is not deeper than it, so the
/// flags follow from the flattened order alone and stay correct for a filtered
/// or projected sequence.
pub fn guide_flags(depths: &[usize]) -> (Vec<bool>, Vec<Vec<bool>>) {
    let count = depths.len();
    let mut branch_ends = vec![count; count];
    let mut open: Vec<usize> = Vec::new();
    for index in 0..count {
        while let Some(&ancestor) = open
            .last()
            .filter(|&&ancestor| depths[ancestor] >= depths[index])
        {
            open.pop();
            branch_ends[ancestor] = index;
        }
        open.push(index);
    }
    let has_next_sibling: Vec<bool> = (0..count)
        .map(|index| {
            let end = branch_ends[index];
            end < count && depths[end] == depths[index]
        })
        .collect();

    let mut ancestors: Vec<Vec<bool>> = vec![Vec::new(); count];
    let mut path: Vec<usize> = Vec::new();
    for index in 0..count {
        let depth = depths[index];
        while path.len() > depth {
            path.pop();
        }
        // Column `level` continues the branch of the ancestor at depth
        // `level + 1`. The root level has no column, so its sibling state must
        // not shift the descendant columns; an ancestor depth missing from the
        // path leaves its column broken.
        let mut columns = vec![false; depth.saturating_sub(1)];
        for &row in &path {
            let ancestor_depth = depths[row];
            if ancestor_depth > 0
                && let Some(column) = columns.get_mut(ancestor_depth - 1)
            {
                *column = has_next_sibling[row];
            }
        }
        ancestors[index] = columns;
        path.push(index);
    }
    (has_next_sibling, ancestors)
}
