//! Translation of CoveUIComposeCore/ViewModifier.swift.
//!
//! Swift models the two special modifier families with marker protocols
//! (`_ScopeApplyingModifier`, `_AroundApplyingModifier`). Rust trait objects
//! cannot be tested for a second protocol, so the same routing decision is
//! exposed through the object-safe capability methods `scope_apply` and
//! `around_apply`.
use crate::AnyView;
use cove_core::scene::{LayoutNode, Node};
use std::rc::Rc;

/// Modifies a view by setting node/layout properties or wrapping its content.
pub trait ViewModifier {
    /// The modifier's type, part of a modified view's structural identity.
    fn modifier_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
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
    applied: Rc<dyn ViewModifier>,
}
impl ModifiedContent {
    pub fn new(content: AnyView, modifier: impl ViewModifier + 'static) -> Self {
        Self {
            content,
            applied: Rc::new(modifier),
        }
    }
    pub fn new_boxed(content: AnyView, modifier: Rc<dyn ViewModifier>) -> Self {
        Self {
            content,
            applied: modifier,
        }
    }
    pub fn content(&self) -> &AnyView {
        &self.content
    }
    /// The applied modifier. Named `modifier_ref` so it does not shadow
    /// `ViewExt::modifier(self, modifier)`, which stacks another modifier.
    pub fn modifier_ref(&self) -> &Rc<dyn ViewModifier> {
        &self.applied
    }
}

impl Clone for ModifiedContent {
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            applied: self.applied.clone(),
        }
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
