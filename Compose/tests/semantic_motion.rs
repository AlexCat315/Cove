//! Translation of Tests/CoveUIComposeTests/SemanticMotionTests.swift.
use cove_core::scene::{Animation, AnimationCurve};
use cove_ui_compose::theme::{
    semantic_animation, staggered, Easing, SemanticMotionRef, Theme,
};

#[test]
fn fast_follows_theme() {
    let a = semantic_animation(&SemanticMotionRef::fast(), &Theme::default_dark());
    assert!((a.duration - 0.080).abs() < 1e-6);
}

#[test]
fn medium_follows_theme() {
    let a = semantic_animation(&SemanticMotionRef::medium(), &Theme::default_dark());
    assert!((a.duration - 0.180).abs() < 1e-6);
}

#[test]
fn slow_follows_theme() {
    let a = semantic_animation(&SemanticMotionRef::slow(), &Theme::default_dark());
    assert!((a.duration - 0.320).abs() < 1e-6);
}

#[test]
fn light_dark_parity() {
    for reference in [SemanticMotionRef::fast(), SemanticMotionRef::medium(), SemanticMotionRef::slow()] {
        let dark = semantic_animation(&reference, &Theme::default_dark());
        let light = semantic_animation(&reference, &Theme::default_light());
        assert_eq!(dark.duration, light.duration);
    }
}

#[test]
fn custom_theme() {
    let mut theme = Theme::default_dark();
    theme.motion.fast = 0.050;
    let a = semantic_animation(&SemanticMotionRef::fast(), &theme);
    assert!((a.duration - 0.050).abs() < 1e-6);
    let snappy = semantic_animation(&SemanticMotionRef::snappy(), &theme);
    assert!((snappy.duration - 0.050).abs() < 1e-6);
}

#[test]
fn snappy_follows_theme_fast() {
    let a = semantic_animation(&SemanticMotionRef::snappy(), &Theme::default_dark());
    assert!((a.duration - 0.080).abs() < 1e-6);
    assert_eq!(a.curve, AnimationCurve::Spring { response: 0.08, damping_fraction: 0.9 });
}

#[test]
fn bouncy_follows_theme_standard() {
    let a = semantic_animation(&SemanticMotionRef::bouncy(), &Theme::default_dark());
    assert!((a.duration - 0.180).abs() < 1e-6);
    assert_eq!(a.curve, AnimationCurve::Spring { response: 0.18, damping_fraction: 0.68 });
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
        _ => panic!("expected cubicBezier"),
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
    assert_eq!(entering.duration, 0.45);
    assert_eq!(entering.curve, Easing::DECELERATE.as_animation_curve());
    assert_eq!(exiting.duration, 0.12);
    assert_eq!(exiting.curve, Easing::DECELERATE.as_animation_curve());
    assert_eq!(
        semantic_animation(&SemanticMotionRef::smooth(), &theme).curve,
        AnimationCurve::Spring { response: 0.3, damping_fraction: 1.0 }
    );
}

#[test]
fn stagger_defers_later_siblings() {
    let base = Animation::new(0.2, AnimationCurve::EaseOut);
    let third = staggered(&base, 3, 0.05);
    assert!((third.delay - 0.15).abs() < 1e-6);
    assert_eq!(third.duration, 0.2);
    assert_eq!(third.curve, AnimationCurve::EaseOut);
    assert_eq!(staggered(&base, 0, 0.05).delay, 0.0);
}
