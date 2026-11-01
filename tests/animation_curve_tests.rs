//! Translation of Tests/Headless/Tests/CoveUISceneTests/Animation/CurveTests.swift.
use cove_core::scene::{
    ActiveAnimationContext, Animation, AnimationController, AnimationCurve, AnimationStepPosition,
    AnimatorScheduler, AnyAnimationController, LinearTimingCurve, TimingPoint,
};
use std::cell::Cell;
use std::rc::Rc;

fn points() -> LinearTimingCurve {
    LinearTimingCurve::new(vec![
        TimingPoint {
            time: 0.0,
            progress: 0.0,
        },
        TimingPoint {
            time: 0.25,
            progress: 0.5,
        },
        TimingPoint {
            time: 0.5,
            progress: 0.5,
        },
        TimingPoint {
            time: 0.75,
            progress: 1.25,
        },
        TimingPoint {
            time: 1.0,
            progress: 1.0,
        },
    ])
}

fn curves() -> Vec<AnimationCurve> {
    vec![
        AnimationCurve::Steps(5, AnimationStepPosition::End),
        AnimationCurve::Steps(5, AnimationStepPosition::Start),
        AnimationCurve::PiecewiseLinear(points()),
        AnimationCurve::BounceOut,
        AnimationCurve::ElasticOut,
    ]
}

#[test]
fn endpoints_clamp_time_tolerate_nonfinite_time_and_end_exactly_at_one() {
    for curve in curves() {
        assert_eq!(curve.evaluate(-1.0), curve.evaluate(0.0));
        assert_eq!(curve.evaluate(f32::NEG_INFINITY), curve.evaluate(0.0));
        assert_eq!(curve.evaluate(f32::NAN), curve.evaluate(0.0));
        assert_eq!(curve.evaluate(1.0), 1.0);
        assert_eq!(curve.evaluate(2.0), 1.0);
        assert_eq!(curve.evaluate(f32::INFINITY), 1.0);
        assert!(curve.evaluate(0.0).is_finite());
    }
}

#[test]
fn steps_jump_on_the_specified_interval_edge_and_remain_monotone() {
    for count in [1_i64, 2, 5, 10] {
        let end = AnimationCurve::Steps(count, AnimationStepPosition::End);
        let start = AnimationCurve::Steps(count, AnimationStepPosition::Start);
        let count_f = count as f32;
        assert_eq!(end.evaluate(0.0), 0.0);
        assert_eq!(start.evaluate(0.0), 1.0 / count_f);
        for index in 1..count {
            let boundary = index as f32 / count_f;
            assert_eq!(end.evaluate(boundary), index as f32 / count_f);
            assert_eq!(
                end.evaluate(boundary.next_down()),
                (index - 1) as f32 / count_f
            );
            assert_eq!(start.evaluate(boundary), (index + 1) as f32 / count_f);
            assert_eq!(start.evaluate(boundary.next_down()), index as f32 / count_f);
        }
        for curve in [start, end] {
            let mut previous = 0.0_f32;
            for sample in 0..=1_000 {
                let progress = curve.evaluate(sample as f32 / 1_000.0);
                assert!(progress >= previous && progress <= 1.0);
                previous = progress;
            }
        }
        assert_eq!(
            AnimationCurve::Steps(count, AnimationStepPosition::End).evaluate(1.0_f32.next_down()),
            (count - 1) as f32 / count_f
        );
    }
}

#[test]
fn invalid_step_counts_resolve_to_one_step_and_large_counts_remain_finite() {
    for count in [0_i64, -1, i64::MIN] {
        assert_eq!(
            AnimationCurve::Steps(count, AnimationStepPosition::End).evaluate(0.5),
            0.0
        );
        assert_eq!(
            AnimationCurve::Steps(count, AnimationStepPosition::Start).evaluate(0.0),
            1.0
        );
    }
    for time in [0.0_f32, 0.1, 0.5, 0.9, 1.0] {
        assert!(
            AnimationCurve::Steps(i64::MAX, AnimationStepPosition::End)
                .evaluate(time)
                .is_finite()
        );
        assert!(
            AnimationCurve::Steps(i64::MAX, AnimationStepPosition::Start).evaluate(time) <= 1.0
        );
    }
}

#[test]
fn linear_points_interpolate_each_segment_including_plateaus_reversal_and_overshoot() {
    let curve = AnimationCurve::PiecewiseLinear(points());
    for (time, expected) in [
        (0.0, 0.0),
        (0.125, 0.25),
        (0.25, 0.5),
        (0.375, 0.5),
        (0.5, 0.5),
        (0.625, 0.875),
        (0.75, 1.25),
        (0.875, 1.125),
        (1.0, 1.0),
    ] {
        assert_eq!(curve.evaluate(time), expected);
    }
    let linear = AnimationCurve::PiecewiseLinear(LinearTimingCurve::new(vec![
        TimingPoint {
            time: 0.0,
            progress: 0.0,
        },
        TimingPoint {
            time: 1.0,
            progress: 1.0,
        },
    ]));
    for sample in 0..=1_000 {
        let time = sample as f32 / 1_000.0;
        assert_eq!(linear.evaluate(time), time);
    }
}

#[test]
fn large_point_lists_retain_value_semantics_and_finite_interpolation() {
    let mut points: Vec<TimingPoint> = (0..=1_000)
        .map(|index| {
            let time = index as f32 / 1_000.0;
            TimingPoint {
                time,
                progress: time * time,
            }
        })
        .collect();
    let value = LinearTimingCurve::new(points.clone());
    let curve = AnimationCurve::PiecewiseLinear(value.clone());
    assert_eq!(value, LinearTimingCurve::new(points.clone()));
    points[500] = TimingPoint {
        time: 0.5,
        progress: 1.0,
    };
    assert_eq!(curve.evaluate(0.5), 0.25);
    assert_ne!(value, LinearTimingCurve::new(points));
    for sample in 0..=2_000 {
        let time = sample as f32 / 2_000.0;
        assert!((curve.evaluate(time) - time * time).abs() < 0.000001);
    }
    let extremes = AnimationCurve::PiecewiseLinear(LinearTimingCurve::new(vec![
        TimingPoint {
            time: 0.0,
            progress: 0.0,
        },
        TimingPoint {
            time: 0.25,
            progress: f32::MAX,
        },
        TimingPoint {
            time: 0.5,
            progress: -f32::MAX,
        },
        TimingPoint {
            time: 1.0,
            progress: 1.0,
        },
    ]));
    assert_eq!(extremes.evaluate(0.375), 0.0);
    assert_eq!(extremes.evaluate(0.3125), f32::MAX / 2.0);
}

#[test]
fn bounce_is_continuous_at_impacts_and_has_diminishing_bounded_valleys() {
    let curve = AnimationCurve::BounceOut;
    for impact in [1.0_f32 / 2.75, 2.0 / 2.75, 2.5 / 2.75] {
        assert!((curve.evaluate(impact) - 1.0).abs() < 0.000001);
        assert!(
            (curve.evaluate(impact - 0.000001) - curve.evaluate(impact + 0.000001)).abs() < 0.00002
        );
    }
    for (time, expected) in [
        (1.5_f32 / 2.75, 0.75),
        (2.25 / 2.75, 0.9375),
        (2.625 / 2.75, 0.984375),
    ] {
        assert!((curve.evaluate(time) - expected).abs() < 0.000001);
    }
    assert_eq!(curve.evaluate(0.0), 0.0);
    for sample in 0..=5_000 {
        let progress = curve.evaluate(sample as f32 / 5_000.0);
        assert!((0.0..=1.0).contains(&progress));
    }
}

#[test]
fn elastic_preserves_overshoot_while_oscillations_decay_to_the_exact_endpoint() {
    let curve = AnimationCurve::ElasticOut;
    assert_eq!(curve.evaluate(0.0), 0.0);
    assert_eq!(curve.evaluate(1.0), 1.0);
    for (time, expected) in [
        (0.2_f32, 1.25),
        (0.4, 0.9375),
        (0.6, 1.015625),
        (0.8, 0.99609375),
    ] {
        assert!((curve.evaluate(time) - expected).abs() < 0.000001);
    }
    assert!(curve.evaluate(0.2) > 1.0 && curve.evaluate(0.4) < 1.0);
    for sample in 0..=1_000 {
        assert!(curve.evaluate(sample as f32 / 1_000.0).is_finite());
    }
}

#[test]
fn controllers_preserve_delay_overshoot_exact_finish_and_scheduler_cleanup() {
    for curve in curves() {
        let value = Rc::new(Cell::new(80.0_f32));
        let writes = Rc::new(Cell::new(0_usize));
        let written = value.clone();
        let count = writes.clone();
        let animation = Animation::new(1.0, curve.clone())
            .with_delay(0.1)
            .with_overshoot(true);
        let controller =
            AnimationController::new(80.0_f32, 200.0_f32, animation, 0.0, move |next| {
                written.set(next);
                count.set(count.get() + 1);
            });
        let scheduler = AnimatorScheduler::default();
        let erased: Rc<dyn AnyAnimationController> = controller.clone();
        scheduler.register(erased);
        scheduler.tick(0.1);
        assert_eq!(value.get(), 80.0);
        assert!(!controller.is_finished());
        scheduler.tick(0.2);
        assert!((value.get() - (80.0 + 120.0 * curve.evaluate(0.2))).abs() < 0.0001);
        if curve == AnimationCurve::ElasticOut {
            assert!(value.get() > 200.0);
        }
        scheduler.tick(1.0);
        assert_eq!(value.get(), 200.0);
        assert!(controller.is_finished());
        assert_eq!(scheduler.active_count(), 0);
        let finished_writes = writes.get();
        scheduler.tick(1.0);
        assert_eq!(writes.get(), finished_writes);
    }
}

#[test]
fn active_context_is_scoped() {
    assert!(ActiveAnimationContext::current().animation.is_none());
    ActiveAnimationContext::with(Some(Animation::linear()), None, Some(false), || {
        assert!(ActiveAnimationContext::current().animation.is_some());
    });
    assert!(ActiveAnimationContext::current().animation.is_none());
}
