//! Translation of Theme/Tokens/ReducedMotionEnvironment.swift.
use crate::{ProvideCompositionLocalModifier, View, ViewExt};
use cove_core::scene::ReducedMotionEnvironment;

pub trait ReducedMotionViewExt: View + Sized + 'static {
    /// Marks the subtree as preferring reduced motion.
    fn reduced_motion(self, enabled: bool) -> crate::ModifiedContent {
        self.modifier(ProvideCompositionLocalModifier::new(
            ReducedMotionEnvironment::key(),
            enabled,
        ))
    }
}
impl<T: View + Sized + 'static> ReducedMotionViewExt for T {}
