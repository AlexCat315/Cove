//! Translation of CoveUIComposeCore/CompositionLocalProvider.swift.
//!
//! Scope-applying modifiers write their effect onto the wrapper node produced
//! by the wrapped content instead of pushing it into descendant layout/render
//! targets. Composition locals must live on an ancestor of every consumer, so
//! the wrapper node is the correct owner whether or not it has a layout node.
use cove_core::scene::Node;
use cove_core::CompositionLocal;
use std::rc::Rc;

/// Marker for modifiers whose effect must be applied to the wrapper node.
pub trait ScopeApplyingModifier {
    /// Returns whether the ambient value changed.
    fn _apply_scope(&self, node: &Node) -> bool;
}

/// Routes a [`ScopeApplyingModifier`] through the object-safe
/// [`crate::ViewModifier`] capability used by `ModifiedContent`.
pub struct ScopeApplying<M: ScopeApplyingModifier> {
    pub modifier: M,
}
impl<M: ScopeApplyingModifier> ScopeApplying<M> {
    pub fn new(modifier: M) -> Self {
        Self { modifier }
    }
}
impl<M: ScopeApplyingModifier + 'static> crate::ViewModifier for ScopeApplying<M> {
    fn scope_apply(&self, node: &Node) -> Option<bool> {
        Some(self.modifier._apply_scope(node))
    }
    fn is_scope_applying(&self) -> bool {
        true
    }
}

/// Provides `Value` for `local` to every descendant of the modified view.
pub struct ProvideCompositionLocalModifier<Value> {
    local: Rc<CompositionLocal<Value>>,
    value: Value,
}
impl<Value> ProvideCompositionLocalModifier<Value> {
    pub fn new(local: Rc<CompositionLocal<Value>>, value: Value) -> Self {
        Self { local, value }
    }
}
impl<Value: Clone + PartialEq + 'static> crate::ViewModifier for ProvideCompositionLocalModifier<Value> {
    fn scope_apply(&self, node: &Node) -> Option<bool> {
        if node.composition_value(&self.local) == self.value {
            return Some(false);
        }
        node.set_composition_value(&self.local, self.value.clone());
        Some(true)
    }
    fn is_scope_applying(&self) -> bool {
        true
    }
}
impl<Value: Clone + PartialEq + 'static> ScopeApplyingModifier for ProvideCompositionLocalModifier<Value> {
    fn _apply_scope(&self, node: &Node) -> bool {
        crate::ViewModifier::scope_apply(self, node).unwrap_or(false)
    }
}
