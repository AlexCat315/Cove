//! Translation of Theme/Tokens/MotionScale.swift.
use cove_core::scene::AnimationCurve;

/// Authored Bezier control points, resolved to runtime curves at the use site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Easing {
    pub c1x: f32,
    pub c1y: f32,
    pub c2x: f32,
    pub c2y: f32,
}
impl Easing {
    pub fn new(c1x: f32, c1y: f32, c2x: f32, c2y: f32) -> Self {
        Self { c1x, c1y, c2x, c2y }
    }
    pub const fn cubic_bezier(c1x: f32, c1y: f32, c2x: f32, c2y: f32) -> Self {
        Self { c1x, c1y, c2x, c2y }
    }
    pub const STANDARD: Self = Self::cubic_bezier(0.4, 0.0, 0.2, 1.0);
    pub const EMPHASIZED: Self = Self::cubic_bezier(0.2, 0.0, 0.0, 1.0);
    pub const DECELERATE: Self = Self::cubic_bezier(0.0, 0.0, 0.2, 1.0);
    /// Strong ease-out: moves at once, then glides into place.
    pub const RESPONSIVE: Self = Self::cubic_bezier(0.23, 1.0, 0.32, 1.0);
    /// The iOS sheet curve for on-screen movement.
    pub const FLUID: Self = Self::cubic_bezier(0.32, 0.72, 0.0, 1.0);

    /// Convert the authored cubic-bezier descriptor into a runtime curve.
    pub fn as_animation_curve(self) -> AnimationCurve {
        AnimationCurve::CubicBezier(self.c1x, self.c1y, self.c2x, self.c2y)
    }
}

/// Floating surfaces share entrance, exit and reveal sizing across components.
/// Surfaces grow out of `reveal_scale` on a short ease-out and leave faster
/// than they arrive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceMotion {
    /// Seconds.
    pub entrance: f64,
    /// Seconds.
    pub exit: f64,
    pub reveal_scale: f32,
}
impl Default for SurfaceMotion {
    fn default() -> Self {
        Self {
            entrance: 0.2,
            exit: 0.12,
            reveal_scale: 0.96,
        }
    }
}

/// Theme-owned timing and easing. Surface presentation and press feedback own
/// their defaults here; components resolve against the active node's theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionScale {
    /// Seconds.
    pub fast: f64,
    /// Seconds.
    pub standard: f64,
    /// Seconds.
    pub slow: f64,
    pub emphasized: Easing,
    pub standard_easing: Easing,
    pub surface: SurfaceMotion,
    pub press_scale: f32,
}
impl Default for MotionScale {
    fn default() -> Self {
        Self::new(0.1, 0.22, 0.35, Easing::FLUID, Easing::RESPONSIVE)
    }
}
impl MotionScale {
    pub fn new(
        fast: f64,
        standard: f64,
        slow: f64,
        emphasized: Easing,
        standard_easing: Easing,
    ) -> Self {
        Self {
            fast,
            standard,
            slow,
            emphasized,
            standard_easing,
            surface: SurfaceMotion::default(),
            press_scale: 0.97,
        }
    }
}
