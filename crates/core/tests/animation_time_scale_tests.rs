//! The global animation time scale every host applies to its animation delta.
//!
//! Flutter's `timeDilation` slows the app's animation clock, not its frame
//! clock. These tests pin the same split: the scale is clamped and global,
//! invalid deltas are left for the scheduler to reject, and the pure delta
//! helper only touches valid animation time.

use cove_core::scene::{
    Animation, AnimationController, AnimationCurve, AnimatorScheduler, AnyAnimationController,
    DEFAULT_ANIMATION_TIME_SCALE, MAX_ANIMATION_TIME_SCALE, MIN_ANIMATION_TIME_SCALE,
    animation_time_scale, clamp_animation_time_scale, reset_animation_time_scale,
    scale_animation_delta, scaled_animation_delta, set_animation_time_scale,
};
use std::cell::Cell;
use std::rc::Rc;

/// Restores real time even when an assertion panics mid-test.
struct RealTime;
impl Drop for RealTime {
    fn drop(&mut self) {
        reset_animation_time_scale();
    }
}

#[test]
fn the_global_scale_applies_to_the_tick_delta_and_resets_to_real_time() {
    let _real_time = RealTime;
    assert_eq!(animation_time_scale(), DEFAULT_ANIMATION_TIME_SCALE);
    assert_eq!(set_animation_time_scale(0.25), 0.25);
    assert_eq!(animation_time_scale(), 0.25);
    assert_eq!(scale_animation_delta(0.5), 0.125);
    assert_eq!(scale_animation_delta(1.0 / 60.0), (1.0 / 60.0) * 0.25);
    // The setter reports the clamped value it stored, not the input.
    assert_eq!(set_animation_time_scale(1_000.0), MAX_ANIMATION_TIME_SCALE);
    assert_eq!(animation_time_scale(), MAX_ANIMATION_TIME_SCALE);
    assert_eq!(reset_animation_time_scale(), DEFAULT_ANIMATION_TIME_SCALE);
    assert_eq!(animation_time_scale(), DEFAULT_ANIMATION_TIME_SCALE);
    assert_eq!(scale_animation_delta(0.5), 0.5);
}

#[test]
fn scales_clamp_to_the_safe_range_and_non_finite_values_fall_back_to_real_time() {
    assert_eq!(clamp_animation_time_scale(0.0), MIN_ANIMATION_TIME_SCALE);
    assert_eq!(clamp_animation_time_scale(0.01), MIN_ANIMATION_TIME_SCALE);
    assert_eq!(clamp_animation_time_scale(100.0), MAX_ANIMATION_TIME_SCALE);
    assert_eq!(
        clamp_animation_time_scale(f64::NAN),
        DEFAULT_ANIMATION_TIME_SCALE
    );
    assert_eq!(
        clamp_animation_time_scale(f64::INFINITY),
        DEFAULT_ANIMATION_TIME_SCALE
    );
    assert_eq!(
        clamp_animation_time_scale(f64::NEG_INFINITY),
        DEFAULT_ANIMATION_TIME_SCALE
    );
    assert_eq!(clamp_animation_time_scale(0.5), 0.5);
    assert_eq!(clamp_animation_time_scale(2.0), 2.0);
}

#[test]
fn a_scaled_delta_slows_a_scheduled_animation() {
    let scheduler = AnimatorScheduler::default();
    let written = Rc::new(Cell::new(-1.0_f32));
    let sink = written.clone();
    let controller: Rc<dyn AnyAnimationController> = AnimationController::new(
        0.0_f32,
        10.0,
        Animation::new(1.0, AnimationCurve::Linear),
        0.0,
        move |value| sink.set(value),
    );
    scheduler.register(controller);

    // At half speed two 250 ms wall-clock ticks advance animation time by
    // 250 ms in total, so the linear 1 s animation reaches a quarter of its
    // range instead of going past it.
    scheduler.tick(scaled_animation_delta(0.25, 0.5));
    scheduler.tick(scaled_animation_delta(0.25, 0.5));

    assert_eq!(written.get(), 2.5);
}

#[test]
fn delta_scaling_only_multiplies_valid_animation_time() {
    let delta = 1.0 / 60.0;
    assert_eq!(scaled_animation_delta(delta, 0.5), delta / 2.0);
    assert_eq!(scaled_animation_delta(delta, 2.0), delta * 2.0);
    // An explicit scale outside the range is clamped before it is applied.
    assert_eq!(
        scaled_animation_delta(delta, 100.0),
        delta * MAX_ANIMATION_TIME_SCALE
    );
    assert_eq!(
        scaled_animation_delta(delta, 0.0),
        delta * MIN_ANIMATION_TIME_SCALE
    );
    // The scheduler rejects these deltas itself; scaling must not hide them.
    assert_eq!(scaled_animation_delta(-0.5, 2.0), -0.5);
    assert!(scaled_animation_delta(f64::NAN, 2.0).is_nan());
    assert!(scaled_animation_delta(f64::INFINITY, 2.0).is_infinite());
}
