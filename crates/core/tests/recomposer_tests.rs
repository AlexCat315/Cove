//! Translation of `Tests/CoveUIRuntimeTests/RecomposerTests.swift`.
//!
//! Swift keys scopes by `ObjectIdentifier(node)`; the Rust `Recomposer` uses a
//! `ScopeId` token, so each test names a scope id directly.
//!
//! Behaviour checklist:
//! - `commit_all` executes closures in registration order
//! - a duplicate scope id is dropped within the same frame
//! - `commit_all` clears the queue
//! - `commit_all` drains child invalidations queued by parent recomposes
//! - `commit_all` leaves self-invalidations queued for the next frame
//! - a queued child does not gain a duplicate invalidation
//! - the same scope id is accepted again in the next frame
//! - a state write wired into the recomposer only commits on `commit_all`
//! - a binding setter reaches the state
//! - `Binding::constant` ignores writes silently

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::Recomposer;
use cove_core::{Binding, State};

#[test]
fn commit_order() {
    let recomposer = Recomposer::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let first = log.clone();
    let second = log.clone();

    recomposer.invalidate(1, None, None, move || first.borrow_mut().push(1));
    recomposer.invalidate(2, None, None, move || second.borrow_mut().push(2));
    recomposer.commit_all();

    assert_eq!(*log.borrow(), [1, 2], "scopes commit in registration order");
}

#[test]
fn deduplication() {
    let recomposer = Recomposer::new();
    let calls = Rc::new(RefCell::new(0));
    let counter = calls.clone();

    recomposer.invalidate(1, None, None, move || *counter.borrow_mut() += 1);
    // The same id is already queued, so this body is dropped.
    recomposer.invalidate(1, None, None, || {
        panic!("a duplicate scope must be dropped")
    });
    recomposer.commit_all();

    assert_eq!(*calls.borrow(), 1);
}

#[test]
fn commit_clears() {
    let recomposer = Recomposer::new();
    recomposer.invalidate(1, None, None, || {});
    assert!(recomposer.has_pending());
    recomposer.commit_all();
    assert!(!recomposer.has_pending(), "commit_all clears the queue");
}

#[test]
fn commit_drains_nested_invalidations() {
    let recomposer = Rc::new(Recomposer::new());
    let log = Rc::new(RefCell::new(Vec::new()));
    let parent_log = log.clone();
    let child_log = log.clone();
    let child_composer = recomposer.clone();

    recomposer.invalidate(1, None, None, move || {
        parent_log.borrow_mut().push(1);
        let child_log = child_log.clone();
        child_composer.invalidate(2, None, None, move || child_log.borrow_mut().push(2));
    });
    recomposer.commit_all();

    assert_eq!(
        *log.borrow(),
        [1, 2],
        "a child queued by the parent commits in the same frame"
    );
    assert!(!recomposer.has_pending());
}

#[test]
fn commit_defers_self_invalidation() {
    let recomposer = Rc::new(Recomposer::new());
    let calls = Rc::new(RefCell::new(0));
    // The body re-queues itself once; that deferred run does not queue again.
    let outer_counter = calls.clone();
    let inner_counter = calls.clone();
    let self_composer = recomposer.clone();

    recomposer.invalidate(1, None, None, move || {
        *outer_counter.borrow_mut() += 1;
        // A scope invalidating itself is deferred to the next frame.
        let inner = inner_counter.clone();
        self_composer.invalidate(1, None, None, move || *inner.borrow_mut() += 1);
    });
    recomposer.commit_all();

    assert_eq!(
        *calls.borrow(),
        1,
        "the self-invalidation is deferred, not run inline"
    );
    assert!(
        recomposer.has_pending(),
        "the self-invalidation stays queued"
    );

    recomposer.commit_all();
    assert_eq!(
        *calls.borrow(),
        2,
        "the deferred body runs on the next frame"
    );
    assert!(
        !recomposer.has_pending(),
        "that deferred run queued nothing further"
    );
}

#[test]
fn commit_drops_duplicate_for_already_queued_child() {
    let recomposer = Rc::new(Recomposer::new());
    let log = Rc::new(RefCell::new(Vec::new()));
    let parent_log = log.clone();
    let child_log = log.clone();
    let skipped = log.clone();
    let child_composer = recomposer.clone();

    recomposer.invalidate(1, None, None, move || {
        parent_log.borrow_mut().push(1);
        let child_log = child_log.clone();
        child_composer.invalidate(2, None, None, move || child_log.borrow_mut().push(3));
    });
    // Already queued, so this body must not replace or duplicate the child.
    recomposer.invalidate(2, None, None, move || skipped.borrow_mut().push(2));
    recomposer.commit_all();

    assert_eq!(
        *log.borrow(),
        [1, 2],
        "the child keeps its first-registered body"
    );
    assert!(!recomposer.has_pending());
}

#[test]
fn scope_reusable_across_frames() {
    let recomposer = Recomposer::new();
    let calls = Rc::new(RefCell::new(0));
    let first = calls.clone();
    let second = calls.clone();

    recomposer.invalidate(1, None, None, move || *first.borrow_mut() += 1);
    recomposer.commit_all();
    recomposer.invalidate(1, None, None, move || *second.borrow_mut() += 1);
    recomposer.commit_all();

    assert_eq!(
        *calls.borrow(),
        2,
        "the same id is accepted again next frame"
    );
}

#[test]
fn state_wires_recomposer() {
    let recomposer = Rc::new(Recomposer::new());
    let state = State::new(0i32);
    let count = Rc::new(RefCell::new(0));
    let counter = count.clone();
    let wired = recomposer.clone();
    let _subscription = state.subscribe_local(move |_revision| {
        let counter = counter.clone();
        wired.invalidate(1, None, None, move || *counter.borrow_mut() += 1);
    });

    state.set(42);

    assert!(recomposer.has_pending(), "the write queued a recompose");
    assert_eq!(*count.borrow(), 0, "nothing ran before commit");

    recomposer.commit_all();
    assert_eq!(*count.borrow(), 1);
    assert!(!recomposer.has_pending());
}

#[test]
fn binding_fires_on_change() {
    let state = State::new("hello");
    let fired = Rc::new(RefCell::new(false));
    let flag = fired.clone();
    let _subscription = state.subscribe_local(move |_revision| *flag.borrow_mut() = true);

    let binding = state.binding();
    binding.set("world");

    assert!(*fired.borrow(), "the binding setter reached the state");
    assert_eq!(state.get(), "world");
}

#[test]
fn binding_constant() {
    let binding = Binding::constant(99);
    binding.set(0);
    assert_eq!(binding.get(), 99, "a constant binding ignores writes");
}
