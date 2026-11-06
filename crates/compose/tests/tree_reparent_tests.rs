//! The live projection of a tree drag, tested against the flattened depth
//! sequence alone.
//!
//! What the pointer sees must be exactly what the drop produces, so the plan,
//! the painted slots and the guide flags are all pure functions of the depths:
//! a tree of `A → {A1 → {A1a}, A2}` and `B` flattens to `[0, 1, 2, 1, 0]`.

use cove_ui_compose::primitives::data::lists::tree::TreeDropPosition;
use cove_ui_compose::primitives::data::lists::tree::reparent::{
    guide_flags, next_sibling, parent_index, plan_move, previous_sibling, project, subtree_len,
};

/// `A`, `A1`, `A1a`, `A2`, `B`.
const FOREST: [usize; 5] = [0, 1, 2, 1, 0];

#[test]
fn subtree_lengths_cover_only_deeper_rows() {
    assert_eq!(subtree_len(&FOREST, 0), 4, "A owns the whole first branch");
    assert_eq!(subtree_len(&FOREST, 1), 2, "A1 owns A1a");
    assert_eq!(subtree_len(&FOREST, 2), 1, "a leaf branch is one row");
    assert_eq!(subtree_len(&FOREST, 4), 1, "a collapsed root is one row");
}

#[test]
fn siblings_and_parents_resolve_from_depths() {
    assert_eq!(previous_sibling(&FOREST, 3), Some(1), "A2 follows A1");
    assert_eq!(
        previous_sibling(&FOREST, 2),
        None,
        "A1a has no upper sibling"
    );
    assert_eq!(
        previous_sibling(&FOREST, 0),
        None,
        "the first row has no sibling"
    );
    assert_eq!(next_sibling(&FOREST, 1), Some(3), "A1 is followed by A2");
    assert_eq!(next_sibling(&FOREST, 0), Some(4), "A is followed by B");
    assert_eq!(next_sibling(&FOREST, 3), None, "A2 has no lower sibling");
    assert_eq!(parent_index(&FOREST, 2), Some(1), "A1a belongs to A1");
    assert_eq!(parent_index(&FOREST, 4), None, "a root has no parent");
}

#[test]
fn a_drop_before_a_row_projects_it_as_the_targets_upper_sibling() {
    let plan = plan_move(&FOREST, 2, 3, TreeDropPosition::Before).expect("a legal move");
    assert_eq!(plan.source, 2);
    assert_eq!(plan.block(), 2..3, "only A1a travels");
    assert_eq!(plan.insert, 2, "the block lands where A1a already was");
    assert_eq!(
        plan.new_depth, 1,
        "A1a becomes an upper sibling of A2, so it loses a level"
    );

    let projection = project(&FOREST, &plan);
    assert_eq!(
        projection.depths[2], 1,
        "the dragged row paints one level up"
    );
    assert_eq!(
        projection.slots,
        vec![0, 1, 2, 3, 4],
        "moving to the slot above A2 keeps every row in place"
    );
}

#[test]
fn a_drop_after_a_row_projects_it_below_the_whole_target_branch() {
    // B after A1a: B is painted inside the branch, below its last row.
    let plan = plan_move(&FOREST, 4, 2, TreeDropPosition::After).expect("a legal move");
    assert_eq!(plan.insert, 3, "the block lands below A1a");
    assert_eq!(plan.new_depth, 2, "B takes A1a's depth");

    let projection = project(&FOREST, &plan);
    assert_eq!(
        projection.slots,
        vec![0, 1, 2, 4, 3],
        "B moves up one slot and A2 moves down one"
    );
    assert_eq!(
        projection.depths,
        vec![0, 1, 2, 1, 2],
        "A2 keeps its level and B is painted at A1a's"
    );
}

#[test]
fn a_drop_inside_a_row_projects_it_as_the_last_child() {
    let plan = plan_move(&FOREST, 2, 4, TreeDropPosition::Inside).expect("a legal move");
    assert_eq!(
        plan.insert, 4,
        "past the source, so the block length is discounted"
    );
    assert_eq!(plan.new_depth, 1, "A1a becomes a child of B");

    let projection = project(&FOREST, &plan);
    assert_eq!(
        projection.slots,
        vec![0, 1, 4, 2, 3],
        "A1a travels to the last slot while A2 and B each step up one"
    );
    assert_eq!(projection.depths, vec![0, 1, 1, 1, 0]);
}

#[test]
fn a_whole_branch_moves_as_one_block() {
    // A (with A1 and A1a and A2) inside B.
    let plan = plan_move(&FOREST, 0, 4, TreeDropPosition::Inside).expect("a legal move");
    assert_eq!(plan.block(), 0..4, "the dragged subtree travels together");
    assert_eq!(plan.insert, 1);
    assert_eq!(plan.new_depth, 1, "A becomes B's child");

    let projection = project(&FOREST, &plan);
    assert_eq!(
        projection.slots,
        vec![1, 2, 3, 4, 0],
        "the block is painted after B, which moves up to the first slot"
    );
    assert_eq!(
        projection.depths,
        vec![1, 2, 3, 2, 0],
        "the branch keeps its internal shape and shifts as a whole"
    );
}

#[test]
fn moves_into_the_dragged_subtree_and_no_ops_are_rejected() {
    assert_eq!(
        plan_move(&FOREST, 0, 1, TreeDropPosition::Inside),
        None,
        "A cannot become a descendant of itself"
    );
    assert_eq!(
        plan_move(&FOREST, 0, 0, TreeDropPosition::After),
        None,
        "the dragged row is not a valid target"
    );
    assert_eq!(
        plan_move(&FOREST, 1, 3, TreeDropPosition::Before),
        None,
        "A1 before A2 is where A1 already is"
    );
}

#[test]
fn projected_slots_are_a_permutation() {
    for source in 0..FOREST.len() {
        for target in 0..FOREST.len() {
            for position in [
                TreeDropPosition::Before,
                TreeDropPosition::Inside,
                TreeDropPosition::After,
            ] {
                let Some(plan) = plan_move(&FOREST, source, target, position) else {
                    continue;
                };
                let projection = project(&FOREST, &plan);
                let mut sorted = projection.slots.clone();
                sorted.sort_unstable();
                assert_eq!(
                    sorted,
                    (0..FOREST.len()).collect::<Vec<_>>(),
                    "slot map for {source} -> {target} {position:?} must be a permutation"
                );
                assert_eq!(
                    projection.slots[source], plan.insert,
                    "the dragged root paints at the planned slot for {source} -> {target} {position:?}"
                );
            }
        }
    }
}

#[test]
fn guide_flags_close_a_branch_at_its_last_sibling() {
    let (has_next_sibling, ancestors) = guide_flags(&FOREST);
    assert_eq!(
        has_next_sibling,
        vec![true, true, false, false, false],
        "A and A1 continue below, the last rows of their branches do not"
    );
    assert_eq!(
        ancestors[0],
        Vec::<bool>::new(),
        "a root has no ancestor columns"
    );
    assert_eq!(
        ancestors[1],
        Vec::<bool>::new(),
        "A1 owns the only column a level-1 row draws, so the root's sibling state is not a column"
    );
    assert_eq!(
        ancestors[2],
        vec![true],
        "A1's column continues past A1a; the root level draws no column of its own"
    );
    assert_eq!(
        ancestors[3],
        Vec::<bool>::new(),
        "A2 closes its own branch, so it has no ancestor column either"
    );
    assert_eq!(
        ancestors[4],
        Vec::<bool>::new(),
        "B has no ancestor columns"
    );
}

#[test]
fn guide_flags_index_columns_by_ancestor_depth() {
    // `A → {A1 → {A1a → A1aI}, A2}` and `B`.
    let (has_next_sibling, ancestors) = guide_flags(&[0, 1, 2, 3, 1, 0]);
    assert_eq!(
        has_next_sibling,
        vec![true, true, false, false, false, false],
        "A and A1 continue below, every last sibling does not"
    );
    assert_eq!(
        ancestors[3],
        vec![true, false],
        "column 0 is A1's continuation and column 1 is A1a's, which closes at A2"
    );
}

#[test]
fn guide_flags_move_with_the_projection() {
    let plan = plan_move(&FOREST, 4, 2, TreeDropPosition::After).expect("a legal move");
    let projection = project(&FOREST, &plan);
    assert_eq!(
        projection.has_next_sibling,
        vec![false, true, true, false, false],
        "B joins the first branch, so A1a gains a lower sibling and the root A loses one"
    );
    assert_eq!(
        projection.ancestor_has_next_siblings[4],
        vec![true],
        "B is painted at depth 2 inside the branch whose only column, A1's, continues below it"
    );
}
