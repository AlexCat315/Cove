//! Translation of CoveUIComposeCore/AroundApplyingModifier.swift.
//!
//! Modifiers implementing [`AroundApplyingModifier`] wrap the materialisation
//! of their content: they install ambient state, invoke `perform` exactly once
//! (which materialises or reconciles the content), then tear that state down.
//! The materialiser gives each slot a synthetic anchor node so the modifier has
//! a stable place to cache state across recompositions.
//!
//! Routing rule: around-applying modifiers are never consulted through
//! `apply(node:)` / `apply(layout:)`; their behaviour is delivered exclusively
//! through `around_apply`.
use cove_core::scene::Node;

pub trait AroundApplyingModifier {
    fn _around_apply(&self, node: &Node, perform: &mut dyn FnMut());
}

/// Routes an [`AroundApplyingModifier`] through the object-safe
/// [`crate::ViewModifier`] capability used by `ModifiedContent`.
pub struct AroundApplying<M: AroundApplyingModifier> {
    pub modifier: M,
}
impl<M: AroundApplyingModifier> AroundApplying<M> {
    pub fn new(modifier: M) -> Self {
        Self { modifier }
    }
}
impl<M: AroundApplyingModifier + 'static> crate::ViewModifier for AroundApplying<M> {
    fn around_apply(&self, node: &Node, perform: &mut dyn FnMut()) -> bool {
        self.modifier._around_apply(node, perform);
        true
    }
    fn is_around_applying(&self) -> bool {
        true
    }
}
