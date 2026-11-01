//! Translation of CoveUIComposeCore/Scancode.swift.
//!
//! SDL/USB-HID keyboard scancodes shared by compose controls. One named
//! constant per key the components handle — no scattered magic numbers.

pub struct Scancode;
impl Scancode {
    pub const A: u32 = 4;
    pub const B: u32 = 5;
    pub const C: u32 = 6;
    pub const D: u32 = 7;
    pub const E: u32 = 8;
    pub const F: u32 = 9;
    pub const K: u32 = 14;
    pub const L: u32 = 15;
    pub const N: u32 = 17;
    pub const O: u32 = 18;
    pub const P: u32 = 19;
    pub const Q: u32 = 20;
    pub const R: u32 = 21;
    pub const S: u32 = 22;
    pub const T: u32 = 23;
    pub const U: u32 = 24;
    pub const V: u32 = 25;
    pub const W: u32 = 26;
    pub const X: u32 = 27;
    pub const Y: u32 = 28;
    pub const Z: u32 = 29;
    pub const DIGIT0: u32 = 39;
    pub const DIGIT1: u32 = 30;
    pub const DIGIT2: u32 = 31;
    pub const DIGIT3: u32 = 32;
    pub const DIGIT4: u32 = 33;
    pub const DIGIT5: u32 = 34;
    pub const DIGIT6: u32 = 35;
    pub const DIGIT7: u32 = 36;
    pub const DIGIT8: u32 = 37;
    pub const DIGIT9: u32 = 38;
    pub const COMMA: u32 = 54;
    pub const PERIOD: u32 = 55;
    pub const LEFT_BRACKET: u32 = 47;
    pub const RIGHT_BRACKET: u32 = 48;
    pub const RETURN: u32 = 40;
    pub const ESCAPE: u32 = 41;
    pub const BACKSPACE: u32 = 42;
    pub const TAB: u32 = 43;
    pub const SPACE: u32 = 44;
    pub const MINUS: u32 = 45;
    pub const EQUALS: u32 = 46;
    pub const HOME: u32 = 74;
    pub const PAGE_UP: u32 = 75;
    pub const DELETE: u32 = 76;
    pub const END: u32 = 77;
    pub const PAGE_DOWN: u32 = 78;
    pub const ARROW_RIGHT: u32 = 79;
    pub const ARROW_LEFT: u32 = 80;
    pub const ARROW_DOWN: u32 = 81;
    pub const ARROW_UP: u32 = 82;
    pub const KEYPAD_ENTER: u32 = 88;
    pub const F1: u32 = 58;
    pub const F2: u32 = 59;
    pub const F9: u32 = 66;
    pub const F12: u32 = 69;
}

/// Unambiguous spelling for clients that also import an engine/runtime module
/// exposing its own SDL scancode namespace.
pub type ComposeScancode = Scancode;
