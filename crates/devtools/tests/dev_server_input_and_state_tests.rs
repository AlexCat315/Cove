//! Translation of `Tests/CoveUIDevToolsTests/DevServerInputAndStateTests.swift`.
//!
//! Behaviour checklist:
//! - wheel metadata survives a wire round-trip and reaches the input bridge
//! - the keyboard bridge maps Web `KeyboardEvent.code` values to USB/HID
//!   scancodes, carries the keycode and modifiers, and preserves repeat
//! - common editing and keypad codes map, and an unknown code yields no event
//!
//! Swift drove these over a real Network.framework WebSocket because the
//! `DevTools` façade owned an async server. Rust's `DevServer` dispatches
//! synchronously (see `dev_server_tests.rs`), so the wire half of the wheel
//! case is exercised directly through JSON, which is the same decode the server
//! performs, and the pointer/state sessions live in `dev_server_tests.rs`.

use cove_devtools::devtools::mirror::InputBridge;
use cove_devtools_protocol::MirrorInput;
use cove_platform::{InputEvent, KeyModifiers};

fn key_event(web_code: &str, key_code: Option<i64>, modifiers: Option<i64>) -> Option<InputEvent> {
    InputBridge::event(&MirrorInput {
        kind: "keyDown".into(),
        x: None,
        y: None,
        delta_x: None,
        delta_y: None,
        button: None,
        key: Some(web_code.into()),
        key_code,
        text: None,
        modifiers,
        click_count: None,
        is_repeat: None,
        scroll: None,
    })
}

fn scancode(web_code: &str) -> Option<u32> {
    match key_event(web_code, None, None) {
        Some(InputEvent::KeyDown(key)) => Some(key.scancode),
        _ => None,
    }
}

#[test]
fn wheel_metadata_survives_wire_decode_and_input_bridge() {
    let scroll = cove_platform::ScrollInput {
        unit: cove_platform::ScrollUnit::Points,
        device: cove_platform::ScrollDevice::Continuous,
        phase: cove_platform::ScrollPhase::Ended,
        momentum_phase: cove_platform::ScrollPhase::Began,
        timestamp: 42.25,
    };
    let payload = serde_json::json!({
        "kind": "wheel",
        "x": 10.0,
        "y": 20.0,
        "deltaX": 0.25,
        "deltaY": -8.5,
        "scroll": scroll,
    });

    let decoded: MirrorInput = serde_json::from_value(payload).expect("decode mirror input");
    let event = InputBridge::event(&decoded).expect("wheel maps to an input event");
    let InputEvent::MouseWheel(wheel) = event else {
        panic!("expected a wheel event, got {event:?}");
    };

    assert_eq!(wheel.input, scroll, "scroll metadata round-trips unchanged");
    assert_eq!(wheel.x, 0.25, "horizontal delta survives the wire");
    assert_eq!(wheel.y, -8.5, "vertical delta survives the wire");
}

#[test]
fn keyboard_input_maps_web_codes_to_scancodes() {
    let Some(InputEvent::KeyDown(down)) = key_event("KeyA", Some(65), Some(8)) else {
        panic!("expected a key down event");
    };
    assert_eq!(down.scancode, 4, "KeyA is USB HID usage 4");
    assert_eq!(down.keycode, 65);
    assert!(down.modifiers.has_gui(), "modifier bit 8 is the GUI key");

    let with_repeat = InputBridge::event(&MirrorInput {
        kind: "keyDown".into(),
        x: None,
        y: None,
        delta_x: None,
        delta_y: None,
        button: None,
        key: Some("KeyA".into()),
        key_code: Some(65),
        text: None,
        modifiers: None,
        click_count: None,
        is_repeat: Some(true),
        scroll: None,
    })
    .expect("key down");
    let InputEvent::KeyDown(repeat) = with_repeat else {
        panic!("expected a key down event");
    };
    assert!(repeat.is_repeat, "repeat flags pass through");

    let Some(InputEvent::KeyUp(up)) = InputBridge::event(&MirrorInput {
        kind: "keyUp".into(),
        x: None,
        y: None,
        delta_x: None,
        delta_y: None,
        button: None,
        key: Some("ArrowLeft".into()),
        key_code: Some(37),
        text: None,
        modifiers: Some(0),
        click_count: None,
        is_repeat: Some(false),
        scroll: None,
    }) else {
        panic!("expected a key up event");
    };
    assert_eq!(up.scancode, 80, "ArrowLeft is USB HID usage 80");
    assert_eq!(up.keycode, 37);
    assert_eq!(up.modifiers, KeyModifiers::default());
}

#[test]
fn keyboard_input_maps_common_editing_codes() {
    assert_eq!(scancode("Slash"), Some(56));
    assert_eq!(scancode("F5"), Some(62));
    assert_eq!(scancode("PageDown"), Some(78));
    assert_eq!(scancode("Numpad0"), Some(98));
    assert_eq!(scancode("NumpadAdd"), Some(87));
    assert!(
        key_event("UnknownKey", Some(0), None).is_none(),
        "an unmapped Web code produces no input event"
    );
}
