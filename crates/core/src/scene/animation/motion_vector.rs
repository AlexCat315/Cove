//! Translation of Animation/MotionVector.swift.
use super::Interpolatable;
#[derive(Clone)]
pub struct MotionVector<T: Interpolatable> {
    pub from: T,
    pub to: T,
    pub rate: f32,
}
impl<T: Interpolatable> MotionVector<T> {
    pub fn adding(&self, base: &T, seconds: f32) -> T {
        let displaced = T::interpolate(&self.from, &self.to, self.rate * seconds);
        let midpoint = T::interpolate(base, &displaced, 0.5);
        T::interpolate(&self.from, &midpoint, 2.0)
    }
}
