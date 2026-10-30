//! Translation of `PlatformCore/Tests/CovePlatformCoreTests/ScrollInputTests.swift`.
//!
//! Behaviour checklist:
//! - units are converted exactly once: points pass through unchanged, lines
//!   multiply by the line step, pages by the page extent
//! - fractional point movement survives (no rounding to whole lines)
//! - the full phase/metadata set survives an `InputEvent` JSON round trip,
//!   including a zero-delta end packet

use cove_platform::{
    InputEvent, MouseWheelEvent, ScrollDevice, ScrollInput, ScrollPhase, ScrollUnit,
};

fn wheel(x: f32, y: f32) -> MouseWheelEvent {
    MouseWheelEvent { x, y, mouse_x: None, mouse_y: None, input: ScrollInput::default() }
}

#[test]
fn units() {
    let points = MouseWheelEvent {
        x: 0.25,
        y: -12.5,
        mouse_x: None,
        mouse_y: None,
        input: ScrollInput { unit: ScrollUnit::Points, ..Default::default() },
    };
    assert_eq!(points.displacement(30.0, 0.0, 0.0).0, 0.25);
    assert_eq!(points.displacement(60.0, 0.0, 0.0).1, -12.5);

    let lines = wheel(0.5, -2.0);
    assert_eq!(lines.displacement(30.0, 0.0, 0.0).0, 15.0);
    assert_eq!(lines.displacement(30.0, 0.0, 0.0).1, -60.0);

    let pages = MouseWheelEvent {
        x: 1.0,
        y: -1.0,
        mouse_x: None,
        mouse_y: None,
        input: ScrollInput { unit: ScrollUnit::Pages, ..Default::default() },
    };
    assert_eq!(pages.displacement(30.0, 200.0, 150.0).0, 200.0);
    assert_eq!(pages.displacement(30.0, 200.0, 150.0).1, -150.0);
}

#[test]
fn serialization() {
    let metadata = ScrollInput {
        unit: ScrollUnit::Points,
        device: ScrollDevice::Continuous,
        phase: ScrollPhase::Ended,
        momentum_phase: ScrollPhase::Began,
        timestamp: 123.5,
    };
    let event = InputEvent::MouseWheel(MouseWheelEvent {
        x: 0.0,
        y: 0.0,
        mouse_x: Some(40.0),
        mouse_y: Some(60.0),
        input: metadata,
    });

    let encoded = serde_json::to_string(&event).expect("wheel event serialises");
    let decoded: InputEvent = serde_json::from_str(&encoded).expect("wheel event round trips");
    let InputEvent::MouseWheel(wheel) = decoded else {
        panic!("decoded event is still a wheel event, got {decoded:?}")
    };
    assert_eq!(wheel.input, metadata, "every phase field survives the round trip");
    assert_eq!((wheel.mouse_x, wheel.mouse_y), (Some(40.0), Some(60.0)));
    assert_eq!((wheel.x, wheel.y), (0.0, 0.0), "a zero-delta end packet stays zero");
}
