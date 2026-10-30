//! Behaviour tests for vector format/parse and clamping.
//!
//! Translation of `Primitives/Controls/Fields/Numeric/Vec3Field.swift`. Parsing a
//! pasted vector is user-facing: the three accepted shapes (`1, 2, 3`,
//! `(1 2 3)`, `Vector3(1, 2, 3)`) and the rejection of anything else are the
//! contract.

use std::collections::HashSet;

use cove_ui_compose::primitives::controls::fields::{
    minimum_axis_width, shows_mixed, Vec3, Vec3Axis, Vec3Codec, Vec3Constraints,
};

#[test]
fn formatting_drops_a_trailing_zero_on_whole_numbers() {
    assert_eq!(Vec3Codec::format(Vec3::new(1.0, 2.0, 3.0)), "1, 2, 3");
    assert_eq!(Vec3Codec::format(Vec3::new(1.5, -2.0, 0.0)), "1.5, -2, 0");
}

#[test]
fn formatting_keeps_fractional_precision() {
    let formatted = Vec3Codec::format(Vec3::new(0.25, 1.0 / 3.0, 9.999));
    assert!(formatted.starts_with("0.25, "), "got {formatted}");
}

#[test]
fn every_accepted_paste_shape_parses() {
    for text in [
        "1, 2, 3",
        "1 2 3",
        "1;2;3",
        "(1 2 3)",
        "[1, 2, 3]",
        "{1,2,3}",
        "Vector3(1, 2, 3)",
        "  SIMD3<Float>(1, 2, 3)  ",
        "1,2,3",
    ] {
        assert_eq!(
            Vec3Codec::parse(text),
            Some(Vec3::new(1.0, 2.0, 3.0)),
            "should parse {text:?}"
        );
    }
}

#[test]
fn negative_and_fractional_components_parse() {
    assert_eq!(
        Vec3Codec::parse("(-1.5, 0.25, -3)"),
        Some(Vec3::new(-1.5, 0.25, -3.0))
    );
}

#[test]
fn anything_other_than_three_numbers_is_rejected() {
    for text in ["", "1, 2", "1, 2, 3, 4", "a, b, c", "1, x, 3", "()"] {
        assert_eq!(Vec3Codec::parse(text), None, "should reject {text:?}");
    }
}

#[test]
fn a_non_finite_component_is_rejected() {
    // `f32` parses `inf`, but a vector must be finite.
    assert_eq!(Vec3Codec::parse("1, inf, 3"), None);
    assert_eq!(Vec3Codec::parse("1, NaN, 3"), None);
}

#[test]
fn brackets_in_the_wrong_order_are_not_stripped() {
    // A closing bracket before an opening one leaves the text untouched, so it
    // fails the three-number test rather than silently parsing something odd.
    assert_eq!(Vec3Codec::parse(")1, 2, 3("), None);
}

#[test]
fn format_round_trips_through_parse() {
    let original = Vec3::new(-1.5, 2.0, 0.125);
    let text = Vec3Codec::format(original);
    assert_eq!(Vec3Codec::parse(&text), Some(original));
}

#[test]
fn writing_clamps_each_axis_and_reports_only_changes() {
    let constraints = Vec3Constraints { minimum: Some(0.0), maximum: Some(10.0), reset: None };
    let current = Vec3::new(1.0, 2.0, 3.0);
    // X stays, Y rises past the maximum, Z drops below the minimum.
    let changes = constraints.write(current, Vec3::new(1.0, 50.0, -5.0));
    assert_eq!(changes, vec![(Vec3Axis::Y, 10.0), (Vec3Axis::Z, 0.0)]);
}

#[test]
fn writing_every_axis_is_a_no_op_when_nothing_changes() {
    let constraints = Vec3Constraints::default();
    let current = Vec3::new(1.0, 2.0, 3.0);
    assert!(constraints.write(current, current).is_empty());
}

#[test]
fn unbounded_constraints_pass_values_through() {
    let constraints = Vec3Constraints::default();
    assert_eq!(constraints.clamp(-1e9), -1e9);
    assert_eq!(constraints.clamp(1e9), 1e9);
}

#[test]
fn mixed_axes_match_on_the_lowercase_key() {
    let mixed: HashSet<String> = ["x".to_owned(), "z".to_owned()].into_iter().collect();
    assert!(shows_mixed(&mixed, Vec3Axis::X));
    assert!(!shows_mixed(&mixed, Vec3Axis::Y));
    assert!(shows_mixed(&mixed, Vec3Axis::Z));
    assert_eq!(Vec3Axis::X.label(), "X");
    assert_eq!(Vec3Axis::X.key(), "x");
}

#[test]
fn the_axis_minimum_width_is_scaled_and_floored() {
    // Below the 44pt floor, the floor wins.
    assert_eq!(minimum_axis_width(20.0, 1.0), 44.0);
    // Above it, the authored width is scaled.
    assert_eq!(minimum_axis_width(88.0, 2.0), 176.0);
}
