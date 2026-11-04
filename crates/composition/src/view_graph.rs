//! Translation of CoveUIComposeCore/ViewGraph.swift.
//!
//! Materialises `View` trees into the underlying `NodeTree`, and recomposes
//! affected subtrees when state writes invalidate them through `Recomposer`.
//!
//! Reconcile strategy: each `Node` carries a `viewTag` recording the type of
//! view that produced it. On recompose the graph walks new and existing
//! children in lockstep; matching tags reuse the node and re-run `update_node`,
//! keeping primitive state in `Node` attachments. A mismatch tears down the old
//! tail from that index and materialises the new tail.
use cove_core::scene::{
    ActiveAnimationContext, ComponentSourceLocation, Direction, DirtyReason, ElementId,
    InvalidationLog, InvalidationLogHolder, InvalidationPhase, InvalidationSource, LayoutNode,
    LocalInvalidationHandle, Node, NodeKey, NodeTree, ObservableStateTracking, RecompositionReason,
    RenderTree, StateRegistry, TextInputAreaResolver, TextInputAttachmentKey,
};
use cove_core::{Observer, Point, Rect, with_deferred_evaluation};
use cove_platform::TextInputArea;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use crate::layout_debug_modifier::LayoutDebugAttachmentKey;
use crate::{
    AnyView, AsAny, DynamicField, EmptyView, IdentifiedView, LayoutTree, ModifierReplay,
    PrimitiveView, Scope, SourceLocatedView, View, ViewKind,
};

pub struct LayoutSnapshotEntry {
    pub debug_name: Option<String>,
    pub layout_role: Option<String>,
    pub semantic_role: Option<String>,
    pub frame: Rect,
    pub absolute_frame: Rect,
}

pub struct ViewGraph {
    pub tree: Rc<RefCell<NodeTree>>,
    pub recomposer: Rc<cove_core::scene::Recomposer>,
    state_registry: RefCell<StateRegistry>,
    pub layout_tree: LayoutTree,
    render_tree: RenderTree,
    /// `Node` → paired `LayoutNode`. Anchors are absent from this map.
    layout_of: RefCell<HashMap<ElementId, LayoutNode>>,
    /// Active user-view scopes keyed by their anchor node identity.
    scopes: RefCell<HashMap<ElementId, Rc<RefCell<ViewScope>>>>,
    pub tracks_recomposition: Cell<bool>,
    skipped_parent_updates: Cell<usize>,
    active_recomposition_scope: Cell<Option<u64>>,
    environment_reasons: RefCell<Vec<RecompositionReason>>,
    last_layout_size: RefCell<Option<(f32, f32)>>,
}

impl ViewGraph {
    pub fn new(tree: Rc<RefCell<NodeTree>>, recomposer: Rc<cove_core::scene::Recomposer>) -> Self {
        recomposer.set_timeline(Some(tree.borrow().timeline.clone()));
        Self {
            tree,
            recomposer,
            state_registry: RefCell::new(StateRegistry::new()),
            layout_tree: LayoutTree::new(),
            render_tree: RenderTree::new(),
            layout_of: RefCell::new(HashMap::new()),
            scopes: RefCell::new(HashMap::new()),
            tracks_recomposition: Cell::new(true),
            skipped_parent_updates: Cell::new(0),
            active_recomposition_scope: Cell::new(None),
            environment_reasons: RefCell::new(Vec::new()),
            last_layout_size: RefCell::new(None),
        }
    }

    pub fn layout_root(&self) -> LayoutNode {
        self.layout_tree.root.clone()
    }

    /// Registered state metadata, for diagnostics.
    pub fn state_descriptors(&self) -> Vec<cove_core::scene::StateDescriptor> {
        self.state_registry.borrow().descriptors()
    }

    /// Current values of the watched state, for diagnostics.
    pub fn state_values(&self, ids: &[String]) -> Vec<cove_core::scene::StateValue> {
        self.state_registry.borrow().values(ids)
    }

    pub fn render_tree(&self) -> &RenderTree {
        &self.render_tree
    }

    pub fn skipped_parent_updates(&self) -> usize {
        self.skipped_parent_updates.get()
    }

    pub(crate) fn active_recomposition_scope(&self) -> Option<u64> {
        self.active_recomposition_scope.get()
    }
    pub(crate) fn environment_reasons(&self) -> Vec<RecompositionReason> {
        self.environment_reasons.borrow().clone()
    }
    pub(crate) fn set_environment_reasons(&self, reasons: Vec<RecompositionReason>) {
        *self.environment_reasons.borrow_mut() = reasons;
    }

    pub(crate) fn timeline_begin(&self) -> Option<f64> {
        self.tree.borrow().timeline.borrow().begin()
    }
    pub(crate) fn timeline_end(
        &self,
        start: Option<f64>,
        phase: &str,
        name: &str,
        scope: Option<String>,
        reasons: &[RecompositionReason],
    ) {
        self.tree
            .borrow()
            .timeline
            .borrow_mut()
            .end(start, phase, name, scope, reasons);
    }
    /// Number of live user-view scopes, exposed for diagnostics.
    pub fn scope_count(&self) -> usize {
        self.scopes.borrow().len()
    }

    // MARK: - Install

    /// Install a root, releasing the previous tree's scopes, layout and resources.
    pub fn install<V: View + 'static>(self: &Rc<Self>, root: V, location: ComponentSourceLocation) {
        let view = AnyView::new(root);
        self.install_view(view, location);
    }

    pub fn install_view(self: &Rc<Self>, view: AnyView, location: ComponentSourceLocation) {
        if let Some(previous) = self.tree.borrow().root() {
            let layout_root = self.layout_root();
            for child in previous.children() {
                self.tear_down(&child, Some(&layout_root));
            }
            if let Some(scope) = self.scopes.borrow_mut().remove(&previous.id()) {
                ViewScope::uninstall(&scope, self);
            }
            self.layout_of.borrow_mut().remove(&previous.id());
            self.render_tree.tear_down(&previous);
        }
        let root_node = Node::new();
        self.tree.borrow_mut().set_root(Some(root_node.clone()));
        self.layout_of
            .borrow_mut()
            .insert(root_node.id(), self.layout_root());
        let nodes = if ViewScope::is_materialised_directly(view.storage.as_ref())
            && !is_empty_view(view.storage.as_ref())
        {
            // Primitive/structural roots have no user-view anchor. Their reader
            // closures still need an owner, without adding a layout container.
            let layout_parent = self.layout_root();
            let scope = Rc::new(RefCell::new(ViewScope::new(
                self,
                &root_node,
                view,
                Some(layout_parent),
                None,
            )));
            self.scopes
                .borrow_mut()
                .insert(root_node.id(), scope.clone());
            ViewScope::install(&scope, self);
            root_node.children()
        } else {
            let layout_parent = self.layout_root();
            self.materialise(&view, &root_node, Some(&layout_parent))
        };
        for node in nodes {
            if node.source_location().is_none() {
                node.set_source_location(Some(location.clone()));
            }
        }
        self.render_tree.install(&root_node);
    }

    // MARK: - Layout

    /// Run a layout pass over the layout tree and write the resulting frames
    /// back to the corresponding `Node`s. Call once per frame after
    /// `recomposer.commit_all()` and before draw.
    pub fn compute_layout(&self, width: f32, height: f32) {
        let trace = self.timeline_begin();
        *self.last_layout_size.borrow_mut() = Some((width, height));
        with_deferred_evaluation(|| {
            self.layout_tree
                .root
                .calculate_layout(Some(width), Some(height), Direction::Ltr);
            if let Some(root) = self.tree.borrow().root() {
                self.write_layout_back(&root, Point::new(0.0, 0.0), true);
            }
        });
        self.timeline_end(trace, "layout", "Flex layout", None, &[]);
    }

    /// Run layout only when the layout tree is dirty or the viewport changed.
    pub fn compute_layout_if_needed(&self, width: f32, height: f32) -> bool {
        if !self.layout_needs_update(width, height) {
            return false;
        }
        self.compute_layout(width, height);
        true
    }

    pub fn layout_needs_update(&self, width: f32, height: f32) -> bool {
        let Some(last) = *self.last_layout_size.borrow() else {
            return true;
        };
        if last.0 != width || last.1 != height {
            return true;
        }
        self.layout_tree.root.subtree_is_dirty()
    }

    fn write_layout_back(&self, node: &Node, parent_origin: Point, parent_placement_visible: bool) {
        self.write_layout_back_at(node, parent_origin, parent_placement_visible);
    }

    /// Layout frames are parent-relative, as Yoga reports them, and the nearest
    /// layout ancestor is the layout parent: nodes without a layout node keep a
    /// zero frame, so `Node::absolute_frame` composes ancestor origins exactly.
    fn write_layout_back_at(
        &self,
        node: &Node,
        parent_origin: Point,
        parent_placement_visible: bool,
    ) {
        let layout = self.layout_of.borrow().get(&node.id()).cloned();
        let placement_visible = parent_placement_visible
            && layout
                .as_ref()
                .is_none_or(LayoutNode::custom_layout_is_visible);
        if let Some(layout) = &layout {
            node.set_layout_visibility(layout.custom_layout_is_visible());
            // Hidden retained slots are not measured. Publish suppressed
            // geometry rather than uninitialized probe frames until placed.
            node.set_frame(if placement_visible {
                layout.frame()
            } else {
                Rect::default()
            });
        }
        let absolute_origin = Point::new(
            parent_origin.x + node.frame().x,
            parent_origin.y + node.frame().y,
        );
        self.sync_text_input_area(node, absolute_origin);
        // Children render translated by -content_offset; keep the IME anchor
        // geometry in the same space as what is actually on screen.
        let child_origin = Point::new(
            absolute_origin.x - node.content_offset().x,
            absolute_origin.y - node.content_offset().y,
        );
        for child in node.children() {
            self.write_layout_back_at(&child, child_origin, placement_visible);
        }
        if let Some(hook) = node.layout_did_update() {
            hook(node);
        }
    }

    fn sync_text_input_area(&self, node: &Node, absolute_origin: Point) {
        let previous: Option<TextInputArea> = node.attachment(TextInputAttachmentKey::AREA);
        let resolver: Option<TextInputAreaResolver> =
            node.attachment(TextInputAttachmentKey::AREA_RESOLVER);
        let next = resolver.and_then(|resolver| resolver(node, absolute_origin));
        if previous == next {
            return;
        }
        match next {
            Some(area) => node.set_attachment(TextInputAttachmentKey::AREA, area),
            None => node.remove_attachment(TextInputAttachmentKey::AREA),
        }
    }

    /// Layout node paired with `node`, if any.
    pub fn layout_node(&self, node: &Node) -> Option<LayoutNode> {
        self.layout_of.borrow().get(&node.id()).cloned()
    }

    pub fn layout_snapshot(&self) -> Vec<LayoutSnapshotEntry> {
        let Some(root) = self.tree.borrow().root() else {
            return Vec::new();
        };
        let mut result = Vec::new();
        self.collect_layout_snapshot(&root, Point::new(0.0, 0.0), &mut result);
        result
    }

    fn collect_layout_snapshot(
        &self,
        node: &Node,
        parent_origin: Point,
        result: &mut Vec<LayoutSnapshotEntry>,
    ) {
        let absolute_origin = Point::new(
            parent_origin.x + node.frame().x,
            parent_origin.y + node.frame().y,
        );
        let absolute_frame = Rect::new(
            absolute_origin.x,
            absolute_origin.y,
            node.frame().width,
            node.frame().height,
        );
        let child_origin = Point::new(
            absolute_origin.x - node.content_offset().x,
            absolute_origin.y - node.content_offset().y,
        );
        let debug_name: Option<String> = node.attachment(LayoutDebugAttachmentKey::DEBUG_NAME);
        let layout_role: Option<String> = node.attachment(LayoutDebugAttachmentKey::LAYOUT_ROLE);
        let semantic_role: Option<String> =
            node.attachment(LayoutDebugAttachmentKey::SEMANTIC_ROLE);
        if debug_name.is_some() || layout_role.is_some() || semantic_role.is_some() {
            result.push(LayoutSnapshotEntry {
                debug_name,
                layout_role,
                semantic_role,
                frame: node.frame(),
                absolute_frame,
            });
        }
        for child in node.children() {
            self.collect_layout_snapshot(&child, child_origin, result);
        }
    }

    // MARK: - Reconcile entry points

    /// Reconcile `parent.children` against the slot list produced by `new_views`.
    pub fn reconcile_children(
        self: &Rc<Self>,
        parent: &Node,
        layout_parent: Option<&LayoutNode>,
        new_views: &[AnyView],
    ) {
        let flat = flatten_slots(new_views);
        let old_children = parent.children();
        if let Some(entries) = self.stable_children_fast_path(&flat, &old_children) {
            for (entry, node) in entries.iter().zip(old_children.iter()) {
                self.update_in_place(node, &entry.view, layout_parent);
            }
            return;
        }

        let entries: Vec<SlotInfo> = flat.iter().map(classify).collect();

        // Build matching tables.
        let mut keyed_old: HashMap<(String, NodeKey), Node> = HashMap::new();
        let mut unkeyed_queues: HashMap<String, UnkeyedSlotQueue> = HashMap::new();
        for child in &old_children {
            match (child.key(), child.view_tag()) {
                (Some(key), Some(tag)) => {
                    keyed_old.insert((tag, key), child.clone());
                }
                (None, Some(tag)) => unkeyed_queues.entry(tag).or_default().append(child.clone()),
                _ => {}
            }
        }

        enum Action {
            Reuse(Node),
            Create,
        }
        let mut actions: Vec<Action> = Vec::with_capacity(entries.len());
        let mut matched: HashSet<ElementId> = HashSet::new();

        // First pass: keyed matches consume their slot.
        for entry in &entries {
            if let Some(key) = &entry.key
                && let Some(found) = keyed_old.get(&(entry.tag.clone(), key.clone()))
            {
                matched.insert(found.id());
                actions.push(Action::Reuse(found.clone()));
            } else {
                actions.push(Action::Create);
            }
        }

        // Second pass: unkeyed entries pull from their per-tag queue.
        for (index, entry) in entries.iter().enumerate() {
            if !matches!(actions[index], Action::Create) || entry.key.is_some() {
                continue;
            }
            let Some(queue) = unkeyed_queues.get_mut(&entry.tag) else {
                continue;
            };
            if let Some(picked) = queue.pop_skipping(&matched) {
                matched.insert(picked.id());
                actions[index] = Action::Reuse(picked);
            }
        }

        // Tear down old children no entry claimed.
        for child in &old_children {
            if !matched.contains(&child.id()) {
                self.tear_down(child, layout_parent);
            }
        }

        // Materialise fresh entries — these append to parent.children and to
        // layout_parent. Capture the resolved Node per entry for reordering.
        let mut resolved: Vec<Option<Node>> = vec![None; entries.len()];
        for (index, action) in actions.iter().enumerate() {
            match action {
                Action::Reuse(node) => resolved[index] = Some(node.clone()),
                Action::Create => {
                    let made = self.materialise(&entries[index].view, parent, layout_parent);
                    // Stamp the explicit key so the next reconcile can match it.
                    for node in &made {
                        node.set_key(entries[index].key.clone());
                    }
                    resolved[index] = made.first().cloned();
                }
            }
        }

        // Reorder parent.children so the final sequence matches `entries`.
        let ordered: Vec<Node> = resolved.into_iter().flatten().collect();
        if ordered.len() == parent.children().len() {
            parent.reorder_children(&ordered);
        }

        // Reorder layout siblings to match. Anchor nodes carry no LayoutNode,
        // so collect the top-level layout nodes they produced under this parent.
        if let Some(layout_parent) = layout_parent {
            let ordered_layouts: Vec<LayoutNode> = ordered
                .iter()
                .flat_map(|node| self.top_level_layouts(node))
                .collect();
            if ordered_layouts.len() == layout_parent.children().len() {
                layout_parent.reorder_children(&ordered_layouts);
            }
        }

        // Refresh reused nodes in place after the sibling order is settled.
        for (index, action) in actions.iter().enumerate() {
            if let Action::Reuse(node) = action {
                node.set_key(entries[index].key.clone());
                self.update_in_place(node, &entries[index].view, layout_parent);
            }
        }

        self.render_tree.reconcile_children(parent);
    }

    fn stable_children_fast_path(
        &self,
        flat: &[AnyView],
        old_children: &[Node],
    ) -> Option<Vec<SlotInfo>> {
        if flat.len() != old_children.len() {
            return None;
        }
        if flat.is_empty() {
            return Some(Vec::new());
        }
        let mut entries = Vec::with_capacity(flat.len());
        for (index, view) in flat.iter().enumerate() {
            let entry = classify(view);
            let old = &old_children[index];
            if old.view_tag().as_deref() != Some(entry.tag.as_str()) || old.key() != entry.key {
                return None;
            }
            entries.push(entry);
        }
        Some(entries)
    }

    /// Refresh the properties of an already-materialised node from `view`, then
    /// recurse into its children. The caller has verified the slot tag.
    pub fn update_in_place(
        self: &Rc<Self>,
        node: &Node,
        view: &AnyView,
        layout_parent: Option<&LayoutNode>,
    ) {
        let inner = view.storage.as_ref();
        if let Some(any) = inner.as_any().downcast_ref::<AnyView>() {
            self.update_in_place(node, any, layout_parent);
            return;
        }
        if let Some(source) = inner.as_any().downcast_ref::<SourceLocatedView>() {
            node.set_source_location(None);
            self.update_in_place(node, &source.content, layout_parent);
            if node.source_location().is_none() || source.location.explicit {
                node.set_source_location(Some(source.location.clone()));
            }
            return;
        }
        if let Some(identified) = inner.as_any().downcast_ref::<IdentifiedView>() {
            if is_direct_structural_content(identified.content.storage.as_ref()) {
                self.reconcile_children(
                    node,
                    layout_parent,
                    std::slice::from_ref(&identified.content),
                );
            } else {
                self.update_in_place(node, &identified.content, layout_parent);
            }
            node.set_key(Some(identified.id.clone()));
            return;
        }

        match inner.kind() {
            ViewKind::Primitive(primitive) => {
                ModifierReplay::reset(node);
                ObservableStateTracking::observe_reads(|| primitive.update_node(node));
                let my_layout = self.layout_of.borrow().get(&node.id()).cloned();
                if let Some(layout) = &my_layout {
                    ObservableStateTracking::observe_reads(|| primitive.update_layout(layout));
                }
                let children = ObservableStateTracking::observe_reads(|| primitive.children(node));
                let child_layout = my_layout.or_else(|| layout_parent.cloned());
                self.reconcile_children(node, child_layout.as_ref(), &children);
            }
            ViewKind::Modified(modified) => {
                modified.update_in_place(node, layout_parent, self);
            }
            _ => {
                // User view: replace the scope's view value and recompose its
                // body. State storage in the new value is copied from the old
                // so the existing scope keeps its values.
                let scope = self.scopes.borrow().get(&node.id()).cloned();
                if let Some(scope) = scope {
                    // The parent rebuilds its modifier declarations even when
                    // the body is reused. Retaining the previous replay list
                    // would append stale callbacks on every skipped update.
                    ModifierReplay::reset(node);
                    let same_inputs = self.environment_reasons.borrow().is_empty() && {
                        let scope = scope.borrow();
                        crate::view_input_comparison::ViewInputComparison::equal(
                            scope.view.storage.as_ref(),
                            inner,
                        )
                    };
                    if same_inputs {
                        self.skipped_parent_updates
                            .set(self.skipped_parent_updates.get() + 1);
                        return;
                    }
                    let mut reasons = Vec::new();
                    let mut parent_reason = RecompositionReason::new("parent");
                    if let Some(active) = self.active_recomposition_scope.get() {
                        parent_reason = parent_reason.with_origin_scope(active);
                    }
                    reasons.push(parent_reason);
                    reasons.extend(self.environment_reasons.borrow().iter().cloned());
                    ViewScope::replace_view(&scope, self, view.clone());
                    ViewScope::recompose(&scope, self, reasons);
                }
            }
        }
    }

    /// Remove a node from the tree, drop its layout bookkeeping, drop any
    /// user-view scope rooted at it, and drop render-mirror entries.
    pub fn tear_down(&self, node: &Node, parent_layout: Option<&LayoutNode>) {
        self.tear_down_subtree_bookkeeping(node, parent_layout);
        self.render_tree.tear_down(node);
        node.remove_from_parent();
    }

    fn tear_down_subtree_bookkeeping(&self, node: &Node, parent_layout: Option<&LayoutNode>) {
        let id = node.id();
        let my_layout = self.layout_of.borrow_mut().remove(&id);
        if let Some(layout) = &my_layout
            && let Some(parent) = parent_layout
        {
            parent.remove_child(layout);
        }
        let child_layout_parent = my_layout.as_ref().or(parent_layout);
        if let Some(scope) = self.scopes.borrow_mut().remove(&id) {
            ViewScope::uninstall(&scope, self);
            ObservableStateTracking::remove_scope(cove_core::ReadScopeId(id.0));
        }
        for child in node.children() {
            self.tear_down_subtree_bookkeeping(&child, child_layout_parent);
        }
    }

    // MARK: - Materialise

    /// Materialise `view` into `parent` and return the top-level nodes added.
    pub fn materialise(
        self: &Rc<Self>,
        view: &AnyView,
        parent: &Node,
        layout_parent: Option<&LayoutNode>,
    ) -> Vec<Node> {
        let inner = view.storage.as_ref();
        if let Some(any) = inner.as_any().downcast_ref::<AnyView>() {
            return self.materialise(any, parent, layout_parent);
        }
        if let Some(source) = inner.as_any().downcast_ref::<SourceLocatedView>() {
            let nodes = self.materialise(&source.content, parent, layout_parent);
            for node in &nodes {
                if node.source_location().is_none() || source.location.explicit {
                    node.set_source_location(Some(source.location.clone()));
                }
            }
            return nodes;
        }
        // 1. Empty
        if inner.as_any().is::<EmptyView>() {
            return Vec::new();
        }
        // 2. Identified — unwrap, materialise inner, stamp the key so the next
        //    reconcile can reuse by `(tag, key)`.
        if let Some(identified) = inner.as_any().downcast_ref::<IdentifiedView>() {
            if is_direct_structural_content(identified.content.storage.as_ref()) {
                let anchor = Node::new();
                anchor.set_is_hit_testable(false);
                anchor.set_view_tag(Some(slot_tag(view)));
                anchor.set_key(Some(identified.id.clone()));
                parent.add_child(&anchor);
                let _ = self.materialise(&identified.content, &anchor, layout_parent);
                return vec![anchor];
            }
            let nodes = self.materialise(&identified.content, parent, layout_parent);
            for node in &nodes {
                node.set_key(Some(identified.id.clone()));
            }
            return nodes;
        }

        match inner.kind() {
            // 3. Primitive (text / box / row / ...)
            ViewKind::Primitive(primitive) => {
                self.materialise_primitive(primitive, inner.tag(), parent, layout_parent)
            }
            // 4. ModifiedContent — forward to the type-erased helper.
            ViewKind::Modified(modified) => modified.materialise_into(parent, layout_parent, self),
            // 5. Structural (tuple / conditional / optional / array)
            ViewKind::Structural(children) => children
                .iter()
                .flat_map(|child| self.materialise(child, parent, layout_parent))
                .collect(),
            // 6. User-defined View — install a scope.
            _ => self.materialise_user_view(view, parent, layout_parent),
        }
    }

    fn materialise_primitive(
        self: &Rc<Self>,
        primitive: &dyn PrimitiveView,
        tag: &'static str,
        parent: &Node,
        layout_parent: Option<&LayoutNode>,
    ) -> Vec<Node> {
        let node = primitive.make_node();
        node.set_view_tag(Some(tag.to_string()));
        // Build the layout node and wire both trees BEFORE `update_node` so
        // that `composition_value` walks a complete parent chain and
        // `node.layout_node` is non-nil.
        let layout = primitive.make_layout_node();
        if let Some(layout) = &layout {
            node.set_layout_node(Some(layout));
            self.layout_of
                .borrow_mut()
                .insert(node.id(), layout.clone());
            if let Some(parent) = layout_parent {
                parent.add_child(layout);
            }
        }
        parent.add_child(&node);
        ObservableStateTracking::observe_reads(|| primitive.update_node(&node));

        let mut child_layout_parent = layout_parent.cloned();
        if let Some(layout) = &layout {
            ObservableStateTracking::observe_reads(|| primitive.update_layout(layout));
            child_layout_parent = Some(layout.clone());
        }

        let children = ObservableStateTracking::observe_reads(|| primitive.children(&node));
        for child in children {
            let _ = self.materialise(&child, &node, child_layout_parent.as_ref());
        }
        vec![node]
    }

    fn materialise_user_view(
        self: &Rc<Self>,
        view: &AnyView,
        parent: &Node,
        layout_parent: Option<&LayoutNode>,
    ) -> Vec<Node> {
        let anchor = Node::new();
        // Anchors are pass-through: no hit testing and no LayoutNode.
        anchor.set_is_hit_testable(false);
        anchor.set_view_tag(Some(slot_tag(view)));
        parent.add_child(&anchor);
        let scope = Rc::new(RefCell::new(ViewScope::new(
            self,
            &anchor,
            view.clone(),
            layout_parent.cloned(),
            InvalidationLogHolder::current(),
        )));
        self.scopes.borrow_mut().insert(anchor.id(), scope.clone());
        ViewScope::install(&scope, self);
        vec![anchor]
    }

    // MARK: - Internal

    fn top_level_layouts(&self, node: &Node) -> Vec<LayoutNode> {
        if let Some(layout) = self.layout_of.borrow().get(&node.id()).cloned() {
            return vec![layout];
        }
        node.children()
            .iter()
            .flat_map(|child| self.top_level_layouts(child))
            .collect()
    }
}

fn is_empty_view(view: &dyn View) -> bool {
    matches!(view.kind(), ViewKind::Empty)
}

// MARK: - Slot classification

pub(crate) struct SlotInfo {
    pub view: AnyView,
    pub tag: String,
    pub key: Option<NodeKey>,
}

/// Tag stored in `Node.view_tag` for reuse decisions during recompose. Strips
/// `IdentifiedView` and `AnyView` wrappers so the tag describes the view kind.
pub(crate) fn slot_tag(view: &dyn View) -> String {
    let mut current: &dyn View = view;
    loop {
        if let Some(source) = current.as_any().downcast_ref::<SourceLocatedView>() {
            current = source.content.storage.as_ref();
            continue;
        }
        if let Some(identified) = current.as_any().downcast_ref::<IdentifiedView>() {
            current = identified.content.storage.as_ref();
            continue;
        }
        if let Some(any) = current.as_any().downcast_ref::<AnyView>() {
            current = any.storage.as_ref();
            continue;
        }
        break;
    }
    // Swift's `ModifiedContent<Content, Modifier>` is generic over both, so a
    // modified view is only the same slot as another with the same modifier
    // around the same content. One erased tag for every modified view let the
    // reconciler update a header in place with a row's content.
    if let Some(modified) = current.as_any().downcast_ref::<crate::ModifiedContent>() {
        return format!(
            "{}<{}>",
            modified.modifier_ref().modifier_name(),
            slot_tag(modified.content())
        );
    }
    current.tag().to_string()
}

/// Stable identity attached to a slot via `.id(_:)`. `None` for unkeyed slots;
/// the reconciler then matches by `(tag, sequential position)`.
pub(crate) fn slot_key(view: &dyn View) -> Option<NodeKey> {
    let mut current: &dyn View = view;
    let mut key: Option<NodeKey> = None;
    loop {
        if let Some(source) = current.as_any().downcast_ref::<SourceLocatedView>() {
            current = source.content.storage.as_ref();
            continue;
        }
        if let Some(identified) = current.as_any().downcast_ref::<IdentifiedView>() {
            // Outermost id wins — `foo.id(a).id(b)` resolves to `b`.
            if key.is_none() {
                key = Some(identified.id.clone());
            }
            current = identified.content.storage.as_ref();
            continue;
        }
        if let Some(any) = current.as_any().downcast_ref::<AnyView>() {
            current = any.storage.as_ref();
            continue;
        }
        break;
    }
    // Non-wrapping modifiers materialise onto the content's node. Match the
    // key they inherit there even when `.id` precedes `.frame`/`.flex`/etc.
    if key.is_none()
        && let ViewKind::Modified(modified) = current.kind()
    {
        key = modified.inherited_slot_key();
    }
    key
}

fn classify(view: &AnyView) -> SlotInfo {
    SlotInfo {
        view: view.clone(),
        tag: slot_tag(view),
        key: slot_key(view),
    }
}

/// An identified tuple/conditional needs one keyed anchor. Do not unwrap
/// another identity here: its existing anchor already owns that boundary.
fn is_direct_structural_content(view: &dyn View) -> bool {
    if let Some(source) = view.as_any().downcast_ref::<SourceLocatedView>() {
        return is_direct_structural_content(source.content.storage.as_ref());
    }
    if let Some(any) = view.as_any().downcast_ref::<AnyView>() {
        return is_direct_structural_content(any.storage.as_ref());
    }
    matches!(view.kind(), ViewKind::Structural(_))
}

/// Flatten structural views and strip `EmptyView` so the result contains
/// exactly one entry per Node slot the parent will hold. `IdentifiedView` and
/// `AnyView` wrappers are kept intact — the reconciler reads their identity at
/// match time via `classify`.
fn flatten_slots(views: &[AnyView]) -> Vec<AnyView> {
    let mut out = Vec::new();
    for view in views {
        let inner = view.storage.as_ref();
        if let Some(any) = inner.as_any().downcast_ref::<AnyView>() {
            out.extend(flatten_slots(std::slice::from_ref(any)));
            continue;
        }
        if let Some(source) = inner.as_any().downcast_ref::<SourceLocatedView>() {
            for entry in flatten_slots(std::slice::from_ref(&source.content)) {
                if entry
                    .as_any()
                    .downcast_ref::<SourceLocatedView>()
                    .is_some_and(|nested| !nested.location.explicit)
                {
                    out.push(entry);
                } else {
                    out.push(AnyView::new(SourceLocatedView {
                        content: entry,
                        location: source.location.clone(),
                    }));
                }
            }
            continue;
        }
        if inner.as_any().is::<EmptyView>() {
            continue;
        }
        match inner.kind() {
            ViewKind::Structural(children) => out.extend(flatten_slots(&children)),
            _ => out.push(view.clone()),
        }
    }
    out
}

/// FIFO queue for unkeyed slot reuse. Keeps a cursor instead of removing from
/// the front, which would make large repeated child lists O(n²) per reconcile.
#[derive(Default)]
struct UnkeyedSlotQueue {
    nodes: Vec<Node>,
    head: usize,
}
impl UnkeyedSlotQueue {
    fn append(&mut self, node: Node) {
        self.nodes.push(node);
    }
    fn pop_skipping(&mut self, claimed: &HashSet<ElementId>) -> Option<Node> {
        while self.head < self.nodes.len() {
            let candidate = self.nodes[self.head].clone();
            self.head += 1;
            if !claimed.contains(&candidate.id()) {
                return Some(candidate);
            }
        }
        None
    }
}

// MARK: - ViewScope

/// One user-view instantiation. Owns an anchor `Node`, the view value, and the
/// bookkeeping needed to recompose into the same anchor.
pub(crate) struct ViewScope {
    graph: Weak<ViewGraph>,
    anchor: Node,
    view: AnyView,
    layout_parent: Option<LayoutNode>,
    invalidation_log: Option<std::sync::Arc<InvalidationLog>>,
    installed: bool,
    fields: Vec<DynamicField>,
    state_registrations: Vec<String>,
    read_invalidation: Option<LocalInvalidationHandle>,
    cx: Scope,
    observer: Option<Rc<Observer>>,
}

impl ViewScope {
    fn new(
        graph: &Rc<ViewGraph>,
        anchor: &Node,
        view: AnyView,
        layout_parent: Option<LayoutNode>,
        invalidation_log: Option<std::sync::Arc<InvalidationLog>>,
    ) -> Self {
        Self {
            graph: Rc::downgrade(graph),
            anchor: anchor.clone(),
            view,
            layout_parent,
            invalidation_log,
            installed: false,
            fields: Vec::new(),
            state_registrations: Vec::new(),
            read_invalidation: None,
            cx: Scope::default(),
            observer: None,
        }
    }

    /// Wire state observers and materialise the body for the first time.
    fn install(scope: &Rc<RefCell<Self>>, graph: &Rc<ViewGraph>) {
        let weak_scope = Rc::downgrade(scope);
        let weak_graph = Rc::downgrade(graph);
        let observer = Observer::new(move || {
            let (Some(scope), Some(graph)) = (weak_scope.upgrade(), weak_graph.upgrade()) else {
                return;
            };
            Self::schedule(&scope, &graph, RecompositionReason::new("signal"));
        });
        {
            let mut scope = scope.borrow_mut();
            scope.installed = true;
            scope.observer = Some(Rc::new(observer));
        }
        Self::wire_dynamic_properties(scope, graph);
        Self::materialise_body(scope, graph);
    }

    /// Evaluate `run` under this scope's observer: the body and the
    /// primitives it materialises directly subscribe to the signals they read.
    fn observed<R>(scope: &Rc<RefCell<Self>>, run: impl FnOnce() -> R) -> R {
        let (observer, tracking) = {
            let scope = scope.borrow();
            (
                scope.observer.clone(),
                scope.read_invalidation.as_ref().map(|reads| {
                    (
                        cove_core::ReadScopeId(scope.anchor.id().0),
                        reads.invalidator(),
                        reads.tracked(),
                    )
                }),
            )
        };
        let evaluate = || match observer {
            Some(observer) => observer.track(run),
            None => run(),
        };
        match tracking {
            Some((id, invalidate, tracked)) => {
                ObservableStateTracking::with_scope(id, invalidate, Some(tracked), evaluate)
            }
            None => evaluate(),
        }
    }

    fn uninstall(scope: &Rc<RefCell<Self>>, graph: &ViewGraph) {
        let anchor_id = {
            let mut scope = scope.borrow_mut();
            scope.installed = false;
            for field in scope.fields.drain(..) {
                field.unwire();
            }
            let registrations = std::mem::take(&mut scope.state_registrations);
            for token in registrations {
                graph.state_registry.borrow_mut().unregister(&token);
            }
            scope.anchor.id()
        };
        {
            let mut scope = scope.borrow_mut();
            scope.read_invalidation.take();
            scope.observer.take();
        }
        ObservableStateTracking::remove_scope(cove_core::ReadScopeId(anchor_id.0));
    }

    /// Discover state members and route their writes into the recomposer.
    fn wire_dynamic_properties(scope: &Rc<RefCell<Self>>, graph: &Rc<ViewGraph>) {
        let (view, anchor_id) = {
            let scope = scope.borrow();
            (scope.view.clone(), scope.anchor.id())
        };
        let fields = view.dynamic_properties();
        let previous: Vec<DynamicField> = {
            let mut scope = scope.borrow_mut();
            std::mem::take(&mut scope.fields)
        };
        let previous_read_scope = scope.borrow_mut().read_invalidation.take();
        drop(previous_read_scope);
        for field in &previous {
            field.unwire();
        }
        let mut registrations = Vec::new();
        let precise = view.storage.precise_state_reads();
        let weak_scope = Rc::downgrade(scope);
        let weak_graph = Rc::downgrade(graph);
        let read_invalidation =
            LocalInvalidationHandle::with_id(cove_core::ReadScopeId(anchor_id.0), move |reason| {
                let (Some(scope), Some(graph)) = (weak_scope.upgrade(), weak_graph.upgrade())
                else {
                    return;
                };
                Self::schedule(&scope, &graph, reason);
            });
        let read_scope = read_invalidation.read_scope();
        for field in &fields {
            let weak_scope = Rc::downgrade(scope);
            let weak_graph = Rc::downgrade(graph);
            let name = field.name().to_string();
            let kind = field.diagnostic_kind().to_string();
            field.wire(
                Rc::new(move || {
                    let (Some(scope), Some(graph)) = (weak_scope.upgrade(), weak_graph.upgrade())
                    else {
                        return;
                    };
                    Self::schedule(
                        &scope,
                        &graph,
                        RecompositionReason::new(kind.clone()).with_detail(name.clone()),
                    );
                }),
                precise,
            );
            field.wire_read_tracking(read_scope.clone(), precise);
            if let Some(read) = field.dev_tools_read() {
                let stable = format!("{}:{}", anchor_id.0, field.name());
                let value_type = field.dev_tools_value_type();
                if let Some(token) = graph.state_registry.borrow_mut().register(
                    Some(stable),
                    field.name(),
                    &value_type,
                    Some(anchor_id.0.to_string()),
                    move || read(),
                ) {
                    registrations.push(token);
                }
            }
        }
        let mut scope = scope.borrow_mut();
        scope.fields = fields;
        scope.state_registrations = registrations;
        scope.read_invalidation = Some(read_invalidation);
    }

    /// Re-evaluate body and reconcile against the existing anchor children.
    fn recompose(
        scope: &Rc<RefCell<Self>>,
        graph: &Rc<ViewGraph>,
        reasons: Vec<RecompositionReason>,
    ) {
        let Some((anchor, layout_parent, tag)) = ({
            let scope = scope.borrow();
            if !scope.installed {
                None
            } else {
                Some((
                    scope.anchor.clone(),
                    scope.layout_parent.clone(),
                    scope
                        .anchor
                        .view_tag()
                        .unwrap_or_else(|| "Component".to_string()),
                ))
            }
        }) else {
            return;
        };
        let start = std::time::Instant::now();
        let previous = graph
            .active_recomposition_scope
            .replace(Some(anchor.id().0));
        let trace = graph.timeline_begin();
        Self::observed(scope, || {
            let body = Self::body(scope);
            graph.reconcile_children(&anchor, layout_parent.as_ref(), std::slice::from_ref(&body));
        });
        graph.active_recomposition_scope.set(previous);
        ModifierReplay::apply_from(&anchor, graph);
        Self::record_timing(scope, graph, start, false, &reasons);
        graph.timeline_end(
            trace,
            "component",
            &tag,
            Some(anchor.id().0.to_string()),
            &reasons,
        );
    }

    /// Swap in a new view value while keeping previously-wired state values
    /// intact. State members are matched by name.
    fn replace_view(scope: &Rc<RefCell<Self>>, graph: &Rc<ViewGraph>, new_view: AnyView) {
        let old_fields = scope.borrow().fields.clone();
        let new_fields = new_view.dynamic_properties();
        for field in &new_fields {
            if let Some(previous) = old_fields
                .iter()
                .find(|previous| previous.name() == field.name())
            {
                field.copy_value_from(previous);
            }
        }
        for field in &old_fields {
            field.unwire();
        }
        {
            let mut scope = scope.borrow_mut();
            scope.view = new_view;
            scope.fields.clear();
            scope.state_registrations.clear();
        }
        Self::wire_dynamic_properties(scope, graph);
    }

    fn materialise_body(scope: &Rc<RefCell<Self>>, graph: &Rc<ViewGraph>) {
        let (anchor, layout_parent) = {
            let scope = scope.borrow();
            (scope.anchor.clone(), scope.layout_parent.clone())
        };
        let start = std::time::Instant::now();
        let previous = graph
            .active_recomposition_scope
            .replace(Some(anchor.id().0));
        let trace = graph.timeline_begin();
        Self::observed(scope, || {
            let body = Self::body(scope);
            let _ = graph.materialise(&body, &anchor, layout_parent.as_ref());
        });
        graph.active_recomposition_scope.set(previous);
        Self::record_timing(scope, graph, start, true, &[]);
        let tag = anchor.view_tag().unwrap_or_else(|| "Component".to_string());
        graph.timeline_end(
            trace,
            "component",
            &tag,
            Some(anchor.id().0.to_string()),
            &[],
        );
    }

    fn record_timing(
        scope: &Rc<RefCell<Self>>,
        graph: &Rc<ViewGraph>,
        start: std::time::Instant,
        initial: bool,
        reasons: &[RecompositionReason],
    ) {
        if !graph.tracks_recomposition.get() {
            return;
        }
        let anchor = scope.borrow().anchor.clone();
        let mut metrics = anchor.recomposition_metrics().unwrap_or_default();
        let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
        metrics.record(milliseconds, initial, reasons);
        anchor.set_recomposition_metrics(Some(metrics));
    }

    fn body(scope: &Rc<RefCell<Self>>) -> AnyView {
        let (view, cx) = {
            let scope = scope.borrow();
            (scope.view.clone(), scope.cx.clone())
        };
        if Self::is_materialised_directly(view.storage.as_ref()) {
            view.clone()
        } else {
            cx.begin();
            view.body(&cx)
        }
    }

    /// Defensive guard for bodyless view forms. These normally get handled by
    /// `ViewGraph::materialise` before a user-view scope is created, but
    /// shutdown/reconcile paths can still hand a scoped slot a bodyless wrapper.
    fn is_materialised_directly(view: &dyn View) -> bool {
        matches!(
            view.kind(),
            ViewKind::Empty
                | ViewKind::Primitive(_)
                | ViewKind::Modified(_)
                | ViewKind::Structural(_)
                | ViewKind::Identified(_)
                | ViewKind::Source(_)
        )
    }

    fn schedule(scope: &Rc<RefCell<Self>>, graph: &Rc<ViewGraph>, reason: RecompositionReason) {
        if !scope.borrow().installed {
            return;
        }
        Self::record_state_write(scope);
        let context = ActiveAnimationContext::current();
        let (anchor_id, _) = {
            let scope = scope.borrow();
            (scope.anchor.id(), scope.graph.clone())
        };
        let weak_scope = Rc::downgrade(scope);
        let weak_graph = Rc::downgrade(graph);
        graph.recomposer.invalidate_tracked(
            anchor_id.0 as usize,
            context.animation,
            context.completion,
            context.disables_animations,
            reason,
            move |reasons| {
                let (Some(scope), Some(graph)) = (weak_scope.upgrade(), weak_graph.upgrade())
                else {
                    return;
                };
                Self::recompose(&scope, &graph, reasons.to_vec());
            },
        );
    }

    fn record_state_write(scope: &Rc<RefCell<Self>>) {
        let scope = scope.borrow();
        let Some(log) = &scope.invalidation_log else {
            return;
        };
        log.record(DirtyReason::new(
            scope.anchor.id(),
            InvalidationSource::StateWrite {
                scope: scope.anchor.id().0,
            },
            InvalidationPhase::Layout,
        ));
    }
}
