//! Translation of `Tests/CoveUIRuntimeTests/AnimationControllerTests.swift`.
//!
//! Behaviour checklist:
//! - a tick at t=0 applies `from`; at t=duration applies `to` and finishes
//! - linear interpolation hits the midpoint
//! - the delay window pins to the `from` value
//! - a tick after `is_finished` is a no-op
//! - a zero-duration animation snaps to the target on construction
//! - `finish_immediately` writes the target and marks finished
//! - an eased curve produces non-linear progress at t=0.5
//! - a colour controller interpolates RGBA
//! - the scheduler tracks and ticks every registered controller
//! - duplicate registration ticks once, and registrations during a tick apply
//!   from the next tick, with removed callbacks staying removed
//! - finished controllers are evicted on the same tick
//! - an empty scheduler tick is a no-op and `reset` drops every active controller

use std::cell::Cell;
use std::rc::Rc;

use cove_core::Color;
use cove_core::scene::{
    Animation, AnimationController, AnimationCurve, AnimatorScheduler, AnyAnimationController,
};

/// A `Float`-writing controller over a shared cell, the Swift `apply:` closure.
fn float_controller(
    from: f32,
    to: f32,
    animation: Animation,
) -> (Rc<AnimationController<f32>>, Rc<Cell<f32>>) {
    let written = Rc::new(Cell::new(-1.0_f32));
    let sink = written.clone();
    let controller =
        AnimationController::new(from, to, animation, 0.0, move |value| sink.set(value));
    (controller, written)
}

fn as_any(controller: Rc<AnimationController<f32>>) -> Rc<dyn AnyAnimationController> {
    controller
}

#[test]
fn endpoints() {
    let (controller, written) =
        float_controller(0.0, 10.0, Animation::new(1.0, AnimationCurve::Linear));

    controller.tick(0.0);
    assert_eq!(written.get(), 0.0, "t=0 applies `from`");
    assert!(!controller.is_finished());

    controller.tick(1.0);
    assert_eq!(written.get(), 10.0, "t=duration applies `to`");
    assert!(controller.is_finished());
}

#[test]
fn midpoint() {
    let (controller, written) =
        float_controller(0.0, 10.0, Animation::new(1.0, AnimationCurve::Linear));
    controller.tick(0.5);
    assert_eq!(written.get(), 5.0, "a linear curve hits the midpoint");
    assert!(!controller.is_finished());
}

#[test]
fn delay_window() {
    let (controller, written) = float_controller(
        0.0,
        10.0,
        Animation::new(1.0, AnimationCurve::Linear).with_delay(0.5),
    );

    controller.tick(0.25);
    assert_eq!(
        written.get(),
        0.0,
        "inside the delay window the value stays at `from`"
    );
    assert!(!controller.is_finished());

    // Now at elapsed 0.75 → active 0.25 of 1.0.
    controller.tick(0.5);
    assert!(
        (written.get() - 2.5).abs() < 1e-5,
        "the delay shifts the active window"
    );
}

#[test]
fn tick_after_finished() {
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let controller = AnimationController::new(
        0.0_f32,
        10.0,
        Animation::new(0.1, AnimationCurve::Linear),
        0.0,
        move |_| counter.set(counter.get() + 1),
    );

    controller.tick(1.0);
    assert!(
        controller.is_finished(),
        "the first tick finishes the short animation"
    );

    let before = calls.get();
    controller.tick(1.0);
    assert_eq!(calls.get(), before, "ticks after completion do not apply");
}

#[test]
fn zero_duration() {
    let (controller, written) =
        float_controller(0.0, 10.0, Animation::new(0.0, AnimationCurve::Linear));
    assert_eq!(
        written.get(),
        10.0,
        "a zero-duration animation snaps on construction"
    );
    assert!(controller.is_finished());
}

#[test]
fn finish_immediately_writes_target() {
    let (controller, written) =
        float_controller(0.0, 10.0, Animation::new(1.0, AnimationCurve::Linear));
    controller.tick(0.25);
    assert_eq!(written.get(), 2.5);

    controller.finish_immediately();
    assert_eq!(written.get(), 10.0, "finishing jumps to the target");
    assert!(controller.is_finished());
}

#[test]
fn easing_shape() {
    let (controller, written) =
        float_controller(0.0, 100.0, Animation::new(1.0, AnimationCurve::EaseIn));
    controller.tick(0.5);
    // EaseIn at t=0.5 is 0.25, so the value should be 25.
    assert!((written.get() - 25.0).abs() < 1e-3, "easeIn(0.5) = 0.25");
}

#[test]
fn color_controller() {
    let written = Rc::new(Cell::new(Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }));
    let sink = written.clone();
    let controller = AnimationController::new(
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        },
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        Animation::new(1.0, AnimationCurve::Linear),
        0.0,
        move |value| sink.set(value),
    );

    controller.tick(0.5);
    let value = written.get();
    assert_eq!(value.r, 0.5, "red interpolates at the midpoint");
    assert_eq!(value.a, 0.5, "alpha interpolates at the midpoint");
}

#[test]
fn scheduler_ticks() {
    let scheduler = AnimatorScheduler::default();
    let (first, first_written) =
        float_controller(0.0, 10.0, Animation::new(1.0, AnimationCurve::Linear));
    let (second, second_written) =
        float_controller(100.0, 0.0, Animation::new(1.0, AnimationCurve::Linear));
    scheduler.register(as_any(first));
    scheduler.register(as_any(second));
    assert_eq!(scheduler.active_count(), 2);

    scheduler.tick(0.5);
    assert_eq!(first_written.get(), 5.0);
    assert_eq!(
        second_written.get(),
        50.0,
        "a descending animation interpolates downward"
    );
}

#[test]
fn scheduler_registration_during_tick() {
    let scheduler = AnimatorScheduler::default();

    let removed_value = Rc::new(Cell::new(0.0_f32));
    let removed_sink = removed_value.clone();
    let removed: Rc<dyn AnyAnimationController> = AnimationController::new(
        0.0_f32,
        1.0,
        Animation::new(1.0, AnimationCurve::Linear),
        0.0,
        move |value| removed_sink.set(value),
    );

    let added_value = Rc::new(Cell::new(0.0_f32));
    let added_sink = added_value.clone();
    let added: Rc<dyn AnyAnimationController> = AnimationController::new(
        0.0_f32,
        1.0,
        Animation::new(1.0, AnimationCurve::Linear),
        0.0,
        move |value| added_sink.set(value),
    );

    let mutating_scheduler = scheduler.clone();
    let to_remove = removed.clone();
    let to_add = added.clone();
    let first: Rc<dyn AnyAnimationController> = AnimationController::new(
        0.0_f32,
        1.0,
        Animation::new(1.0, AnimationCurve::Linear),
        0.0,
        move |_| {
            mutating_scheduler.unregister(&to_remove);
            mutating_scheduler.register(to_add.clone());
        },
    );

    scheduler.register(first.clone());
    scheduler.register(removed.clone());
    scheduler.register(first);
    assert_eq!(
        scheduler.active_count(),
        2,
        "duplicate registration is ignored"
    );

    scheduler.tick(0.25);
    assert_eq!(
        removed_value.get(),
        0.0,
        "a controller removed during the tick never applies"
    );
    assert_eq!(
        added_value.get(),
        0.0,
        "a controller added during the tick waits for the next one"
    );

    scheduler.tick(0.25);
    assert_eq!(
        added_value.get(),
        0.25,
        "the newly added controller starts on the next tick"
    );

    scheduler.reset();
    scheduler.register(removed);
    assert_eq!(
        scheduler.active_count(),
        1,
        "reset clears the registry, allowing re-registration"
    );
}

#[test]
fn scheduler_evicts() {
    let scheduler = AnimatorScheduler::default();
    let (controller, _written) =
        float_controller(0.0, 10.0, Animation::new(0.1, AnimationCurve::Linear));
    scheduler.register(as_any(controller));
    assert_eq!(scheduler.active_count(), 1);

    scheduler.tick(1.0);
    assert_eq!(
        scheduler.active_count(),
        0,
        "eviction happens at the end of the same tick"
    );
}

#[test]
fn empty_tick() {
    let scheduler = AnimatorScheduler::default();
    scheduler.tick(0.016);
    assert_eq!(scheduler.active_count(), 0);
}

#[test]
fn reset_drops_everything() {
    let scheduler = AnimatorScheduler::default();
    let (controller, written) =
        float_controller(0.0, 1.0, Animation::new(1.0, AnimationCurve::Linear));
    scheduler.register(as_any(controller));
    assert_eq!(scheduler.active_count(), 1);

    scheduler.reset();
    assert_eq!(scheduler.active_count(), 0);
    assert_eq!(
        written.get(),
        -1.0,
        "reset cancels without applying the target"
    );
}
