//! Translation of Theme/Tokens/SemanticMotion.swift.
use super::Easing;
use crate::theme::Theme;
use cove_core::scene::{Animation, AnimationCurve};
use std::sync::Arc;

/// Late-bound animation reference resolved against the active `Theme`.
#[derive(Clone)]
pub struct SemanticMotionRef {
    resolve: Arc<dyn Fn(&Theme) -> Animation>,
}
impl SemanticMotionRef {
    pub fn new(resolve: impl Fn(&Theme) -> Animation + 'static) -> Self {
        Self {
            resolve: Arc::new(resolve),
        }
    }
    pub fn resolve(&self, theme: &Theme) -> Animation {
        (self.resolve)(theme)
    }
    /// Surfaces arrive on a strong ease-out: they move at once and glide into
    /// place, with no overshoot and a bounded duration.
    pub fn entering() -> Self {
        Self::new(|theme| {
            Animation::new(
                theme.motion.surface.entrance,
                Easing::RESPONSIVE.as_animation_curve(),
            )
        })
    }
    pub fn exiting() -> Self {
        Self::new(|theme| {
            Animation::new(
                theme.motion.surface.exit,
                Easing::RESPONSIVE.as_animation_curve(),
            )
        })
    }
    /// 100 ms in the default themes — micro-interactions, hover/press.
    pub fn fast() -> Self {
        Self::new(|theme| {
            Animation::new(
                theme.motion.fast,
                theme.motion.standard_easing.as_animation_curve(),
            )
        })
    }
    /// 220 ms in the default themes — surface state changes, default choice.
    pub fn medium() -> Self {
        Self::new(|theme| {
            Animation::new(
                theme.motion.standard,
                theme.motion.standard_easing.as_animation_curve(),
            )
        })
    }
    /// 350 ms in the default themes — emphasized motion.
    pub fn slow() -> Self {
        Self::new(|theme| {
            Animation::new(
                theme.motion.slow,
                theme.motion.emphasized.as_animation_curve(),
            )
        })
    }
    /// On-screen movement (a selection sliding to a new row): the iOS sheet
    /// curve over the standard duration — decisive, bounded, no bounce.
    pub fn snappy() -> Self {
        Self::new(|theme| {
            Animation::new(
                theme.motion.standard,
                Easing::FLUID.as_animation_curve(),
            )
        })
    }
    /// Opt-in playful motion. No built-in control uses it.
    pub fn bouncy() -> Self {
        Self::new(|theme| Animation::spring(theme.motion.slow, 0.62).with_overshoot(true))
    }
    pub fn smooth() -> Self {
        Self::new(|theme| {
            Animation::new(theme.motion.slow, Easing::FLUID.as_animation_curve())
        })
    }
}

/// Resolve a semantic motion reference against the supplied theme.
pub fn semantic_animation(reference: &SemanticMotionRef, theme: &Theme) -> Animation {
    reference.resolve(theme)
}

/// Cascade a group of sibling entrances by deferring each item's start.
pub fn staggered(animation: &Animation, index: usize, interval: f64) -> Animation {
    let mut result = animation.clone();
    result.delay = animation.delay.max(0.0) + index as f64 * interval.max(0.0);
    result
}

/// Compatibility helper mirroring `Easing.asAnimationCurve`.
pub fn animation_curve(easing: Easing) -> AnimationCurve {
    easing.as_animation_curve()
}
