//! Translation of `Tests/Headless/Tests/CoveUISceneTests/Scrolling/RubberBandTests.swift`.
//!
//! Rubber-banding turns raw overscroll into a visible displacement that grows
//! monotonically, stays shorter than the viewport, is symmetric about both
//! bounds, and can be inverted so a gesture can be replayed.
//!
//! Behaviour checklist:
//! - the first point past the bound travels at the configured compliance
//! - a stiffer band moves proportionally less
//! - growth is monotonic but always shorter than the viewport extent
//! - compression is symmetric around both bounds
//! - no viewport or zero compliance leaves nothing to stretch against
//! - `unproject` inverts `project`, including near the asymptote
//! - `unproject` refuses displacements the curve can never reach
//! - `excess` reports the signed overflow of both bounds

use cove_core::scene::RubberBand;

#[test]
fn initial_slope() {
    let first = RubberBand::project(1.0, 100.0, 0.55);
    assert!(
        (first - 0.55).abs() < 0.01,
        "the first point travels at the compliance, got {first}"
    );

    // A stiffer band moves proportionally less.
    let stiff = RubberBand::project(1.0, 100.0, 0.2);
    assert!(stiff < first, "stiff {stiff} < compliant {first}");
}

#[test]
fn saturates_below_extent() {
    let mut previous = -1.0_f32;
    for step in 0..=40 {
        let raw = step as f32 * 50.0;
        if raw <= 0.0 {
            continue;
        }
        let visible = RubberBand::project(raw, 120.0, 0.55);
        assert!(visible > previous, "overscroll {raw} grows: {visible}");
        assert!(
            visible < 120.0,
            "overscroll {raw} stays below extent: {visible}"
        );
        previous = visible;
    }
}

#[test]
fn symmetric() {
    let down = RubberBand::project(60.0, 100.0, 0.55);
    let up = RubberBand::project(-60.0, 100.0, 0.55);
    assert!(down > 0.0, "the downward projection is positive");
    assert_eq!(up, -down, "compression mirrors across the bound");
}

#[test]
fn degenerate_inputs() {
    assert_eq!(
        RubberBand::project(40.0, 0.0, 0.55),
        0.0,
        "no viewport to stretch against"
    );
    assert_eq!(
        RubberBand::project(0.0, 100.0, 0.55),
        0.0,
        "no overscroll to project"
    );
    assert_eq!(
        RubberBand::project(40.0, 100.0, 0.0),
        0.0,
        "zero compliance leaves nothing to stretch"
    );
}

#[test]
fn unproject_is_the_inverse() {
    for raw in [1.0_f32, 5.0, 40.0, 120.0, 400.0] {
        let visible = RubberBand::project(raw, 150.0, 0.55);
        let recovered = RubberBand::unproject(visible, 150.0, 0.55);
        assert!(
            (recovered - raw).abs() < 0.01,
            "raw {raw} round-tripped to {recovered} through {visible}"
        );
    }
}

#[test]
fn unproject_beyond_asymptote() {
    assert_eq!(
        RubberBand::unproject(0.0, 100.0, 0.55),
        0.0,
        "no visible overscroll recovers nothing"
    );
    assert_eq!(
        RubberBand::unproject(100.0, 100.0, 0.55),
        0.0,
        "a displacement equal to the extent is unreachable"
    );
    assert_eq!(
        RubberBand::unproject(140.0, 100.0, 0.55),
        0.0,
        "a displacement past the extent is unreachable"
    );
}

#[test]
fn excess() {
    assert_eq!(
        RubberBand::excess(-12.0, 100.0),
        -12.0,
        "underflow is signed"
    );
    assert_eq!(RubberBand::excess(40.0, 100.0), 0.0, "inside the range");
    assert_eq!(
        RubberBand::excess(112.0, 100.0),
        12.0,
        "overflow is positive"
    );
}
