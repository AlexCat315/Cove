//! Translation of `Tests/CoveUIComposeTests/SemanticMotionTests.swift`.
//!
//! Behaviour checklist:
//! - `.fast` resolves to `theme.motion.fast.seconds`
//! - `.medium` resolves to `theme.motion.standard`
//! - `.slow` resolves to `theme.motion.slow`
//! - the light and dark default themes share their motion durations
//! - custom theme motion overrides flow through
//! - `.snappy` is a spring on `theme.motion.fast`, `.bouncy` on `standard`
//! - an `Easing` bridges to a cubic-bezier `AnimationCurve`
//! - surface entrance/exit and `smooth` resolve the active theme's authored groups
//! - `staggered` defers later siblings without altering the base animation

use cove_core::scene::{Animation, AnimationCurve};
use cove_ui_compose::theme::{Easing, SemanticMotionRef, Theme, semantic_animation, staggered};

#[test]
fn fast_follows_theme() {
    let animation = semantic_animation(&SemanticMotionRef::fast(), &Theme::default_dark());
    assert!(
        (animation.duration - 0.080).abs() < 1e-6,
        "`.fast` uses the theme's fast duration (got {})",
        animation.duration
    );
}

#[test]
fn medium_follows_theme() {
    let animation = semantic_animation(&SemanticMotionRef::medium(), &Theme::default_dark());
    assert!(
        (animation.duration - 0.180).abs() < 1e-6,
        "`.medium` uses the theme's standard duration (got {})",
        animation.duration
    );
}

#[test]
fn slow_follows_theme() {
    let animation = semantic_animation(&SemanticMotionRef::slow(), &Theme::default_dark());
    assert!(
        (animation.duration - 0.320).abs() < 1e-6,
        "`.slow` uses the theme's slow duration (got {})",
        animation.duration
    );
}

#[test]
fn light_and_dark_parity() {
    for reference in [
        SemanticMotionRef::fast(),
        SemanticMotionRef::medium(),
        SemanticMotionRef::slow(),
    ] {
        let dark = semantic_animation(&reference, &Theme::default_dark());
        let light = semantic_animation(&reference, &Theme::default_light());
        assert_eq!(
            dark.duration, light.duration,
            "the presets share their motion durations ({} vs {})",
            dark.duration, light.duration
        );
    }
}

#[test]
fn custom_theme_overrides_flow_through() {
    let mut theme = Theme::default_dark();
    theme.motion.fast = 0.050;
    let fast = semantic_animation(&SemanticMotionRef::fast(), &theme);
    assert!(
        (fast.duration - 0.050).abs() < 1e-6,
        "the theme's fast override applies (got {})",
        fast.duration
    );

    let snappy = semantic_animation(&SemanticMotionRef::snappy(), &theme);
    assert!(
        (snappy.duration - 0.050).abs() < 1e-6,
        "`snappy` springs on the overridden fast duration (got {})",
        snappy.duration
    );
}

#[test]
fn snappy_follows_theme_fast() {
    let animation = semantic_animation(&SemanticMotionRef::snappy(), &Theme::default_dark());
    assert!(
        (animation.duration - 0.080).abs() < 1e-6,
        "`snappy` runs for the theme's fast duration (got {})",
        animation.duration
    );
    assert_eq!(
        animation.curve,
        AnimationCurve::Spring {
            response: 0.08,
            damping_fraction: 0.9,
        },
        "`snappy` is a 0.9-damped spring on the fast response"
    );
}

#[test]
fn bouncy_follows_theme_standard() {
    let animation = semantic_animation(&SemanticMotionRef::bouncy(), &Theme::default_dark());
    assert!(
        (animation.duration - 0.180).abs() < 1e-6,
        "`bouncy` runs for the theme's standard duration (got {})",
        animation.duration
    );
    assert_eq!(
        animation.curve,
        AnimationCurve::Spring {
            response: 0.18,
            damping_fraction: 0.68,
        },
        "`bouncy` is a 0.68-damped spring on the standard response"
    );
}

#[test]
fn easing_bridge() {
    let curve = Easing::STANDARD.as_animation_curve();
    match curve {
        AnimationCurve::CubicBezier(c1x, c1y, c2x, c2y) => {
            assert_eq!(c1x, Easing::STANDARD.c1x);
            assert_eq!(c1y, Easing::STANDARD.c1y);
            assert_eq!(c2x, Easing::STANDARD.c2x);
            assert_eq!(c2y, Easing::STANDARD.c2y);
        }
        other => panic!("expected a cubic-bezier curve, got {other:?}"),
    }
}

#[test]
fn surface_motion() {
    let mut theme = Theme::default_light();
    theme.motion.surface.entrance = 0.450;
    theme.motion.surface.exit = 0.120;
    theme.motion.standard = 0.300;
    let entering = semantic_animation(&SemanticMotionRef::entering(), &theme);
    let exiting = semantic_animation(&SemanticMotionRef::exiting(), &theme);
    assert_eq!(
        (entering.duration, entering.curve),
        (0.45, Easing::DECELERATE.as_animation_curve()),
        "entrance reads the theme's surface entrance"
    );
    assert_eq!(
        (exiting.duration, exiting.curve),
        (0.12, Easing::DECELERATE.as_animation_curve()),
        "exit reads the theme's surface exit"
    );
    assert_eq!(
        semantic_animation(&SemanticMotionRef::smooth(), &theme).curve,
        AnimationCurve::Spring {
            response: 0.3,
            damping_fraction: 1.0,
        },
        "`smooth` springs critically on the standard response"
    );
}

#[test]
fn staggered_defers_later_siblings() {
    let base = Animation::new(0.200, AnimationCurve::EaseOut);
    let third = staggered(&base, 3, 0.050);
    assert!(
        (third.delay - 0.150).abs() < 1e-6,
        "the third sibling waits two intervals (got {})",
        third.delay
    );
    assert_eq!(
        third.duration, 0.2,
        "staggering preserves the base duration"
    );
    assert_eq!(
        third.curve,
        AnimationCurve::EaseOut,
        "staggering preserves the base curve"
    );

    // A zero index stays inert.
    assert_eq!(
        staggered(&base, 0, 0.050).delay,
        0.0,
        "the first sibling is not deferred"
    );
}
