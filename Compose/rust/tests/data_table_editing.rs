//! Behaviour tests for `DataTableEditingState`.
//!
//! Translation of `Primitives/Data/DataTable/DataTableEditing.swift`. A rejected
//! edit must keep the person's input, and Tab must skip non-editable columns —
//! both are easy to get subtly wrong and invisible without assertions.

use cove_ui_compose::primitives::data::data_table::{
    CommitAnswer, CommitOutcome, DataTableEditingState, EditableColumn,
};

fn columns() -> Vec<EditableColumn> {
    vec![
        EditableColumn { id: "name".into(), is_editable: true },
        EditableColumn { id: "size".into(), is_editable: false },
        EditableColumn { id: "kind".into(), is_editable: true },
    ]
}

#[test]
fn beginning_an_edit_opens_a_draft() {
    let mut state = DataTableEditingState::new();
    assert!(state.begin("row-1", "name", "Frame rate", |_| CommitAnswer::Accept));
    let draft = state.draft.as_ref().expect("draft is open");
    assert_eq!(draft.buffer, "Frame rate");
    assert_eq!(draft.error, None);
}

#[test]
fn re_beginning_the_same_cell_is_a_no_op() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "name", "a", |_| CommitAnswer::Accept);
    state.draft.as_mut().unwrap().buffer = "edited".into();
    assert!(!state.begin("row-1", "name", "a", |_| CommitAnswer::Accept));
    assert_eq!(
        state.draft.as_ref().unwrap().buffer,
        "edited",
        "re-entering the same cell must not discard the in-progress text"
    );
}

#[test]
fn committing_an_unchanged_value_names_no_update() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "name", "same", |_| CommitAnswer::Accept);
    assert_eq!(state.commit(|_| CommitAnswer::Accept), CommitOutcome::Unchanged);
    assert!(state.draft.is_none());
}

#[test]
fn committing_a_changed_value_reports_the_new_value() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "name", "old", |_| CommitAnswer::Accept);
    state.draft.as_mut().unwrap().buffer = "new".into();
    assert_eq!(
        state.commit(|_| CommitAnswer::Accept),
        CommitOutcome::Updated("new".into())
    );
    assert!(state.draft.is_none());
}

#[test]
fn a_rejected_commit_keeps_the_draft_and_records_the_error() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "size", "12", |_| CommitAnswer::Accept);
    state.draft.as_mut().unwrap().buffer = "abc".into();
    assert_eq!(
        state.commit(|_| CommitAnswer::Rejected("Enter a number".into())),
        CommitOutcome::Rejected("Enter a number".into())
    );
    assert!(state.draft.is_some(), "a rejected edit stays open");
    assert!(state.has_error());
    assert_eq!(state.draft.as_ref().unwrap().buffer, "abc");
}

#[test]
fn a_new_edit_aborts_when_the_previous_one_is_rejected() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "name", "a", |_| CommitAnswer::Accept);
    state.draft.as_mut().unwrap().buffer = "bad".into();
    // Moving to another cell must not silently swallow the rejected value.
    assert!(!state.begin("row-2", "name", "b", |_| CommitAnswer::Rejected("invalid".into())));
    assert_eq!(
        state.draft.as_ref().map(|draft| draft.row_id.as_str()),
        Some("row-1"),
        "focus stays on the rejected cell"
    );
}

#[test]
fn a_new_edit_commits_the_previous_one_when_it_is_valid() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "name", "a", |_| CommitAnswer::Accept);
    state.draft.as_mut().unwrap().buffer = "edited".into();
    assert!(state.begin("row-2", "name", "b", |_| CommitAnswer::Accept));
    assert_eq!(state.draft.as_ref().unwrap().row_id, "row-2");
}

#[test]
fn an_orphaned_row_drops_the_draft() {
    let mut state = DataTableEditingState::new();
    state.begin("row-1", "name", "a", |_| CommitAnswer::Accept);
    assert_eq!(state.commit(|_| CommitAnswer::Orphaned), CommitOutcome::Orphaned);
    assert!(state.draft.is_none(), "a deleted row cannot keep an editor open");
}

#[test]
fn tab_skips_non_editable_columns() {
    let cols = columns();
    // `size` cannot be edited, so Tab from `name` lands on `kind`.
    assert_eq!(
        DataTableEditingState::next_editable("name", 1, &cols),
        Some("kind".into())
    );
    // And backwards from `kind` also lands on `name`.
    assert_eq!(
        DataTableEditingState::next_editable("kind", -1, &cols),
        Some("name".into())
    );
}

#[test]
fn tab_past_the_last_editable_column_ends_the_walk() {
    let cols = columns();
    assert_eq!(DataTableEditingState::next_editable("kind", 1, &cols), None);
    assert_eq!(DataTableEditingState::next_editable("name", -1, &cols), None);
}

#[test]
fn an_unknown_column_yields_no_target() {
    assert_eq!(
        DataTableEditingState::next_editable("ghost", 1, &columns()),
        None
    );
}
