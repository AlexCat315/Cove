//! Translation of `Tests/CoveUIRuntimeTests/AnimationValueTests.swift`.
//!
//! Behaviour checklist:
//! - `Linear` maps t directly to progress; `EaseIn` is quadratic at 0.5
//! - `EaseOut` mirrors `EaseIn` about y=x; `EaseInOut` is symmetric about
//!   (0.5, 0.5)
//! - inputs outside [0,1] are clamped
//! - `CubicBezier(0,0,1,1)` reproduces linear within tolerance and every cubic
//!   anchors at (0,0)/(1,1) and increases monotonically for canonical easings
//! - a spring anchors its endpoints and overshoots when underdamped; a critical
//!   spring is monotone
//! - overshoot reaches the animated value only when the animation opts in
//! - defaults are 0.25s / ease-in-out / no delay, presets carry their curve
//! - `Animation` is value-equatable; `ease_in_out(duration)` zeroes the delay
//! - the spring factory uses response as duration and keeps its curve
//! - `snappy` and `bouncy` are spring-based

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{Animation, AnimationController, AnimationCurve, AnyAnimationController};

fn assert_close(actual: f32, expected: f32, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{context}: expected {expected}, got {actual}"
    );
}

#[test]
fn linear() {
    let curve = AnimationCurve::Linear;
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert_eq!(curve.evaluate(t), t);
    }
}

#[test]
fn ease_in() {
    let curve = AnimationCurve::EaseIn;
    assert_eq!(curve.evaluate(0.0), 0.0);
    assert_eq!(curve.evaluate(0.5), 0.25, "quadratic midpoint");
    assert_eq!(curve.evaluate(1.0), 1.0);
}

#[test]
fn ease_out() {
    let curve = AnimationCurve::EaseOut;
    assert_eq!(curve.evaluate(0.0), 0.0);
    assert_close(curve.evaluate(0.5), 0.75, "easeOut midpoint");
    assert_eq!(curve.evaluate(1.0), 1.0);
}

#[test]
fn ease_in_out_symmetric() {
    let curve = AnimationCurve::EaseInOut;
    let mut raw = 0.0_f32;
    while raw <= 0.5 {
        let low = curve.evaluate(raw);
        let high = curve.evaluate(1.0 - raw);
        assert!(
            ((low + high) - 1.0).abs() < 1e-5,
            "symmetry at t={raw}: {low} + {high} != 1"
        );
        raw += 0.05;
    }
}

#[test]
fn curve_clamps() {
    let curve = AnimationCurve::EaseInOut;
    assert_eq!(curve.evaluate(-1.0), curve.evaluate(0.0));
    assert_eq!(curve.evaluate(2.0), curve.evaluate(1.0));
}

#[test]
fn cubic_linear() {
    let curve = AnimationCurve::CubicBezier(0.0, 0.0, 1.0, 1.0);
    let mut raw = 0.0_f32;
    while raw <= 1.0 {
        assert!(
            (curve.evaluate(raw) - raw).abs() < 1e-3,
            "cubic(0,0,1,1) tracks linear at t={raw}"
        );
        raw += 0.1;
    }
}

#[test]
fn cubic_endpoints() {
    let curve = AnimationCurve::CubicBezier(0.42, 0.0, 0.58, 1.0);
    assert!(curve.evaluate(0.0).abs() < 1e-4);
    assert!((curve.evaluate(1.0) - 1.0).abs() < 1e-4);
}

#[test]
fn cubic_monotone() {
    let curve = AnimationCurve::CubicBezier(0.25, 0.1, 0.25, 1.0);
    let mut last = -1.0_f32;
    let mut raw = 0.0_f32;
    while raw <= 1.0 {
        let progress = curve.evaluate(raw);
        assert!(progress >= last, "monotone at t={raw}: {progress} < {last}");
        last = progress;
        raw += 0.05;
    }
}

#[test]
fn spring_anchors_and_overshoots() {
    let curve = AnimationCurve::Spring { response: 0.40, damping_fraction: 0.82 };
    assert!(curve.evaluate(0.0).abs() < 1e-4);
    assert!((curve.evaluate(1.0) - 1.0).abs() < 1e-4);

    // Damping below 1 means the spring is still travelling when it first reaches
    // the target, so it must go past it. Clamping this away is what once made
    // every spring in the UI behave like a plain ease.
    let mut peak = 0.0_f32;
    let mut raw = 0.0_f32;
    while raw <= 1.0 {
        peak = peak.max(curve.evaluate(raw));
        raw += 0.01;
    }
    assert!(peak > 1.0, "an underdamped spring overshoots, peak {peak}");
    assert!(curve.evaluate(0.25) >= 0.0);
}

#[test]
fn overshoot_is_opt_in() {
    let spring = Animation::spring(0.4, 0.6);
    assert!(!spring.allows_overshoot, "springs do not overshoot by default");

    let peak = |animation: Animation| {
        let highest = Rc::new(Cell::new(0.0_f32));
        let sink = highest.clone();
        let controller = AnimationController::new(0.0_f32, 1.0_f32, animation, 0.0, move |value| {
            sink.set(sink.get().max(value));
        });
        for step in 1..=20 {
            controller.tick(f64::from(step) * 0.01);
        }
        highest.get()
    };

    let permissive = Animation {
        allows_overshoot: true,
        ..spring.clone()
    };
    assert!(peak(spring) <= 1.0, "a non-overshooting animation clamps to its target");
    assert!(peak(permissive) > 1.0, "opting in lets the overshoot through");
}

#[test]
fn spring_critical_monotone() {
    let curve = AnimationCurve::Spring { response: 0.35, damping_fraction: 1.0 };
    let mut last = -1.0_f32;
    let mut raw = 0.0_f32;
    while raw <= 1.0 {
        let progress = curve.evaluate(raw);
        assert!(progress >= last, "critical spring is monotone at t={raw}");
        last = progress;
        raw += 0.05;
    }
}

#[test]
fn defaults() {
    let default = Animation::default();
    assert_eq!(default.duration, 0.25);
    assert_eq!(default.curve, AnimationCurve::EaseInOut);
    assert_eq!(default.delay, 0.0);

    assert_eq!(Animation::linear().curve, AnimationCurve::Linear);
    assert_eq!(Animation::ease_in().curve, AnimationCurve::EaseIn);
    assert_eq!(Animation::ease_out().curve, AnimationCurve::EaseOut);
}

#[test]
fn equatable() {
    let a = Animation::new(0.3, AnimationCurve::EaseIn).with_delay(0.1);
    let b = Animation::new(0.3, AnimationCurve::EaseIn).with_delay(0.1);
    let c = Animation::new(0.3, AnimationCurve::EaseIn).with_delay(0.2);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn ease_in_out_factory() {
    let animation = Animation::ease_in_out(0.5);
    assert_eq!(animation.duration, 0.5);
    assert_eq!(animation.curve, AnimationCurve::EaseInOut);
    assert_eq!(animation.delay, 0.0);
}

#[test]
fn spring_factory() {
    let animation = Animation::spring(0.6, 0.9).with_delay(0.1);
    assert!((animation.duration - 0.6).abs() < 1e-8, "response is the duration");
    assert!((animation.delay - 0.1).abs() < 1e-8);
    assert_eq!(
        animation.curve,
        AnimationCurve::Spring { response: 0.6, damping_fraction: 0.9 }
    );
}

#[test]
fn spring_presets() {
    for (name, animation) in [("snappy", Animation::snappy()), ("bouncy", Animation::bouncy())] {
        assert!(
            matches!(animation.curve, AnimationCurve::Spring { .. }),
            "{name} is spring-based"
        );
    }
}
