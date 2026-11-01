//! Translation of `Tests/Headless/Tests/CoveUISceneTests/Animation/PlaybackTests.swift`.
//!
//! Animation playback and physical motion. Repetition counting, delay handling,
//! spring derivatives and completion ordering are all things a dropped frame or
//! a retarget can get wrong in ways that only show up as motion that stutters,
//! overshoots, or never settles.
//!
//! Behaviour checklist:
//! - finite repetitions count complete forward/reverse cycles and finish on the
//!   correct end value
//! - forever playback preserves the delay, survives dropped frames and a huge
//!   tick, and only stops when told to
//! - invalid tick deltas are ignored, and zero/invalid durations snap once
//! - a timeline samples plateaus, reversals and exact boundaries independently
//!   of tick cadence, including NaN and infinity
//! - a physical spring's velocity matches the derivative of its position, and it
//!   reaches equilibrium at the target
//! - spring parameters and non-finite time cannot produce non-finite geometry
//! - a retargeted point animation preserves both components of the real value
//!   velocity
//! - unmounting cancels a forever property animation immediately and suppresses
//!   its completion
//! - logical completion precedes physical removal
//! - a cancelled completion group is released without another scheduler tick

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use cove_core::scene::{
    ActiveAnimationContext, Animation, AnimationCompletionCriteria, AnimationCompletionGroup,
    AnimationController, AnimationCurve, AnimationKeyframe, AnimatorScheduler,
    AnyAnimationController, KeyframeTimeline, Node, Point, SpringSolution,
};

fn opacity_animation(node: &Node, target: f32) {
    let current = node.opacity();
    let written = node.clone();
    node.animatable_set_with("opacity", current, target, move |value| {
        written.set_opacity(value)
    });
}

fn offset_animation(node: &Node, target: Point) {
    let current = node.content_offset();
    let written = node.clone();
    node.animatable_set_with("content_offset", current, target, move |value| {
        written.set_content_offset(value)
    });
}

#[test]
fn repeats() {
    for autoreverses in [false, true] {
        let value = Rc::new(Cell::new(-1.0_f32));
        let written = value.clone();
        let animation = Animation::new(1.0, AnimationCurve::Linear).repeat_count(2, autoreverses);
        let controller = AnimationController::new(0.0_f32, 10.0_f32, animation, 0.0, move |next| {
            written.set(next);
        });

        AnyAnimationController::tick(&*controller, 0.5);
        assert_eq!(value.get(), 5.0, "half a cycle is the midpoint");
        AnyAnimationController::tick(&*controller, 0.5);
        assert_eq!(
            value.get(),
            if autoreverses { 10.0 } else { 0.0 },
            "a full cycle lands on the far end (autoreverses={autoreverses})"
        );
        AnyAnimationController::tick(&*controller, 0.5);
        assert_eq!(value.get(), 5.0, "the second cycle restarts");
        AnyAnimationController::tick(&*controller, 10.0);
        assert_eq!(
            value.get(),
            if autoreverses { 0.0 } else { 10.0 },
            "two cycles finish on the correct end (autoreverses={autoreverses})"
        );
        assert!(controller.is_finished());
    }
}

#[test]
fn forever() {
    let value = Rc::new(Cell::new(0.0_f64));
    let scheduler = AnimatorScheduler::default();
    let written = value.clone();
    let animation = Animation::new(1.0, AnimationCurve::Linear)
        .with_delay(0.2)
        .repeat_forever(false);
    let controller = AnimationController::new(0.0_f64, 10.0_f64, animation, 0.0, move |next| {
        written.set(next);
    });
    let erased: Rc<dyn AnyAnimationController> = controller.clone();
    scheduler.register(erased);

    scheduler.tick(0.2);
    assert_eq!(value.get(), 0.0, "the delay pins the start value");
    scheduler.tick(10.75);
    assert!(
        (value.get() - 7.5).abs() < 0.0001,
        "a dropped-frame tick still lands on the right phase, got {}",
        value.get()
    );
    scheduler.tick(1e14);
    assert!(
        value.get().is_finite() && (0.0..=10.0).contains(&value.get()),
        "an absurd tick stays inside the range, got {}",
        value.get()
    );
    assert_eq!(scheduler.active_count(), 1);
    assert!(
        !controller.is_finished(),
        "forever never finishes on its own"
    );

    controller.finish_immediately();
    scheduler.tick(0.0);
    assert_eq!(value.get(), 10.0, "an explicit finish writes the target");
    assert_eq!(scheduler.active_count(), 0);
}

#[test]
fn invalid_time() {
    let writes = Rc::new(Cell::new(0_usize));
    let counter = writes.clone();
    let controller =
        AnimationController::new(0.0_f32, 1.0_f32, Animation::linear(), 0.0, move |_| {
            counter.set(counter.get() + 1);
        });
    for delta in [-1.0, f64::NAN, f64::INFINITY] {
        AnyAnimationController::tick(&*controller, delta);
    }
    assert_eq!(
        controller.elapsed(),
        0.0,
        "an invalid delta does not advance"
    );
    assert_eq!(writes.get(), 0, "and does not write a value");

    for duration in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let value = Rc::new(Cell::new(0.0_f64));
        let written = value.clone();
        let instant = AnimationController::new(
            0.0_f64,
            0.1_f64,
            Animation::new(duration, AnimationCurve::Linear).repeat_forever(false),
            0.0,
            move |next| written.set(next),
        );
        assert!(
            instant.is_finished(),
            "a duration of {duration} finishes immediately"
        );
        assert_eq!(value.get(), 0.1, "and snaps once to the target");
    }
}

#[test]
fn timeline() {
    let timeline = KeyframeTimeline::new(
        0.0_f32,
        vec![
            AnimationKeyframe::new(10.0_f32, 0.5, AnimationCurve::Linear),
            AnimationKeyframe::new(10.0_f32, 0.25, AnimationCurve::Linear),
            AnimationKeyframe::new(-5.0_f32, 0.25, AnimationCurve::Linear),
        ],
    );
    for (time, expected) in [
        (-1.0, 0.0_f32),
        (0.25, 5.0),
        (0.5, 10.0),
        (0.7, 10.0),
        (0.875, 2.5),
        (1.0, -5.0),
        (f64::INFINITY, -5.0),
    ] {
        assert_eq!(
            timeline.value(time),
            expected,
            "sample at {time} must be {expected}"
        );
    }
    assert_eq!(timeline.value(f64::NAN), 0.0, "NaN falls back to the start");
    assert_eq!(timeline.duration(), 1.0);
}

#[test]
fn physical_spring() {
    for damping in [0.4_f32, 1.0, 3.0] {
        let spring = SpringSolution::new(0.4, damping);
        let time = 0.12_f32;
        let delta = 0.0001_f32;
        let derivative = (spring.position(time + delta, 3.0, -7.0, 12.0)
            - spring.position(time - delta, 3.0, -7.0, 12.0))
            / (2.0 * delta);
        assert!(
            (derivative - spring.velocity(time, 3.0, -7.0, 12.0)).abs() < 0.02,
            "velocity is the derivative of position at damping {damping}"
        );
        assert!(
            (spring.position(spring.settling_duration() as f32, 3.0, -7.0, 12.0) - 12.0).abs()
                < 0.002,
            "the spring has reached the target once settled, at damping {damping}"
        );
        assert_eq!(
            AnimationCurve::Spring {
                response: 0.4,
                damping_fraction: damping
            }
            .evaluate(1.0),
            1.0,
            "a spring curve ends exactly at 1"
        );
    }
}

#[test]
fn spring_validation() {
    for parameter in [0.0_f32, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        let spring = SpringSolution::new(parameter, parameter);
        assert!(
            (0.001..=30.0).contains(&spring.response),
            "response {parameter} is clamped, got {}",
            spring.response
        );
        assert!(
            (0.01..=20.0).contains(&spring.damping_fraction),
            "damping {parameter} is clamped, got {}",
            spring.damping_fraction
        );
        assert!(
            spring.position(0.1, 0.0, 0.0, 1.0).is_finite(),
            "geometry stays finite for parameter {parameter}"
        );
        assert_eq!(
            spring.position(f32::NAN, 3.0, 2.0, 1.0),
            3.0,
            "NaN time is the start value"
        );
        assert_eq!(
            spring.position(f32::INFINITY, 3.0, 2.0, 1.0),
            1.0,
            "infinite time is the target"
        );
    }
}

#[test]
fn inherited_vector() {
    let scheduler = AnimatorScheduler::default();
    scheduler.with_current(|| {
        let node = Node::new();
        let motion = Animation::spring(0.4, 0.6).with_overshoot(true);
        ActiveAnimationContext::with(Some(motion.clone()), None, None, || {
            offset_animation(&node, Point::new(100.0, 50.0))
        });
        scheduler.tick(0.08);
        let before = node.content_offset();

        ActiveAnimationContext::with(Some(motion), None, None, || {
            offset_animation(&node, Point::new(-20.0, 140.0))
        });
        assert_eq!(
            node.content_offset(),
            before,
            "a retarget does not jump on the frame it is applied"
        );
        assert_eq!(
            scheduler.active_count(),
            1,
            "the old controller is replaced"
        );

        scheduler.tick(0.0001);
        let expected = SpringSolution::new(0.4, 0.6).velocity(0.08, 0.0, 0.0, 1.0);
        assert!(
            ((node.content_offset().x - before.x) / 0.0001 - expected * 100.0).abs() < 2.0,
            "the x velocity is inherited from the real value"
        );
        assert!(
            ((node.content_offset().y - before.y) / 0.0001 - expected * 50.0).abs() < 2.0,
            "the y velocity is inherited from the real value"
        );

        scheduler.tick(5.0);
        assert_eq!(node.content_offset(), Point::new(-20.0, 140.0));
        assert_eq!(scheduler.active_count(), 0);
    });
}

#[test]
fn unmount() {
    let scheduler = AnimatorScheduler::default();
    scheduler.with_current(|| {
        let root = Node::new();
        let node = Node::new();
        root.add_child(&node);

        let completions = Rc::new(Cell::new(0_usize));
        let count = completions.clone();
        let group =
            AnimationCompletionGroup::new(AnimationCompletionCriteria::Removed, move || {
                count.set(count.get() + 1)
            });
        scheduler.track(group.clone());
        ActiveAnimationContext::with(
            Some(Animation::linear().repeat_forever(false)),
            Some(group.clone()),
            None,
            || opacity_animation(&node, 0.0),
        );
        assert_eq!(scheduler.active_count(), 1);

        root.remove_child(&node);
        assert_eq!(
            scheduler.active_count(),
            0,
            "unmounting cancels immediately, without waiting for a tick"
        );

        scheduler.tick(10.0);
        assert!(group.has_settled(), "the group released");
        assert_eq!(
            completions.get(),
            0,
            "a cancelled animation never reports completion"
        );
    });
}

#[test]
fn completion() {
    for criteria in [
        AnimationCompletionCriteria::LogicallyComplete,
        AnimationCompletionCriteria::Removed,
    ] {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let node = Node::new();
            let completions = Rc::new(Cell::new(0_usize));
            let count = completions.clone();
            let group = AnimationCompletionGroup::new(criteria, move || count.set(count.get() + 1));
            scheduler.track(group.clone());
            ActiveAnimationContext::with(
                Some(Animation::spring(0.4, 0.6)),
                Some(group.clone()),
                None,
                || opacity_animation(&node, 0.0),
            );

            scheduler.tick(0.4);
            assert_eq!(
                completions.get(),
                if criteria == AnimationCompletionCriteria::LogicallyComplete {
                    1
                } else {
                    0
                },
                "logical completion fires early; removal waits for the physics ({criteria:?})"
            );
            assert_eq!(scheduler.active_count(), 1);

            scheduler.tick(5.0);
            assert_eq!(completions.get(), 1, "the group completes exactly once");
            assert_eq!(scheduler.active_count(), 0);
        });
    }
}

#[test]
fn completion_lifetime() {
    let scheduler = AnimatorScheduler::default();
    scheduler.with_current(|| {
        let root = Node::new();
        let node = Node::new();
        root.add_child(&node);

        let retained: Rc<RefCell<Option<Weak<AnimationCompletionGroup>>>> =
            Rc::new(RefCell::new(None));
        {
            let group = AnimationCompletionGroup::new(AnimationCompletionCriteria::Removed, || {
                panic!("a cancelled scope must not complete")
            });
            *retained.borrow_mut() = Some(Rc::downgrade(&group));
            scheduler.track(group.clone());
            ActiveAnimationContext::with(Some(Animation::linear()), Some(group), None, || {
                opacity_animation(&node, 0.0)
            });
        }
        assert!(
            retained
                .borrow()
                .as_ref()
                .is_some_and(|weak| weak.upgrade().is_some()),
            "the scope still holds the group before unmount"
        );

        root.remove_child(&node);
        assert!(
            retained
                .borrow()
                .as_ref()
                .is_some_and(|weak| weak.upgrade().is_none()),
            "cancellation releases the group without another tick"
        );
        assert_eq!(scheduler.active_count(), 0);
    });
}
