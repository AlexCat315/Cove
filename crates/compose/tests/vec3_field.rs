//! Translation of `Tests/CoveUIComposeTests/Vec3FieldTests.swift`.
//!
//! Behaviour checklist:
//! - scrubbing scales the drag by the step, Shift (coarse) and Option (fine),
//!   and clamps at the configured bounds
//! - a vector copies locale-independently and pastes from the common spellings
//! - malformed vectors parse to nothing rather than a partial result
//! - clamping reports only the axes that actually changed
//! - three axes share one row only when each keeps its minimum width
//!
//! Model difference: the Swift suite mounts `Vec3Field` and dispatches pointer
//! events at the axis chips. The scrub arithmetic, codec and constraint halves
//! are the Rust model, so those are asserted directly with the same inputs.

use cove_platform::KeyModifiers;
use cove_ui_compose::primitives::controls::fields::numeric::scrub::NumericScrub;
use cove_ui_compose::primitives::controls::fields::{
    Vec3, Vec3Axis, Vec3Codec, Vec3Constraints, minimum_axis_width, shows_mixed,
};
use std::collections::HashSet;

fn scrub() -> NumericScrub {
    NumericScrub::new(0.25, Some(0.0), |value: f32| value.clamp(-10.0, 10.0))
}

#[test]
fn scrubbing_scales_the_drag_by_the_step_and_modifiers() {
    let scrub = scrub();
    assert_eq!(scrub.value(1.0, 8.0, KeyModifiers(0)), 3.0, "8pt x 0.25");
    assert_eq!(
        scrub.value(1.0, 8.0, KeyModifiers::LEFT_SHIFT),
        10.0,
        "Shift is coarse and the result clamps at the maximum"
    );
    let fine = scrub.value(1.0, 8.0, KeyModifiers::LEFT_ALT);
    assert!((fine - 1.2).abs() < 0.0001, "Option is fine: 8pt x 0.025");
}

#[test]
fn scrubbing_clamps_at_the_configured_bounds() {
    let scrub = scrub();
    assert_eq!(
        scrub.value(1.0, -400.0, KeyModifiers(0)),
        -10.0,
        "a long downward drag stops at the minimum"
    );
    assert_eq!(
        scrub.value(1.0, 400.0, KeyModifiers(0)),
        10.0,
        "a long upward drag stops at the maximum"
    );
}

#[test]
fn a_vector_copies_without_locale_decoration() {
    assert_eq!(Vec3Codec::format(Vec3::new(1.0, -2.5, 0.0)), "1, -2.5, 0");
    assert_eq!(
        Vec3Codec::format(Vec3::new(1.0, 2.0, 3.0)),
        "1, 2, 3",
        "whole numbers print without a trailing .0"
    );
}

#[test]
fn a_vector_pastes_from_the_common_spellings() {
    let expected = Vec3::new(1.0, -2.5, 0.0);
    for text in [
        "1, -2.5, 0",
        "(1 -2.5 0)",
        "Vector3(1, -2.5, 0)",
        " [1;-2.5;0] ",
        "1\t-2.5\t0",
    ] {
        assert_eq!(Vec3Codec::parse(text), Some(expected), "{text}");
    }
}

#[test]
fn malformed_vectors_parse_to_nothing() {
    for text in ["1, 2", "1, 2, 3, 4", "a, b, c", "1, nan, 2", ""] {
        assert_eq!(Vec3Codec::parse(text), None, "{text} is not a vector");
    }
}

#[test]
fn a_vector_round_trips_through_its_own_text() {
    let vector = Vec3::new(1.0, -2.5, 0.0);
    assert_eq!(
        Vec3Codec::parse(&Vec3Codec::format(vector)),
        Some(vector),
        "copy then paste reproduces the value"
    );
}

#[test]
fn clamping_reports_only_the_axes_that_changed() {
    let constraints = Vec3Constraints {
        minimum: Some(0.0),
        maximum: Some(10.0),
        reset: Some(Vec3::new(0.0, 0.0, 0.0)),
    };
    let current = Vec3::new(1.0, 2.0, 3.0);
    // x is over the maximum and z under the minimum; y is already in range.
    let changes = constraints.write(current, Vec3::new(11.0, 2.0, -5.0));
    assert_eq!(
        changes,
        vec![(Vec3Axis::X, 10.0), (Vec3Axis::Z, 0.0)],
        "an unchanged axis is not written back"
    );
}

#[test]
fn constraints_clamp_one_value() {
    let constraints = Vec3Constraints {
        minimum: Some(-1.0),
        maximum: Some(1.0),
        reset: None,
    };
    assert_eq!(constraints.clamp(5.0), 1.0);
    assert_eq!(constraints.clamp(-5.0), -1.0);
    assert_eq!(constraints.clamp(0.5), 0.5);
}

#[test]
fn an_axis_shows_the_mixed_label_only_when_it_is_listed() {
    let mut mixed = HashSet::new();
    mixed.insert(Vec3Axis::Y.key().to_owned());
    assert!(!shows_mixed(&mixed, Vec3Axis::X), "x is uniform");
    assert!(shows_mixed(&mixed, Vec3Axis::Y), "y is mixed");
}

#[test]
fn the_axis_minimum_width_never_drops_below_the_usable_floor() {
    assert_eq!(
        minimum_axis_width(56.0, 1.0),
        56.0,
        "a wide enough minimum passes through"
    );
    assert_eq!(
        minimum_axis_width(10.0, 1.0),
        44.0,
        "a cramped minimum is floored at 44 points"
    );
    assert_eq!(
        minimum_axis_width(56.0, 1.5),
        84.0,
        "the text scale multiplies the resolved minimum"
    );
}
