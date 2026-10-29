//! Translation of Tests/Headless/Tests/CoveUISceneTests/Scrolling/RubberBandTests.swift.
use cove_core::scene::RubberBand;

#[test]
fn first_point_past_the_bound_travels_at_the_configured_compliance() {
    let first = RubberBand::project(1.0, 100.0, 0.55);
    assert!((first - 0.55).abs() < 0.01);
    let stiff = RubberBand::project(1.0, 100.0, 0.2);
    assert!(stiff < first);
}

#[test]
fn growth_is_monotonic_but_always_shorter_than_the_viewport_extent() {
    let mut previous = -1.0_f32;
    let mut raw = 0.0_f32;
    while raw <= 2_000.0 {
        if raw > 0.0 {
            let visible = RubberBand::project(raw, 120.0, 0.55);
            assert!(visible > previous);
            assert!(visible < 120.0);
            previous = visible;
        }
        raw += 50.0;
    }
}

#[test]
fn compression_is_symmetric_around_both_bounds() {
    let down = RubberBand::project(60.0, 100.0, 0.55);
    let up = RubberBand::project(-60.0, 100.0, 0.55);
    assert!(down > 0.0);
    assert_eq!(up, -down);
}

#[test]
fn no_viewport_or_zero_compliance_leaves_nothing_to_stretch_against() {
    assert_eq!(RubberBand::project(40.0, 0.0, 0.55), 0.0);
    assert_eq!(RubberBand::project(0.0, 100.0, 0.55), 0.0);
    assert_eq!(RubberBand::project(40.0, 100.0, 0.0), 0.0);
}

#[test]
fn unproject_inverts_project_including_near_the_asymptote() {
    for raw in [1.0_f32, 5.0, 40.0, 120.0, 400.0] {
        let visible = RubberBand::project(raw, 150.0, 0.55);
        let recovered = RubberBand::unproject(visible, 150.0, 0.55);
        assert!((recovered - raw).abs() < 0.01);
    }
}

#[test]
fn unproject_refuses_displacements_the_curve_can_never_reach() {
    assert_eq!(RubberBand::unproject(0.0, 100.0, 0.55), 0.0);
    assert_eq!(RubberBand::unproject(100.0, 100.0, 0.55), 0.0);
    assert_eq!(RubberBand::unproject(140.0, 100.0, 0.55), 0.0);
}

#[test]
fn excess_reports_the_signed_overflow_of_both_bounds() {
    assert_eq!(RubberBand::excess(-12.0, 100.0), -12.0);
    assert_eq!(RubberBand::excess(40.0, 100.0), 0.0);
    assert_eq!(RubberBand::excess(112.0, 100.0), 12.0);
}
