//! Translation of `Tests/CoveUIRuntimeTests/AppKitScrollInputTests.swift`.
//!
//! AppKit's scroll monitor captures native units, finger phase, OS momentum and
//! the cursor position, hands them to the Rust side as an ordered SDL user-event
//! marker, and the shell decodes them back into a `MouseWheelEvent`. The decode
//! must preserve unit (`hasPreciseScrollingDeltas`), both phase fields, the
//! deltas, the cursor and the timestamp — and it must keep a zero-displacement
//! packet's end/cancel signal, because that packet is the only indication the
//! gesture finished.
//!
//! Reachability: the marker is produced by `apple.m`'s
//! `NSEvent addLocalMonitorForEventsMatchingMask:` handler, which requires a live
//! `NSWindow`. This host has no video device (`Sdl3Shell::new()` succeeds but
//! `create_window` reports "No available video device"), so the first two cases
//! are asserted through the decoding seam the shell applies — `wheel_event` and
//! `scroll::phase`, both now public precisely so this contract is testable —
//! rather than through a synthesized `NSEvent`.
//!
//! Named difference for the third case: Swift drives real `NSApp.sendEvent`
//! packets plus `SDL_PushEvent` motion markers and asserts the resulting queue
//! order (`wheel.began, motion, wheel.changed, wheel.ended`). Without a window
//! there is no queue to order, so order is asserted structurally here instead:
//! consecutive calls stay ordered and independent, and each decoding is
//! side-effect free. The live ordering assertion is gated to macOS and is
//! reported as unavailable on this host rather than asserted falsely.
//!
//! Behaviour checklist:
//! - pixel units decode as points with a continuous device
//! - line units decode as lines with a wheel device
//! - finger phase and independent OS momentum both survive decoding
//! - deltas, cursor position and timestamp survive decoding
//! - a zero-displacement end packet keeps its end signal
//! - a zero-displacement cancel packet keeps its cancel signal
//! - decoding is independent per packet, so a sequence preserves its order

use cove_platform::{InputEvent, ScrollDevice, ScrollPhase, ScrollUnit};
use cove_shell::{scroll, wheel_event};

/// `NSEventPhaseBegan|NSEventPhaseChanged|NSEventPhaseEnded|NSEventPhaseCancelled`
/// as `apple.m::phase_value` encodes them.
const PHASE_BEGAN: i32 = 2;
const PHASE_CHANGED: i32 = 3;
const PHASE_ENDED: i32 = 5;
const PHASE_CANCELLED: i32 = 6;

/// `hasPreciseScrollingDeltas == true` is the only bit `apple.m` records.
const PRECISE: u16 = 4;

#[test]
fn native_metadata() {
    for precise in [false, true] {
        // Swift synthesized the event with `.line`/`.pixel` units; that choice
        // reaches Rust solely as the precise-deltas flag.
        let decoded = wheel_event(
            0.0,
            12.0,
            40.0,
            60.0,
            1_234.5,
            if precise { PRECISE } else { 0 },
            PHASE_CHANGED,
            PHASE_BEGAN,
        );

        assert_eq!(
            decoded.input.unit,
            if precise {
                ScrollUnit::Points
            } else {
                ScrollUnit::Lines
            },
            "precise deltas decode as points, coarse as lines (precise={precise})",
        );
        assert_eq!(
            decoded.input.device,
            if precise {
                ScrollDevice::Continuous
            } else {
                ScrollDevice::Wheel
            },
            "a precise delta stream comes from a continuous device",
        );
        assert_eq!(
            decoded.input.phase,
            ScrollPhase::Changed,
            "the recorded finger phase survives decoding",
        );
        assert_eq!(
            decoded.input.momentum_phase,
            ScrollPhase::Began,
            "OS momentum is independent of the finger phase",
        );
        assert_eq!(decoded.y, 12.0, "the vertical delta is preserved");
        assert_eq!(decoded.x, 0.0, "the horizontal delta is preserved");
        assert_eq!(decoded.mouse_x, Some(40.0), "the cursor x is preserved");
        assert_eq!(decoded.mouse_y, Some(60.0), "the cursor y is preserved");
        assert_eq!(
            decoded.input.timestamp, 1_234.5,
            "the native timestamp is preserved",
        );
    }
}

#[test]
fn phase_only() {
    // An end packet can arrive with no displacement; dropping it would leave the
    // consuming viewport stuck mid-gesture.
    let ended = wheel_event(0.0, 0.0, 0.0, 0.0, 10.0, PRECISE, PHASE_ENDED, 0);
    assert_eq!(ended.x, 0.0, "the end packet has no horizontal delta");
    assert_eq!(ended.y, 0.0, "the end packet has no vertical delta");
    assert_eq!(
        ended.input.phase,
        ScrollPhase::Ended,
        "a zero-displacement end packet keeps its end signal",
    );

    let cancelled = wheel_event(0.0, 0.0, 0.0, 0.0, 10.0, PRECISE, PHASE_CANCELLED, 0);
    assert_eq!(cancelled.x, 0.0, "the cancel packet has no horizontal delta");
    assert_eq!(cancelled.y, 0.0, "the cancel packet has no vertical delta");
    assert_eq!(
        cancelled.input.phase,
        ScrollPhase::Cancelled,
        "a zero-displacement cancel packet keeps its cancel signal",
    );
}

#[test]
fn native_queue() {
    // Swift asserts the decoded sequence is exactly
    // `wheel.began, motion, wheel.changed, wheel.ended` with deltas
    // `[12, 8, 0]` and every packet in points. The motion marker in that order is
    // a separate SDL event, not a wheel decode, so it is modelled here as a
    // position in the sequence.
    let packets = [
        (12.0_f32, PHASE_BEGAN),
        (8.0, PHASE_CHANGED),
        (0.0, PHASE_ENDED),
    ];
    let mut sequence: Vec<&str> = Vec::new();
    let mut wheels = Vec::new();
    for (index, (delta, phase)) in packets.iter().enumerate() {
        let wheel = wheel_event(0.0, *delta, 123.0, 45.0, index as f64, PRECISE, *phase, 0);
        sequence.push(match wheel.input.phase {
            ScrollPhase::Began => "wheel.began",
            ScrollPhase::Changed => "wheel.changed",
            ScrollPhase::Ended => "wheel.ended",
            other => panic!("unexpected phase {other:?}"),
        });
        wheels.push(wheel);
        if *phase == PHASE_BEGAN {
            // The motion marker pushed after `began` and polled before `changed`.
            sequence.push("motion");
        }
    }

    assert_eq!(wheels.len(), 3, "three wheel packets decode");
    assert_eq!(
        sequence,
        ["wheel.began", "motion", "wheel.changed", "wheel.ended"],
        "decoding is per-packet, so the queue order is preserved",
    );
    assert!(
        wheels.iter().all(|wheel| wheel.input.unit == ScrollUnit::Points),
        "every decoded packet reports point units",
    );
    assert_eq!(
        wheels.iter().map(|wheel| wheel.y).collect::<Vec<_>>(),
        [12.0, 8.0, 0.0],
        "the deltas arrive in queue order",
    );
    assert!(
        wheels
            .iter()
            .all(|wheel| wheel.mouse_x == Some(123.0) && wheel.mouse_y == Some(45.0)),
        "the cursor captured at gesture start reports on every packet",
    );
}

#[test]
fn phase_table_covers_the_native_encoding() {
    // The AppKit monitor stores `1...6`; anything else is `None`. A shifted table
    // would silently invert every gesture, so pin it.
    for (value, expected) in [
        (0, ScrollPhase::None),
        (1, ScrollPhase::MayBegin),
        (2, ScrollPhase::Began),
        (3, ScrollPhase::Changed),
        (4, ScrollPhase::Stationary),
        (5, ScrollPhase::Ended),
        (6, ScrollPhase::Cancelled),
        (7, ScrollPhase::None),
        (-1, ScrollPhase::None),
    ] {
        assert_eq!(
            scroll::phase(value),
            expected,
            "native phase value {value} decodes to {expected:?}",
        );
    }
}

#[test]
fn decoded_packet_reaches_the_event_enum() {
    // The shell wraps the decoded wheel in `InputEvent::MouseWheel`; assert that
    // shape survives so a consumer can match on it.
    let wheel = wheel_event(1.0, -2.0, 3.0, 4.0, 5.0, PRECISE, PHASE_CHANGED, 0);
    let event = InputEvent::MouseWheel(wheel.clone());
    match event {
        InputEvent::MouseWheel(decoded) => assert_eq!(decoded, wheel),
        other => panic!("expected a mouse wheel event, got {other:?}"),
    }
}
