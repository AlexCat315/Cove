//! Translation of `Tests/CoveUIComposeTests/KeyboardShortcutTests.swift`.
//!
//! Behaviour checklist:
//! - the primary modifier renders as Command on macOS
//! - the primary modifier renders as Control on Windows and Linux
//! - adding shift inserts the shift glyph/word in the right place

mod support;

use cove_ui_compose::primitives::controls::fields::select::{
    KeyboardShortcut, KeyboardShortcutPlatform,
};

#[test]
fn macos_primary_display() {
    assert_eq!(
        KeyboardShortcut::primary("D").display_string_for(KeyboardShortcutPlatform::MacOS),
        "⌘D"
    );
    assert_eq!(
        KeyboardShortcut::primary_shift("T").display_string_for(KeyboardShortcutPlatform::MacOS),
        "⌘⇧T"
    );
}

#[test]
fn non_mac_primary_display() {
    assert_eq!(
        KeyboardShortcut::primary("D").display_string_for(KeyboardShortcutPlatform::Windows),
        "Ctrl+D"
    );
    assert_eq!(
        KeyboardShortcut::primary_shift("T").display_string_for(KeyboardShortcutPlatform::Linux),
        "Ctrl+Shift+T"
    );
}
