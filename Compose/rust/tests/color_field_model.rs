//! Behaviour tests for colour hex round-tripping.
//!
//! Translation of `Primitives/Controls/Fields/ColorField.swift`. The hex field is
//! the one place a person types a colour, so the accepted shapes and the
//! alpha-pair position are user-facing contract.

use cove_core::Color;
use cove_ui_compose::primitives::controls::fields::{hex_string, panel_width, parse_hex};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

#[test]
fn a_colour_formats_as_uppercase_hex() {
    let color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    assert_eq!(hex_string(color, false), "#FF0000");
}

#[test]
fn alpha_is_appended_only_when_shown() {
    let color = Color { r: 0.0, g: 0.0, b: 1.0, a: 0.5 };
    assert_eq!(hex_string(color, false), "#0000FF");
    let with_alpha = hex_string(color, true);
    assert_eq!(with_alpha.len(), 9, "8 hex digits plus #, got {with_alpha}");
    assert!(with_alpha.ends_with("80"), "0.5 rounds to 0x80, got {with_alpha}");
}

#[test]
fn channels_clamp_and_round() {
    // Out-of-range values clamp rather than wrapping.
    let bright = Color { r: 2.0, g: -1.0, b: 0.5, a: 1.0 };
    assert_eq!(hex_string(bright, false), "#FF0080");
}

#[test]
fn six_digit_hex_parses_any_case_with_or_without_hash() {
    for text in ["#FF8000", "ff8000", "  #ff8000  ", "FF8000"] {
        let color = parse_hex(text).unwrap_or_else(|| panic!("{text} should parse"));
        assert!(close(color.r, 1.0), "{text}");
        assert!(close(color.g, 128.0 / 255.0), "{text}");
        assert!(close(color.b, 0.0), "{text}");
        assert_eq!(color.a, 1.0, "six digits are opaque");
    }
}

#[test]
fn eight_digit_hex_takes_the_last_pair_as_alpha() {
    let color = parse_hex("#FF000080").expect("8-digit hex parses");
    assert!(close(color.r, 1.0));
    assert!(close(color.g, 0.0));
    assert!(close(color.b, 0.0));
    assert!(close(color.a, 128.0 / 255.0), "alpha is the trailing pair");
}

#[test]
fn other_lengths_are_rejected() {
    for text in ["#FFF", "#FFFFF", "#FF0000000", "", "#"] {
        assert_eq!(parse_hex(text), None, "{text} should not parse");
    }
}

#[test]
fn a_non_hex_character_is_rejected() {
    assert_eq!(parse_hex("#FF00GG"), None);
    assert_eq!(parse_hex("#ff 000"), None);
}

#[test]
fn format_and_parse_round_trip_within_8_bit_quantisation() {
    // Hex carries 8 bits per channel, so a round trip is exact only to 1/510
    // (half a step) — 0.25 formats as 0x40 and reads back as 64/255.
    let original = Color { r: 0.25, g: 0.5, b: 0.75, a: 1.0 };
    let text = hex_string(original, false);
    assert_eq!(text, "#4080BF");
    let parsed = parse_hex(&text).expect("formatted output parses");
    let tolerance = 1.0 / 255.0;
    assert!((parsed.r - original.r).abs() <= tolerance, "r from {text}");
    assert!((parsed.g - original.g).abs() <= tolerance, "g from {text}");
    assert!((parsed.b - original.b).abs() <= tolerance, "b from {text}");
    assert_eq!(parsed.a, 1.0);
}

#[test]
fn the_panel_width_is_clamped_around_the_viewport() {
    // Narrow viewport hits the 176 floor.
    assert_eq!(panel_width(100.0), 176.0);
    // Mid-range tracks the viewport minus the inset.
    assert_eq!(panel_width(250.0), 226.0);
    // Wide viewport hits the 280 ceiling.
    assert_eq!(panel_width(900.0), 280.0);
}
