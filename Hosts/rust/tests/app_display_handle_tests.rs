//! Translation of `Tests/CoveUITests/AppDisplayHandleTests.swift`.
//!
//! Behaviour checklist:
//! - repeated display requests coalesce to one event-loop wake

use cove_hosts::AppDisplayHandle;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn coalesces_display_wakes() {
    let wakes = Rc::new(Cell::new(0));
    let counter = wakes.clone();
    let display = AppDisplayHandle::new(move || counter.set(counter.get() + 1));

    display.request_display();
    display.request_display();
    display.request_display();

    assert_eq!(wakes.get(), 1, "a burst of requests wakes the loop once");
    assert!(display.drain_display_request(), "a wake was owed");
    assert!(
        !display.drain_display_request(),
        "draining clears the pending request"
    );

    display.request_display();
    assert_eq!(wakes.get(), 2, "the next request after a drain wakes again");
}
