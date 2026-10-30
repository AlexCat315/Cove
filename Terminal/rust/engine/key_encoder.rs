//! Key press encoding.
//!
//! A physical key press as the terminal sees it: `scancode` is the USB HID usage
//! id (SDL scancode) and `text` is what the key would type unmodified. Ghostty's
//! encoder turns it into the bytes the terminal's live modes call for
//! (application cursor keys, keypad mode, Kitty keyboard flags, ...).

use crate::engine::ffi::{self, GhosttyKeyEncoder, GhosttyKeyEvent};
use std::ptr;

/// Keyboard modifiers as the terminal cares about them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct KeyModifiers(pub u8);

impl KeyModifiers {
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const OPTION: Self = Self(1 << 2);
    pub const COMMAND: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn to_ghostty(self) -> ffi::GhosttyMods {
        let mut mods: ffi::GhosttyMods = 0;
        if self.contains(Self::SHIFT) {
            mods |= ffi::MODS_SHIFT;
        }
        if self.contains(Self::CONTROL) {
            mods |= ffi::MODS_CTRL;
        }
        if self.contains(Self::OPTION) {
            mods |= ffi::MODS_ALT;
        }
        if self.contains(Self::COMMAND) {
            mods |= ffi::MODS_SUPER;
        }
        mods
    }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A physical key press as the terminal sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TerminalKey {
    pub scancode: u32,
    pub modifiers: KeyModifiers,
    pub text: String,
    pub is_repeat: bool,
}

impl TerminalKey {
    pub fn new(scancode: u32) -> Self {
        Self { scancode, ..Self::default() }
    }
}

/// USB HID usage id -> W3C key code, as Ghostty names them.
fn ghostty_key(scancode: u32) -> Option<i32> {
    use ffi::key::*;
    const LETTERS: [i32; 26] = [A, A + 1, A + 2, A + 3, A + 4, A + 5, A + 6, A + 7, A + 8, A + 9, A + 10, A + 11, A + 12, A + 13, A + 14, A + 15, A + 16, A + 17, A + 18, A + 19, A + 20, A + 21, A + 22, A + 23, A + 24, A + 25];
    const DIGITS: [i32; 10] = [DIGIT_1, DIGIT_2, DIGIT_3, DIGIT_4, DIGIT_5, DIGIT_6, DIGIT_7, DIGIT_8, DIGIT_9, DIGIT_0];
    const FUNCTIONS: [i32; 12] = [F1, F1 + 1, F1 + 2, F1 + 3, F1 + 4, F1 + 5, F1 + 6, F1 + 7, F1 + 8, F1 + 9, F1 + 10, F1 + 11];
    const KEYPAD: [i32; 16] = [
        NUMPAD_DIVIDE,
        NUMPAD_MULTIPLY,
        NUMPAD_SUBTRACT,
        NUMPAD_ADD,
        NUMPAD_ENTER,
        NUMPAD_1,
        NUMPAD_2,
        NUMPAD_3,
        NUMPAD_4,
        NUMPAD_5,
        NUMPAD_6,
        NUMPAD_7,
        NUMPAD_8,
        NUMPAD_9,
        NUMPAD_0,
        NUMPAD_DECIMAL,
    ];
    // Scancodes that do not form a regular run, listed explicitly.
    const NAMED: [(u32, i32); 29] = [
        (40, ENTER),
        (41, ESCAPE),
        (42, BACKSPACE),
        (43, TAB),
        (44, SPACE),
        (45, MINUS),
        (46, EQUAL),
        (47, BRACKET_LEFT),
        (48, BRACKET_RIGHT),
        (49, BACKSLASH),
        (51, SEMICOLON),
        (52, QUOTE),
        (53, BACKQUOTE),
        (54, COMMA),
        (55, PERIOD),
        (56, SLASH),
        (73, INSERT),
        (74, HOME),
        (75, PAGE_UP),
        (76, DELETE),
        (77, END),
        (78, PAGE_DOWN),
        (79, ARROW_RIGHT),
        (80, ARROW_LEFT),
        (81, ARROW_DOWN),
        (82, ARROW_UP),
        (100, INTL_BACKSLASH),
        (103, NUMPAD_EQUAL),
        (u32::MAX, UNIDENTIFIED),
    ];
    if (4..=29).contains(&scancode) {
        return Some(LETTERS[(scancode - 4) as usize]);
    }
    if (30..=39).contains(&scancode) {
        return Some(DIGITS[(scancode - 30) as usize]);
    }
    if (58..=69).contains(&scancode) {
        return Some(FUNCTIONS[(scancode - 58) as usize]);
    }
    if (84..=99).contains(&scancode) {
        return Some(KEYPAD[(scancode - 84) as usize]);
    }
    NAMED.iter().find(|(code, _)| *code == scancode).map(|(_, key)| *key)
}

/// Wraps a Ghostty key encoder so the bytes follow the terminal's live modes.
pub(crate) struct KeyEncoder {
    encoder: GhosttyKeyEncoder,
    event: GhosttyKeyEvent,
}

impl KeyEncoder {
    pub(crate) fn new() -> Self {
        // SAFETY: both allocations are owned by this value and freed in `Drop`.
        unsafe {
            let mut encoder: GhosttyKeyEncoder = ptr::null_mut();
            let mut event: GhosttyKeyEvent = ptr::null_mut();
            let encoder_ok = ffi::ghostty_key_encoder_new(ptr::null(), &mut encoder) == ffi::GHOSTTY_SUCCESS;
            let event_ok = ffi::ghostty_key_event_new(ptr::null(), &mut event) == ffi::GHOSTTY_SUCCESS;
            Self { encoder: if encoder_ok { encoder } else { ptr::null_mut() }, event: if event_ok { event } else { ptr::null_mut() } }
        }
    }

    pub(crate) fn encode(&self, key: &TerminalKey, terminal: ffi::GhosttyTerminal) -> Vec<u8> {
        if self.encoder.is_null() || self.event.is_null() {
            return Vec::new();
        }
        let Some(code) = ghostty_key(key.scancode) else {
            return Vec::new();
        };
        let text = key.text.as_bytes().to_vec();
        // SAFETY: the encoder and event are live; `text` outlives the call and
        // the event's borrowed pointer is cleared before `text` is dropped.
        unsafe {
            ffi::ghostty_key_encoder_setopt_from_terminal(self.encoder, terminal);
            ffi::ghostty_key_event_set_action(
                self.event,
                if key.is_repeat { ffi::KEY_ACTION_REPEAT } else { ffi::KEY_ACTION_PRESS },
            );
            ffi::ghostty_key_event_set_key(self.event, code);
            ffi::ghostty_key_event_set_mods(self.event, key.modifiers.to_ghostty());
            ffi::ghostty_key_event_set_consumed_mods(self.event, 0);
            ffi::ghostty_key_event_set_composing(self.event, false);
            ffi::ghostty_key_event_set_unshifted_codepoint(
                self.event,
                key.text.chars().next().map(u32::from).unwrap_or(0),
            );
            ffi::ghostty_key_event_set_utf8(self.event, text.as_ptr(), text.len());
            let mut buffer = vec![0u8; 128];
            let mut written = 0usize;
            let mut result = ffi::ghostty_key_encoder_encode(
                self.encoder,
                self.event,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut written,
            );
            if result == ffi::GHOSTTY_OUT_OF_SPACE {
                buffer = vec![0u8; written];
                result = ffi::ghostty_key_encoder_encode(
                    self.encoder,
                    self.event,
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut written,
                );
            }
            // The event borrows `text`; drop the pointer before it dies.
            ffi::ghostty_key_event_set_utf8(self.event, ptr::null(), 0);
            if result != ffi::GHOSTTY_SUCCESS {
                return Vec::new();
            }
            buffer.truncate(written);
            buffer
        }
    }
}

impl Drop for KeyEncoder {
    fn drop(&mut self) {
        // SAFETY: both handles were allocated by the library and are freed once.
        unsafe {
            if !self.event.is_null() {
                ffi::ghostty_key_event_free(self.event);
            }
            if !self.encoder.is_null() {
                ffi::ghostty_key_encoder_free(self.encoder);
            }
        }
    }
}
