//! Process-wide animation time scale, the runtime counterpart of Flutter's
//! `timeDilation`.
//!
//! Only the delta handed to the animator scheduler is scaled. Frame pacing,
//! input and diagnostics keep measuring wall-clock time, so slowing an
//! animation does not slow the surface, the pointer or the frame chart.
use std::sync::atomic::{AtomicU64, Ordering};

/// Real time: one animation second per wall-clock second.
pub const DEFAULT_ANIMATION_TIME_SCALE: f64 = 1.0;
/// Slowest accepted scale, one twentieth of real time.
pub const MIN_ANIMATION_TIME_SCALE: f64 = 0.05;
/// Fastest accepted scale, ten times real time.
pub const MAX_ANIMATION_TIME_SCALE: f64 = 10.0;

static ANIMATION_TIME_SCALE: AtomicU64 = AtomicU64::new(DEFAULT_ANIMATION_TIME_SCALE.to_bits());

/// A finite scale inside the accepted range; non-finite input falls back to
/// real time instead of poisoning every later frame.
pub fn clamp_animation_time_scale(scale: f64) -> f64 {
    if scale.is_finite() {
        scale.clamp(MIN_ANIMATION_TIME_SCALE, MAX_ANIMATION_TIME_SCALE)
    } else {
        DEFAULT_ANIMATION_TIME_SCALE
    }
}

/// The scale every host applies to its animation delta.
pub fn animation_time_scale() -> f64 {
    f64::from_bits(ANIMATION_TIME_SCALE.load(Ordering::Acquire))
}

/// Applies `scale` after clamping and returns the value that took effect.
pub fn set_animation_time_scale(scale: f64) -> f64 {
    let applied = clamp_animation_time_scale(scale);
    ANIMATION_TIME_SCALE.store(applied.to_bits(), Ordering::Release);
    applied
}

/// Returns the runtime to real time; DevTools teardown always does this.
pub fn reset_animation_time_scale() -> f64 {
    set_animation_time_scale(DEFAULT_ANIMATION_TIME_SCALE)
}

/// Multiplies an animation delta by an explicit scale.
///
/// A delta the scheduler would reject (non-finite or negative) passes through
/// unchanged so the scheduler stays the single place that validates it.
pub fn scaled_animation_delta(delta: f64, scale: f64) -> f64 {
    if !delta.is_finite() || delta <= 0.0 {
        return delta;
    }
    delta * clamp_animation_time_scale(scale)
}

/// The delta a host ticks with under the current global scale.
pub fn scale_animation_delta(delta: f64) -> f64 {
    scaled_animation_delta(delta, animation_time_scale())
}
