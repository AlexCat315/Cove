//! Translation of `Tests/CoveUIRuntimeTests/FrameDeadlineTests.swift`.
//!
//! Behaviour checklist:
//! - fixed deadlines retain their phase across late work and early input wakes
//! - a blocked present consumes the deadline without a new full-period wait
//! - changing rates or leaving cadence resets the pending deadline
//! - native callbacks retain only the newest timestamp and coalesce wakeups

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cove_runtime::FrameDeadline;
use cove_shell::display_refresh::{DisplayRefreshInbox, DisplayRefreshTick};

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-9
}

#[test]
fn absolute_phase() {
    let mut cadence = FrameDeadline::default();
    cadence.update(Some(0.01), 100.0);
    let first_due = cadence.consume_if_due(100.0);
    let early_due = cadence.consume_if_due(100.004);
    assert!(first_due, "the first due time is immediate");
    assert!(
        !early_due,
        "an early input wake does not consume the deadline"
    );
    assert!(close(cadence.next.expect("a pending deadline"), 100.01));

    let late_due = cadence.consume_if_due(100.027);
    assert!(late_due, "late work still consumes the deadline");
    assert!(
        close(cadence.next.expect("a pending deadline"), 100.03),
        "the phase is retained across late work",
    );

    let repeated_due = cadence.consume_if_due(100.027);
    assert!(
        !repeated_due,
        "a second consume at the same instant does not fire"
    );
    assert!(close(
        cadence.remaining(100.028).expect("a remaining time"),
        0.002
    ));
}

#[test]
fn presentation_overrun() {
    let mut cadence = FrameDeadline::default();
    cadence.update(Some(1.0 / 60.0), 100.0);
    assert!(
        cadence.consume_if_due(100.0),
        "the first frame is due immediately"
    );
    assert_eq!(
        cadence.remaining(100.020),
        Some(0.0),
        "the deadline is already past"
    );

    assert!(
        cadence.consume_if_due(100.020),
        "a blocked present still consumes the deadline",
    );
    assert!(
        close(
            cadence.next.expect("a pending deadline"),
            100.0 + 2.0 / 60.0
        ),
        "the next deadline advances by whole periods, not from the overrun instant",
    );
}

#[test]
fn reconfiguration() {
    let mut cadence = FrameDeadline::default();
    cadence.update(Some(0.01), 100.0);
    assert!(cadence.consume_if_due(100.0));

    cadence.update(Some(0.02), 100.004);
    assert!(
        cadence.consume_if_due(100.004),
        "a new rate starts a new phase"
    );
    assert!(close(cadence.next.expect("a pending deadline"), 100.024));

    cadence.update(None, 100.005);
    assert!(
        cadence.next.is_none(),
        "leaving cadence clears the deadline"
    );
    assert!(
        !cadence.consume_if_due(200.0),
        "no deadline means nothing is ever due"
    );
}

#[test]
fn native_callback_coalescing() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    let inbox = DisplayRefreshInbox::new(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    let first = DisplayRefreshTick {
        timestamp: 100.0,
        target_timestamp: 100.016,
    };
    let latest = DisplayRefreshTick {
        timestamp: 100.032,
        target_timestamp: 100.048,
    };

    inbox.publish(first);
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        0,
        "a disabled inbox publishes nothing"
    );

    inbox.set_enabled(true);
    inbox.publish(first);
    inbox.publish(latest);
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        1,
        "a second tick coalesces into the pending wake"
    );
    assert_eq!(
        inbox.take(),
        Some(latest),
        "only the newest tick is retained"
    );
    assert_eq!(inbox.take(), None, "taking clears the slot");

    inbox.publish(first);
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        2,
        "a tick after an empty slot wakes again"
    );

    inbox.set_enabled(false);
    assert_eq!(inbox.take(), None, "disabling clears the pending tick");
    inbox.publish(latest);
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        2,
        "a disabled inbox stays silent"
    );

    inbox.set_enabled(true);
    assert_eq!(
        inbox.take(),
        None,
        "re-enabling does not resurrect the old tick"
    );
}
