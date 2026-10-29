//! Translation of CoveUIComposeCore/ViewModifier.swift.
//!
//! Swift models the two special modifier families with marker protocols
//! (`_ScopeApplyingModifier`, `_AroundApplyingModifier`). Rust trait objects
//! cannot be tested for a second protocol, so the same routing decision is
//! exposed through the object-safe capability methods `scope_apply` and
//! `around_apply`.
use cove_core::scene::{LayoutNode, Node};
use std::rc::Rc;
use crate::AnyView;

/// Modifies a view by setting node/layout properties or wrapping its content.
pub trait ViewModifier {
    /// Write directly into the materialised node.
    fn apply_node(&self, _node: &Node) {}
    /// Write into the materialised layout node (padding/frame/flex/...).
    fn apply_layout(&self, _layout: &LayoutNode) {}
    /// Wrap `content` in additional structure. Default is a pass-through.
    fn body(&self, content: AnyView) -> AnyView {
        content
    }
    /// `Some(changed)` when the modifier must push ambient state onto the
    /// wrapper node before its content is materialised or reconciled.
    fn scope_apply(&self, _node: &Node) -> Option<bool> {
        None
    }
    /// `true` for scope-applying modifiers. Checked before the materialiser
    /// routes the content, without running the effect.
    fn is_scope_applying(&self) -> bool {
        false
    }
    /// Install ambient state around the wrapped content's materialisation.
    /// Return `true` when the modifier owns this boundary.
    fn around_apply(&self, _node: &Node, _perform: &mut dyn FnMut()) -> bool {
        false
    }
    /// `true` for around-applying modifiers. Checked before routing.
    fn is_around_applying(&self) -> bool {
        false
    }
}

/// A view paired with a modifier, produced by `ViewModifier` application.
pub struct ModifiedContent {
    content: AnyView,
    modifier: Rc<dyn ViewModifier>,
}
impl ModifiedContent {
    pub fn new(content: AnyView, modifier: impl ViewModifier + 'static) -> Self {
        Self { content, modifier: Rc::new(modifier) }
    }
    pub fn new_boxed(content: AnyView, modifier: Rc<dyn ViewModifier>) -> Self {
        Self { content, modifier }
    }
    pub fn content(&self) -> &AnyView {
        &self.content
    }
    pub fn modifier(&self) -> &Rc<dyn ViewModifier> {
        &self.modifier
    }
}

impl Clone for ModifiedContent {
    fn clone(&self) -> Self {
        Self { content: self.content.clone(), modifier: self.modifier.clone() }
    }
}

impl crate::View for ModifiedContent {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Modified(self)
    }
}

/// Placeholder content passed to a modifier's wrapped body. The graph
/// substitutes the real content at materialisation time; the type exists so
/// ports of wrappers can name their content slot.
pub struct ViewModifierContent;
