//! Behaviour tests for `SearchFieldState`.
//!
//! Translation of the matching/highlight/key logic in
//! `Primitives/Controls/Fields/SearchField.swift`.

use cove_ui_compose::primitives::controls::fields::{
    SearchAction, SearchFieldState, SearchKey,
};

fn suggestions() -> Vec<String> {
    vec!["Frame rate".into(), "frame buffer".into(), "Memory".into()]
}

#[test]
fn matching_is_case_insensitive_and_shows_everything_for_an_empty_query() {
    assert_eq!(SearchFieldState::matches("", &suggestions()).len(), 3);
    assert_eq!(SearchFieldState::matches("   ", &suggestions()).len(), 3, "whitespace trims away");
    assert_eq!(
        SearchFieldState::matches("FRAME", &suggestions()),
        vec!["Frame rate".to_owned(), "frame buffer".to_owned()]
    );
    assert!(SearchFieldState::matches("zzz", &suggestions()).is_empty());
}

#[test]
fn suggestions_hide_while_suppressed_disabled_or_unfocused_and_empty() {
    let mut state = SearchFieldState::new();
    // Unfocused with empty text: nothing to show.
    assert!(!state.shows_suggestions("", &suggestions(), true));
    // Typing opens it.
    state.on_change("f", &suggestions());
    state.is_focused = true;
    assert!(state.shows_suggestions("f", &suggestions(), true));
    // Disabled hides it.
    assert!(!state.shows_suggestions("f", &suggestions(), false));
    // Choosing suppresses it.
    state.choose();
    assert!(!state.shows_suggestions("f", &suggestions(), true));
    // A further edit reopens it.
    state.on_change("fr", &suggestions());
    assert!(state.shows_suggestions("fr", &suggestions(), true));
}

#[test]
fn typing_highlights_the_first_match_or_clears_it() {
    let mut state = SearchFieldState::new();
    state.on_change("frame", &suggestions());
    assert_eq!(state.highlighted, Some(0));
    state.on_change("zzz", &suggestions());
    assert_eq!(state.highlighted, None, "no matches means no highlight");
}

#[test]
fn arrow_keys_move_and_clamp() {
    let mut state = SearchFieldState::new();
    let items = suggestions();
    state.is_focused = true;
    state.on_change("", &items);
    assert_eq!(state.highlighted, Some(0));
    state.handle_key(SearchKey::ArrowDown, "", &items, true);
    assert_eq!(state.highlighted, Some(1));
    state.handle_key(SearchKey::ArrowDown, "", &items, true);
    assert_eq!(state.highlighted, Some(2));
    // Clamps at the last row rather than wrapping.
    state.handle_key(SearchKey::ArrowDown, "", &items, true);
    assert_eq!(state.highlighted, Some(2));
    // And back up to the first.
    for _ in 0..5 {
        state.handle_key(SearchKey::ArrowUp, "", &items, true);
    }
    assert_eq!(state.highlighted, Some(0));
}

#[test]
fn escape_clears_only_when_enabled() {
    let mut state = SearchFieldState::new();
    let items = suggestions();
    assert_eq!(
        state.handle_key(SearchKey::Escape, "frame", &items, true),
        SearchAction::Clear
    );
    assert_eq!(
        state.handle_key(SearchKey::Escape, "frame", &items, false),
        SearchAction::None,
        "clears_on_escape: false leaves the text alone"
    );
}

#[test]
fn return_chooses_the_highlighted_row_otherwise_submits() {
    let mut state = SearchFieldState::new();
    let items = suggestions();
    state.on_change("frame", &items);
    assert_eq!(
        state.handle_key(SearchKey::Return, "frame", &items, true),
        SearchAction::Choose(0)
    );
    assert!(state.suppress_suggestions, "choosing closes the list");

    // With no highlight, Return submits the typed text.
    let mut fresh = SearchFieldState::new();
    assert_eq!(
        fresh.handle_key(SearchKey::KeypadEnter, "frame buffer", &[], true),
        SearchAction::Submit
    );
}

#[test]
fn arrows_are_inert_when_nothing_matches() {
    let mut state = SearchFieldState::new();
    assert_eq!(
        state.handle_key(SearchKey::ArrowDown, "zzz", &suggestions(), true),
        SearchAction::None
    );
    assert_eq!(state.highlighted, None);
}

#[test]
fn blur_clears_the_highlight() {
    let mut state = SearchFieldState::new();
    state.on_change("frame", &suggestions());
    state.is_focused = true;
    assert_eq!(state.highlighted, Some(0));
    state.on_blur();
    assert_eq!(state.highlighted, None);
    assert!(!state.is_focused);
}
