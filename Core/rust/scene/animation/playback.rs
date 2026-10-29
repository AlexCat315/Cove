//! Translation of Animation/Playback.swift.
use super::Animation;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationRepetition { #[default] Once, Count(usize, bool), Forever(bool) }
#[derive(Clone, Copy, Debug)]
pub struct Playback { pub time: f32, pub finished: bool }
impl Animation {
    pub fn playback(&self, elapsed: f64) -> Playback {
        let active = (elapsed - self.delay).max(0.0); if self.duration <= 0.0 { return Playback { time: 1.0, finished: elapsed >= self.delay }; }
        let (reversing, cycles) = match self.repetition { AnimationRepetition::Once => return Playback { time: (active / self.duration).min(1.0) as f32, finished: active >= self.duration },
            AnimationRepetition::Count(count, reverses) => (reverses, count.max(1) as f64), AnimationRepetition::Forever(reverses) => (reverses, f64::INFINITY) };
        let cycle_duration = self.duration * if reversing { 2.0 } else { 1.0 };
        if active / cycle_duration >= cycles { return Playback { time: if reversing { 0.0 } else { 1.0 }, finished: true }; }
        let position = (active % cycle_duration) / self.duration; Playback { time: if position <= 1.0 { position } else { 2.0 - position } as f32, finished: false }
    }
}
