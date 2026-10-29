//! Translation of Animation/Keyframes/Timeline.swift.
use super::{Animation, AnimationCurve, Interpolatable};
#[derive(Clone)]
pub struct AnimationKeyframe<T: Interpolatable> { pub value: T, pub animation: Animation }
impl<T: Interpolatable> AnimationKeyframe<T> {
    pub fn new(value: T, duration: f64, curve: AnimationCurve) -> Self { assert!(duration.is_finite() && duration > 0.0, "keyframe duration must be finite and positive"); Self { value, animation: Animation::new(duration, curve).with_overshoot(true) } }
}
#[derive(Clone)]
pub struct KeyframeTimeline<T: Interpolatable> { pub initial_value: T, keyframes: Vec<AnimationKeyframe<T>>, ends: Vec<f64> }
impl<T: Interpolatable> KeyframeTimeline<T> {
    pub fn new(initial_value: T, keyframes: Vec<AnimationKeyframe<T>>) -> Self {
        assert!(!keyframes.is_empty(), "a timeline requires a keyframe"); let mut total = 0.0; let ends = keyframes.iter().map(|k| { total += k.animation.duration; total }).collect();
        assert!(total.is_finite(), "timeline duration must be finite"); Self { initial_value, keyframes, ends }
    }
    pub fn duration(&self) -> f64 { self.ends.last().copied().unwrap_or(0.0) }
    pub fn value(&self, time: f64) -> T {
        if time.is_nan() || time <= 0.0 { return self.initial_value.clone(); } if time >= self.duration() { return self.keyframes.last().unwrap().value.clone(); }
        let lower = self.ends.partition_point(|&end| end <= time); let segment = &self.keyframes[lower];
        let start_time = if lower == 0 { 0.0 } else { self.ends[lower - 1] }; let start = if lower == 0 { &self.initial_value } else { &self.keyframes[lower - 1].value };
        T::interpolate(start, &segment.value, segment.animation.curve.evaluate(((time - start_time) / segment.animation.duration) as f32))
    }
}
