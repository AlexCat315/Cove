//! Translation of CoveUIComposeCore/ModifiedContent+Materialise.swift.
use cove_core::scene::{LayoutNode, Node, NodeKey};
use std::rc::Rc;
use crate::{AnyView, IdentifiedView, ModifierReplay, ModifiedContent, SourceLocatedView, View, ViewGraph, ViewKind};

/// Type-erased helper that lets `ViewGraph` materialise (and update) a
/// `ModifiedContent` without knowing its generic parameters at the call site.
pub trait AnyModifiedContent {
    /// Ordinary modifiers share their content's node and therefore its key.
    /// Scope, around and structural modifiers own an anchor and keep child keys
    /// local, which is why they return `None`.
    fn inherited_slot_key(&self) -> Option<NodeKey> {
        None
    }

    fn materialise_into(&self, parent: &Node, layout_parent: Option<&LayoutNode>, graph: &Rc<ViewGraph>) -> Vec<Node>;

    /// Re-apply this modifier to an already-existing node produced for this
    /// `ModifiedContent` slot. Reuses the node, recurses into the wrapped
    /// content, then re-applies the modifier (writes are idempotent setters).
    fn update_in_place(&self, node: &Node, layout_parent: Option<&LayoutNode>, graph: &Rc<ViewGraph>);
}

/// Walks through erased/source wrappers without unwrapping user identity.
pub(crate) fn has_structural_content(view: &dyn View) -> bool {
    if let Some(any) = view.as_any().downcast_ref::<AnyView>() {
        return has_structural_content(any.storage.as_ref());
    }
    if let Some(source) = view.as_any().downcast_ref::<SourceLocatedView>() {
        return has_structural_content(source.content.storage.as_ref());
    }
    if let Some(identified) = view.as_any().downcast_ref::<IdentifiedView>() {
        return has_structural_content(identified.content.storage.as_ref());
    }
    matches!(view.kind(), ViewKind::Structural(_))
}

fn modifier_targets(node: &Node, graph: &ViewGraph) -> Vec<Node> {
    if graph.layout_node(node).is_some() || node.children().is_empty() {
        return vec![node.clone()];
    }
    node.children().into_iter().flat_map(|child| modifier_targets(&child, graph)).collect()
}

fn apply_modifier(modifier: &Rc<dyn crate::ViewModifier>, node: &Node, graph: &Rc<ViewGraph>) {
    retain_modifier(modifier, node, graph);
    for target in modifier_targets(node, graph) {
        modifier.apply_node(&target);
        if let Some(layout) = graph.layout_node(&target) {
            modifier.apply_layout(&layout);
        }
    }
}

/// Transparent components have no layout node of their own; the caller's
/// modifier declarations are stored so they can be replayed after the
/// component rebuilds its body.
fn retain_modifier(modifier: &Rc<dyn crate::ViewModifier>, node: &Node, graph: &Rc<ViewGraph>) {
    if graph.layout_node(node).is_some() || node.children().is_empty() {
        return;
    }
    // The replay list lives on the node itself; keep a separate handle for the
    // callback so the borrow needed by `install` ends before the move.
    let target = node.clone();
    let replay_target = node.clone();
    let graph = Rc::downgrade(graph);
    let modifier = modifier.clone();
    ModifierReplay::install(&target, Rc::new(move || {
        let Some(graph) = graph.upgrade() else { return };
        apply_modifier(&modifier, &replay_target, &graph);
    }));
}

impl ModifiedContent {
    fn content_view(&self) -> &dyn View {
        self.content().storage.as_ref()
    }
}

impl AnyModifiedContent for ModifiedContent {
    fn inherited_slot_key(&self) -> Option<NodeKey> {
        if self.modifier().is_scope_applying() || self.modifier().is_around_applying() || has_structural_content(self.content_view()) {
            return None;
        }
        crate::view_graph::slot_key(self.content_view())
    }

    fn materialise_into(&self, parent: &Node, layout_parent: Option<&LayoutNode>, graph: &Rc<ViewGraph>) -> Vec<Node> {
        // Scope-applying modifiers must expose their value to the content
        // during materialisation, not after, so descendants resolve against the
        // provided theme on first install. Wrap the content in a synthetic,
        // layout-transparent anchor and push the value onto it before recursing.
        if self.modifier().is_scope_applying() {
            let anchor = Node::new();
            anchor.set_is_hit_testable(false);
            anchor.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            parent.add_child(&anchor);
            self.modifier().scope_apply(&anchor);
            let _ = graph.materialise(self.content(), &anchor, layout_parent);
            return vec![anchor];
        }

        if self.modifier().is_around_applying() {
            let anchor = Node::new();
            anchor.set_is_hit_testable(false);
            anchor.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            parent.add_child(&anchor);
            let content = self.content().clone();
            let anchor_for_content = anchor.clone();
            let mut perform = move || {
                let _ = graph.materialise(&content, &anchor_for_content, layout_parent);
            };
            self.modifier().around_apply(&anchor, &mut perform);
            return vec![anchor];
        }

        if has_structural_content(self.content_view()) {
            // A conditional/tuple/array can produce several roots. It needs its
            // own slot: overwriting a child's viewTag would replace that child's
            // user scope with the structural value on the next update.
            let anchor = Node::new();
            anchor.set_is_hit_testable(false);
            anchor.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            parent.add_child(&anchor);
            let _ = graph.materialise(self.content(), &anchor, layout_parent);
            apply_modifier(self.modifier(), &anchor, graph);
            return vec![anchor];
        }

        let nodes = graph.materialise(self.content(), parent, layout_parent);
        for node in &nodes {
            // Override the inner content's tag with the wrapper's tag so the
            // reconciler knows this slot carries a particular modifier
            // combination.
            node.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            apply_modifier(self.modifier(), node, graph);
        }
        nodes
    }

    fn update_in_place(&self, node: &Node, layout_parent: Option<&LayoutNode>, graph: &Rc<ViewGraph>) {
        // Mirror the materialise-time wrapping: scope-applying modifiers own a
        // synthetic anchor whose single child carries the wrapped content.
        if self.modifier().is_scope_applying() {
            node.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            ModifierReplay::reset(node);
            // The content must reconcile even when the provided value is
            // unchanged: this slot was handed a NEW content value (fresh
            // closures and bindings into the owning scope's current state
            // storage). Skipping the recursion strands descendants on captures
            // of storage that `ViewScope::replace_view` has already swapped out.
            let changed = self.modifier().scope_apply(node).unwrap_or(false);
            let previous = graph.environment_reasons();
            if changed {
                let mut reason = cove_core::scene::RecompositionReason::new("environment").with_detail("CompositionLocal");
                if let Some(scope) = graph.active_recomposition_scope() {
                    reason = reason.with_origin_scope(scope);
                }
                let mut reasons = previous.clone();
                reasons.push(reason);
                graph.set_environment_reasons(reasons);
            }
            graph.reconcile_children(node, layout_parent, std::slice::from_ref(self.content()));
            graph.set_environment_reasons(previous);
            return;
        }

        if self.modifier().is_around_applying() {
            node.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            ModifierReplay::reset(node);
            let content = self.content().clone();
            let parent = node.clone();
            let mut perform = move || {
                graph.reconcile_children(&parent, layout_parent, std::slice::from_ref(&content));
            };
            self.modifier().around_apply(node, &mut perform);
            return;
        }

        if has_structural_content(self.content_view()) {
            ModifierReplay::reset(node);
            graph.reconcile_children(node, layout_parent, std::slice::from_ref(self.content()));
            node.set_view_tag(Some(crate::view_graph::slot_tag(self)));
            apply_modifier(self.modifier(), node, graph);
            return;
        }

        // Recurse into the wrapped content first so its primitive's
        // `update_node` runs and any deeper reconciliation happens.
        graph.update_in_place(node, self.content(), layout_parent);
        node.set_view_tag(Some(crate::view_graph::slot_tag(self)));
        apply_modifier(self.modifier(), node, graph);
    }
}
