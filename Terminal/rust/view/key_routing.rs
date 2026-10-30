//! Decides who turns a key press into bytes.
//!
//! Printable keys arrive twice from the platform: as a key event and as composed
//! text (layout- and IME-aware). The text path owns them, so the key event is
//! swallowed. Everything else - editing keys, arrows, function keys and Control
//! chords - goes through the encoder, which follows the program's keyboard modes.

use crate::engine::{KeyModifiers, TerminalKey};
use cove_platform::{KeyEvent, KeyModifiers as PlatformModifiers};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyRouting {
    /// Consume the key; the matching text event will deliver the bytes.
    Text,
    /// Encode with the terminal's key encoder.
    Encode(TerminalKey),
    /// Leave it to the host (Command shortcuts).
    Host,
    Paste,
    /// Cmd-C: copies the selection, or falls through to the host without one.
    Copy,
}

impl KeyRouting {
    pub fn new(event: &KeyEvent) -> Self {
        let modifiers = event.modifiers;
        if modifiers.has_gui() {
            let plain = !modifiers.has_shift() && !modifiers.has_ctrl() && !modifiers.has_alt();
            return match event.scancode {
                25 if plain => Self::Paste,
                6 if plain => Self::Copy,
                _ => Self::Host,
            };
        }
        let mut terminal = KeyModifiers::empty();
        if modifiers.has_shift() {
            terminal |= KeyModifiers::SHIFT;
        }
        if modifiers.has_ctrl() {
            terminal |= KeyModifiers::CONTROL;
        }
        if modifiers.has_alt() {
            terminal |= KeyModifiers::OPTION;
        }

        let unshifted = unshifted_text(event.scancode);
        if unshifted.is_some() && !modifiers.has_ctrl() && !alt_encodes_escape(modifiers) {
            return Self::Text;
        }
        Self::Encode(TerminalKey {
            scancode: event.scancode,
            modifiers: terminal,
            text: unshifted.unwrap_or_default().to_owned(),
            is_repeat: event.is_repeat,
        })
    }
}

/// macOS Option composes characters (Option-3 is a pound sign on a UK layout), so
/// it stays on the text path; elsewhere Alt sends an ESC prefix.
fn alt_encodes_escape(modifiers: PlatformModifiers) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = modifiers;
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        modifiers.has_alt()
    }
}

/// US-layout characters for keys that produce text, keyed by HID usage id. Used
/// as the key's unshifted codepoint (Control chords, Kitty protocol).
fn unshifted_text(scancode: u32) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t",
        "u", "v", "w", "x", "y", "z",
    ];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    const NAMED: [(u32, &str); 29] = [
        (44, " "),
        (45, "-"),
        (46, "="),
        (47, "["),
        (48, "]"),
        (49, "\\"),
        (51, ";"),
        (52, "'"),
        (53, "`"),
        (54, ","),
        (55, "."),
        (56, "/"),
        (84, "/"),
        (85, "*"),
        (86, "-"),
        (87, "+"),
        (89, "1"),
        (90, "2"),
        (91, "3"),
        (92, "4"),
        (93, "5"),
        (94, "6"),
        (95, "7"),
        (96, "8"),
        (97, "9"),
        (98, "0"),
        (99, "."),
        (100, "\\"),
        (103, "="),
    ];
    if (4..=29).contains(&scancode) {
        return Some(LETTERS[(scancode - 4) as usize]);
    }
    if (30..=39).contains(&scancode) {
        return Some(DIGITS[(scancode - 30) as usize]);
    }
    NAMED.iter().find(|(code, _)| *code == scancode).map(|(_, text)| *text)
}
