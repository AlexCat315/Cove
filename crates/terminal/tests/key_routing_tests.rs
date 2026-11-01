//! Translation of `Terminal/Tests/CoveUITerminalTests/KeyRoutingTests.swift`.
//!
//! Behaviour checklist:
//! - printable keys are left to composed text input
//! - Control chords and editing keys are encoded
//! - Command shortcuts belong to the host except copy and paste
//! - Option composes on macOS and sends ESC elsewhere

use cove_platform::{KeyEvent, KeyModifiers as PlatformModifiers};
use cove_terminal::engine::{KeyModifiers, TerminalKey};
use cove_terminal::view::KeyRouting;

fn route(scancode: u32, modifiers: PlatformModifiers) -> KeyRouting {
    KeyRouting::new(&KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    })
}

#[test]
fn printable_keys_are_left_to_composed_text_input() {
    assert_eq!(
        route(4, PlatformModifiers::default()),
        KeyRouting::Text,
        "a"
    );
    assert_eq!(
        route(4, PlatformModifiers::LEFT_SHIFT),
        KeyRouting::Text,
        "Shift-a is still composed text"
    );
    assert_eq!(
        route(44, PlatformModifiers::default()),
        KeyRouting::Text,
        "space"
    );
    assert_eq!(
        route(30, PlatformModifiers::LEFT_SHIFT),
        KeyRouting::Text,
        "Shift-1 is still composed text"
    );
}

#[test]
fn control_chords_and_editing_keys_are_encoded() {
    assert_eq!(
        route(6, PlatformModifiers::LEFT_CTRL),
        KeyRouting::Encode(TerminalKey {
            scancode: 6,
            modifiers: KeyModifiers::CONTROL,
            text: "c".into(),
            is_repeat: false,
        }),
        "Control-C reaches the encoder"
    );
    assert_eq!(
        route(82, PlatformModifiers::default()),
        KeyRouting::Encode(TerminalKey::new(82)),
        "the up arrow reaches the encoder"
    );
    assert_eq!(
        route(40, PlatformModifiers::default()),
        KeyRouting::Encode(TerminalKey::new(40)),
        "Return reaches the encoder"
    );
    assert_eq!(
        route(43, PlatformModifiers::LEFT_SHIFT),
        KeyRouting::Encode(TerminalKey {
            scancode: 43,
            modifiers: KeyModifiers::SHIFT,
            text: String::new(),
            is_repeat: false,
        }),
        "Shift-Tab reaches the encoder"
    );
}

#[test]
fn command_shortcuts_belong_to_the_host_except_copy_and_paste() {
    assert_eq!(
        route(25, PlatformModifiers::LEFT_GUI),
        KeyRouting::Paste,
        "Cmd-V pastes"
    );
    assert_eq!(
        route(6, PlatformModifiers::LEFT_GUI),
        KeyRouting::Copy,
        "Cmd-C copies the selection"
    );
    assert_eq!(
        route(6, PlatformModifiers::LEFT_GUI | PlatformModifiers::LEFT_ALT),
        KeyRouting::Host,
        "Cmd-Option-C stays with the host"
    );
    assert_eq!(
        route(17, PlatformModifiers::LEFT_GUI),
        KeyRouting::Host,
        "Cmd-W stays with the host"
    );
    assert_eq!(
        route(
            25,
            PlatformModifiers::LEFT_GUI | PlatformModifiers::LEFT_SHIFT
        ),
        KeyRouting::Host,
        "Cmd-Shift-V stays with the host"
    );
}

#[test]
fn option_composes_on_macos_and_sends_escape_elsewhere() {
    #[cfg(target_os = "macos")]
    {
        assert_eq!(
            route(4, PlatformModifiers::LEFT_ALT),
            KeyRouting::Text,
            "macOS Option composes characters"
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(
            route(4, PlatformModifiers::LEFT_ALT),
            KeyRouting::Encode(TerminalKey {
                scancode: 4,
                modifiers: KeyModifiers::OPTION,
                text: "a".into(),
                is_repeat: false,
            }),
            "elsewhere Alt prefixes the key with ESC"
        );
    }
}
