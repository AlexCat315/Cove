//! Behaviour tests for the combobox remote-search session.
//!
//! Translation of `Primitives/Controls/Fields/Combobox/{RemoteSearch,Results}.swift`.
//! The generation guard is the critical invariant: a slow response must never
//! overwrite results for a later query.

use cove_ui_compose::primitives::controls::fields::{
    ComboboxResultsPresentation, ComboboxSearchPhase, ComboboxSearchSession, results_presentation,
    too_short_prompt,
};

fn options(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn a_query_shorter_than_the_minimum_publishes_too_short_without_requesting() {
    let mut session = ComboboxSearchSession::new();
    assert_eq!(
        session.begin("ab", 3, false),
        None,
        "no request id is issued"
    );
    assert_eq!(session.phase(), &ComboboxSearchPhase::TooShort(3));
    assert!(session.results().is_empty());
}

#[test]
fn the_minimum_counts_characters_not_bytes() {
    let mut session = ComboboxSearchSession::new();
    // Three multi-byte characters are three characters, so this requests.
    assert!(session.begin("日本語", 3, false).is_some());
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loading);
}

#[test]
fn a_long_enough_query_enters_loading_and_issues_a_generation() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("frame", 0, false).expect("request issued");
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loading);
    assert_eq!(session.generation(), generation);
}

#[test]
fn repeating_the_current_query_does_not_refetch() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("frame", 0, false).unwrap();
    session.deliver(generation, options(&["a"]));
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loaded);
    // Same query again: skip, because the previous attempt succeeded.
    assert_eq!(session.begin("frame", 0, false), None);
}

#[test]
fn a_failed_query_may_be_retried_without_forcing() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("frame", 0, false).unwrap();
    session.fail(generation, "offline");
    assert!(session.phase().is_failure());
    assert!(
        session.begin("frame", 0, false).is_some(),
        "a failure may retry"
    );
}

#[test]
fn forcing_always_reissues_the_request() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("frame", 0, false).unwrap();
    session.deliver(generation, options(&["a"]));
    assert!(
        session.begin("frame", 0, true).is_some(),
        "force bypasses the repeat guard"
    );
}

#[test]
fn a_stale_generation_cannot_publish() {
    let mut session = ComboboxSearchSession::new();
    let first = session.begin("fra", 0, false).unwrap();
    // The person keeps typing before the first response lands.
    let second = session.begin("frame", 0, false).unwrap();
    assert_ne!(first, second, "a new query gets a new generation");

    assert!(
        !session.deliver(first, options(&["stale"])),
        "the older response must be rejected"
    );
    assert_eq!(session.results(), &[] as &[String]);
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loading);

    assert!(session.deliver(second, options(&["fresh"])));
    assert_eq!(session.results(), &["fresh".to_owned()]);
}

#[test]
fn duplicate_result_values_are_dropped_keeping_the_first() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("q", 0, false).unwrap();
    session.deliver(generation, options(&["a", "b", "a", "c", "b"]));
    assert_eq!(session.results(), &options(&["a", "b", "c"]));
}

#[test]
fn cancelling_a_loading_request_returns_to_idle_and_forgets_the_query() {
    let mut session = ComboboxSearchSession::new();
    session.begin("frame", 0, false);
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loading);
    session.cancel();
    assert_eq!(session.phase(), &ComboboxSearchPhase::Idle);
    assert_eq!(session.query(), None, "the next open re-requests");
}

#[test]
fn cancelling_keeps_loaded_results() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("frame", 0, false).unwrap();
    session.deliver(generation, options(&["kept"]));
    session.cancel();
    assert_eq!(
        session.results(),
        &["kept".to_owned()],
        "loaded results survive for the next open"
    );
}

#[test]
fn cancelling_invalidates_an_in_flight_response() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("frame", 0, false).unwrap();
    session.cancel();
    assert!(
        !session.deliver(generation, options(&["late"])),
        "cancelled delivery is rejected"
    );
    assert!(session.results().is_empty());
}

#[test]
fn the_results_area_shows_a_spinner_only_before_anything_arrives() {
    assert_eq!(
        results_presentation(&ComboboxSearchPhase::Loading, 0, "None"),
        ComboboxResultsPresentation::Searching
    );
    // With prior matches, a refresh dims them instead of blanking the list.
    assert_eq!(
        results_presentation(&ComboboxSearchPhase::Loading, 2, "None"),
        ComboboxResultsPresentation::List { dimmed: true }
    );
    assert_eq!(
        results_presentation(&ComboboxSearchPhase::Loaded, 2, "None"),
        ComboboxResultsPresentation::List { dimmed: false }
    );
}

#[test]
fn an_empty_result_set_uses_the_configured_empty_text() {
    assert_eq!(
        results_presentation(&ComboboxSearchPhase::Loaded, 0, "No matches"),
        ComboboxResultsPresentation::Empty("No matches".to_owned())
    );
}

#[test]
fn a_failure_and_a_too_short_phase_both_short_circuit_the_list() {
    assert_eq!(
        results_presentation(&ComboboxSearchPhase::Failed("offline".into()), 5, "x"),
        ComboboxResultsPresentation::Failed("offline".to_owned())
    );
    assert_eq!(
        results_presentation(&ComboboxSearchPhase::TooShort(3), 5, "x"),
        ComboboxResultsPresentation::TooShort(3)
    );
}

#[test]
fn the_prompt_pluralises() {
    assert_eq!(too_short_prompt(1), "Type at least 1 character to search");
    assert_eq!(too_short_prompt(3), "Type at least 3 characters to search");
}
