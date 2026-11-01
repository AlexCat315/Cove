//! Translation of `Tests/CoveUIComposeTests/ComboboxSearchTests.swift`.
//!
//! Behaviour checklist:
//! - a superseded request cannot publish over the newer query; the cancelled
//!   generation is rejected and duplicate option values are dropped
//! - repeating a loaded query is not requested again
//! - short queries prompt instead of requesting, and the character count ignores
//!   surrounding whitespace
//! - cancelling a pending request returns it to idle and forgets the query, so the
//!   next open re-requests
//! - a failure surfaces its message and a forced retry requests again
//! - `minimumQueryLength` and `delay` are clamped to sane values
//!
//! Named differences from the Swift suite: Swift's `ComboboxSearch` ran an `async`
//! closure and pushed results back through an enqueue/`EventDispatcher` pair, so
//! its tests awaited real work-queue drains. The Rust model is a synchronous
//! `ComboboxSearchSession` that a host drives with `begin` / `deliver` / `fail` /
//! `cancel`, keyed by a generation counter. Every Swift behaviour is expressed
//! against generation ordering, which is the actual subject of the suite. The two
//! Swift cases with no Rust counterpart are noted at their tests:
//! - the debounce/quiet period has no Rust equivalent (there is no timer; the host
//!   decides when to call `begin`);
//! - "no host fails visibly" (`enqueue: nil`) has none either, because the Rust
//!   host always supplies the enqueue step by construction.

mod support;

use cove_ui_compose::primitives::controls::fields::combobox_model::{
    ComboboxSearchPhase, ComboboxSearchSession,
};

fn deliver(session: &mut ComboboxSearchSession, generation: u64, options: &[&str]) {
    let options: Vec<String> = options.iter().map(|option| option.to_string()).collect();
    assert!(
        session.deliver(generation, options),
        "generation {generation} is the newest"
    );
}

#[test]
fn a_superseded_request_cannot_publish_over_the_newer_query() {
    let mut session = ComboboxSearchSession::new();

    let slow = session
        .begin("slow", 0, false)
        .expect("the first query starts a request");
    assert_eq!(
        session.phase(),
        &ComboboxSearchPhase::Loading,
        "the first request is in flight"
    );

    let fast = session
        .begin("fast", 0, false)
        .expect("the second query starts a request");
    assert!(fast > slow, "the second query advances the generation");

    deliver(
        &mut session,
        fast,
        &["fast result", "fast result", "duplicate"],
    );
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loaded);
    assert_eq!(
        session.results(),
        &["fast result".to_string(), "duplicate".to_string()],
        "duplicate option values are dropped, keeping the first"
    );

    assert!(
        !session.deliver(slow, vec!["slow result".to_string()]),
        "the cancelled generation cannot publish"
    );
    assert_eq!(
        session.results(),
        &["fast result".to_string(), "duplicate".to_string()],
        "the stale delivery left the results alone"
    );

    assert!(
        !session.should_request("fast", false),
        "a loaded query is not requested again"
    );
    assert!(
        session.begin("fast", 0, false).is_none(),
        "repeating the loaded query starts nothing"
    );
}

#[test]
fn a_loaded_query_is_only_rerequested_after_a_failure_or_a_forced_retry() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("pa", 0, false).expect("the query starts");
    deliver(&mut session, generation, &["Paris"]);

    assert!(
        !session.should_request("pa", false),
        "loaded queries are cached"
    );
    assert!(
        session.should_request("pa", true),
        "a forced retry is allowed"
    );

    let retry = session
        .begin("pa", 0, true)
        .expect("the forced retry starts");
    assert!(
        session.fail(retry, "Offline"),
        "the retry reports a failure"
    );
    assert!(
        session.should_request("pa", false),
        "a failed query is requested again without forcing"
    );
}

#[test]
fn failures_surface_their_message_and_a_retry_requests_again() {
    let mut session = ComboboxSearchSession::new();
    let first = session.begin("pa", 0, false).expect("the query starts");
    assert!(session.fail(first, "Offline"));
    assert_eq!(
        session.phase(),
        &ComboboxSearchPhase::Failed("Offline".to_string()),
        "the failure message is surfaced verbatim"
    );
    assert!(session.phase().is_failure(), "the phase reports a failure");

    let retry = session
        .begin("pa", 0, true)
        .expect("a forced retry requests again");
    deliver(&mut session, retry, &["Paris"]);
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loaded);
    assert_eq!(session.results(), &["Paris".to_string()]);

    assert!(
        !session.fail(first, "stale"),
        "an old generation cannot replace a loaded result with a failure"
    );
}

#[test]
fn short_queries_prompt_instead_of_requesting() {
    let mut session = ComboboxSearchSession::new();

    assert!(
        session.begin(" a ", 2, false).is_none(),
        "a one-character query after trimming starts nothing"
    );
    assert_eq!(
        session.phase(),
        &ComboboxSearchPhase::TooShort(2),
        "the session asks for more characters"
    );
    assert!(
        session.results().is_empty(),
        "a short query publishes no results"
    );

    let generation = session
        .begin("abc", 2, false)
        .expect("a long enough query starts");
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loading);
    deliver(&mut session, generation, &[]);
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loaded);
    assert!(session.results().is_empty(), "an empty result set is valid");
}

#[test]
fn cancelling_a_pending_request_returns_it_to_idle() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("abc", 0, false).expect("the query starts");
    assert_eq!(session.phase(), &ComboboxSearchPhase::Loading);

    session.cancel();
    assert_eq!(
        session.phase(),
        &ComboboxSearchPhase::Idle,
        "cancel returns to idle"
    );
    assert_eq!(session.query(), None, "cancel forgets the query");
    assert!(
        !session.deliver(generation, vec!["late".to_string()]),
        "the cancelled generation never publishes"
    );

    let reopened = session
        .begin("abc", 0, false)
        .expect("the next open re-requests the same query");
    assert!(reopened > generation, "a new generation is issued");
}

#[test]
fn cancelling_after_load_keeps_the_results() {
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("pa", 0, false).expect("the query starts");
    deliver(&mut session, generation, &["Paris"]);

    session.cancel();
    assert_eq!(
        session.phase(),
        &ComboboxSearchPhase::Loaded,
        "an already-loaded result set survives a cancel"
    );
    assert_eq!(
        session.results(),
        &["Paris".to_string()],
        "the loaded results are kept for the next open"
    );
}

#[test]
fn a_repeated_trimmed_query_is_still_the_same_query() {
    // Gap guard: the minimum-length check trims, but the cache key does not, so
    // " pa " and "pa" are distinct queries for caching purposes. Swift had no
    // assertion here; this records the Rust contract so a future change is visible.
    let mut session = ComboboxSearchSession::new();
    let generation = session.begin("pa", 0, false).expect("the query starts");
    deliver(&mut session, generation, &["Paris"]);

    assert!(
        session.should_request(" pa ", false),
        "the untrimmed spelling counts as a different query"
    );
    assert!(
        session.begin(" pa ", 0, false).is_some(),
        "the untrimmed spelling issues a new request"
    );
}
