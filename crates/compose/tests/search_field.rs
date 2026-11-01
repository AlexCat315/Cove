//! Translation of `Tests/CoveUIComposeTests/SearchFieldTests.swift`.
//!
//! Behaviour checklist:
//! - Escape clears the query when the field opts in
//! - arrows move the suggestion highlight and Return chooses it
//! - Return submits the current query when no suggestion is highlighted
//! - an empty query browses every suggestion
//! - choosing closes the list so it does not immediately reopen
//!
//! Model difference: the Swift suite mounts the field and dispatches real key
//! events. The Rust `SearchFieldState` exposes the same key contract as a plain
//! method (`handle_key` returning a `SearchAction`), so these tests drive it
//! directly with the same inputs and expected crates/text/highlight outcomes.

use cove_ui_compose::primitives::controls::fields::{SearchAction, SearchFieldState, SearchKey};

const SUGGESTIONS: [&str; 3] = ["Camera", "Canvas", "Counter"];

fn suggestions() -> Vec<String> {
    SUGGESTIONS
        .iter()
        .map(|value| (*value).to_owned())
        .collect()
}

#[test]
fn escape_clears_the_query_when_the_field_opts_in() {
    let mut state = SearchFieldState::new();
    state.on_change("camera", &suggestions());

    let action = state.handle_key(SearchKey::Escape, "camera", &suggestions(), true);
    assert_eq!(
        action,
        SearchAction::Clear,
        "Escape clears rather than passing through"
    );
    assert_eq!(state.highlighted, None, "clearing drops the highlight");
}

#[test]
fn escape_is_passed_through_when_clearing_is_disabled() {
    let mut state = SearchFieldState::new();
    let action = state.handle_key(SearchKey::Escape, "camera", &suggestions(), false);
    assert_eq!(
        action,
        SearchAction::None,
        "the caller decides what Escape means"
    );
}

#[test]
fn arrow_keys_move_the_highlight_and_return_chooses_it() {
    let mut state = SearchFieldState::new();
    state.on_change("Ca", &suggestions());
    // "Ca" matches Camera then Canvas, in source order.
    assert_eq!(state.highlighted, Some(0));

    assert_eq!(
        state.handle_key(SearchKey::ArrowDown, "Ca", &suggestions(), true),
        SearchAction::MoveHighlight
    );
    assert_eq!(
        state.highlighted,
        Some(1),
        "the highlight steps to the second row"
    );

    assert_eq!(
        state.handle_key(SearchKey::Return, "Ca", &suggestions(), true),
        SearchAction::Choose(1)
    );
    assert_eq!(state.highlighted, None, "choosing drops the highlight");
    assert!(
        state.suppress_suggestions,
        "the list stays closed after choosing"
    );
    assert!(!state.is_focused, "choosing also drops focus");
}

#[test]
fn the_highlight_clamps_at_both_ends() {
    let mut state = SearchFieldState::new();
    state.on_change("Ca", &suggestions());

    state.handle_key(SearchKey::ArrowUp, "Ca", &suggestions(), true);
    assert_eq!(
        state.highlighted,
        Some(0),
        "an upward move from the first row stays put"
    );

    // "Ca" matches Camera and Canvas, but not Counter.
    for _ in 0..5 {
        state.handle_key(SearchKey::ArrowDown, "Ca", &suggestions(), true);
    }
    assert_eq!(
        state.highlighted,
        Some(1),
        "a downward move stops at the last match"
    );
}

#[test]
fn return_submits_when_no_suggestion_is_highlighted() {
    let mut state = SearchFieldState::new();
    // No focused list and no matches, so nothing is highlighted.
    let action = state.handle_key(SearchKey::Return, "light", &[], true);
    assert_eq!(
        action,
        SearchAction::Submit,
        "an unhighlighted Return submits the text"
    );
}

#[test]
fn an_empty_query_browses_every_suggestion() {
    assert_eq!(
        SearchFieldState::matches("", &suggestions()),
        suggestions(),
        "an empty query lists everything"
    );
    assert_eq!(
        SearchFieldState::matches("  ", &suggestions()),
        suggestions(),
        "whitespace counts as empty"
    );
}

#[test]
fn matching_folds_case_and_ignores_surrounding_space() {
    let matches = SearchFieldState::matches("  caN  ", &suggestions());
    assert_eq!(
        matches,
        vec!["Canvas".to_owned()],
        "only the suggestion containing the folded needle remains"
    );
}

#[test]
fn the_list_closes_when_nothing_matches_or_the_field_is_disabled() {
    let mut state = SearchFieldState::new();
    state.on_focus("zzz", &suggestions());
    assert!(
        !state.shows_suggestions("zzz", &suggestions(), true),
        "no match means no list"
    );
    assert!(
        !state.shows_suggestions("Ca", &suggestions(), false),
        "a disabled field never shows the list"
    );
    assert!(
        state.shows_suggestions("Ca", &suggestions(), true),
        "a focused match opens the list"
    );
}

#[test]
fn blurring_clears_the_highlight_and_reopening_restores_the_first_row() {
    let mut state = SearchFieldState::new();
    state.on_focus("Ca", &suggestions());
    assert_eq!(state.highlighted, Some(0));

    state.on_blur();
    assert_eq!(state.highlighted, None, "blur drops the highlight");

    state.on_focus("Ca", &suggestions());
    assert_eq!(
        state.highlighted,
        Some(0),
        "focus highlights the first match again"
    );
}

#[test]
fn a_change_reopens_a_list_closed_by_choosing() {
    let mut state = SearchFieldState::new();
    state.on_change("Ca", &suggestions());
    state.choose();
    assert!(state.suppress_suggestions);

    state.on_change("Can", &suggestions());
    assert!(
        !state.suppress_suggestions,
        "retargeting the query reopens the list"
    );
    assert_eq!(state.highlighted, Some(0));
}

#[test]
fn arrow_keys_do_nothing_without_matches() {
    let mut state = SearchFieldState::new();
    assert_eq!(
        state.handle_key(SearchKey::ArrowDown, "zzz", &suggestions(), true),
        SearchAction::None,
        "there is nothing to move through"
    );
    assert_eq!(state.highlighted, None);
}
