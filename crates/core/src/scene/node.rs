//! Translation of CoveUIScene/Node.swift.
//!
//! Swift's `final class Node` becomes a reference-counted handle over
//! `RefCell<NodeData>`; every `didSet` observer becomes the explicit setter
//! that performs the same invalidation.
use super::{
    AccessibilityRevision, AccessibilitySemantics, AccessibilityValueProvider,
    ComponentRecompositionMetrics, ComponentSourceLocation, CompositionLocal, CompositionLocalKey,
    DirtyReason, InvalidationLogHolder, InvalidationPhase, InvalidationSource, LayoutNode,
    LayoutNodeWeak, NodeResource, RenderObject, RenderObjectWeak,
};
use crate::{Color, DrawList, LinearGradient, Point, Rect, Reveal, SurfaceShadow};
use cove_platform::SystemCursor;
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

pub type NodeWeak = Weak<RefCell<NodeData>>;
pub type NodeCell = Rc<RefCell<NodeData>>;

/// Identity of authored render content, compared without knowing its type.
pub trait RenderIdentity: 'static {
    fn as_any(&self) -> &dyn Any;
    fn render_identity_eq(&self, other: &dyn Any) -> bool;
}
impl<T: PartialEq + 'static> RenderIdentity for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn render_identity_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|other| self == other)
    }
}

#[derive(Clone)]
pub struct AnyRenderContentIdentity(Rc<dyn RenderIdentity>);
impl AnyRenderContentIdentity {
    pub fn new<T: PartialEq + 'static>(value: T) -> Self {
        Self(Rc::new(value))
    }
}
impl PartialEq for AnyRenderContentIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.0.render_identity_eq(other.0.as_any())
    }
}

/// A type-erased, hashable identity supplied through `.id(_:)`.
pub trait NodeKeyValue: 'static {
    fn as_any(&self) -> &dyn Any;
    fn key_eq(&self, other: &dyn Any) -> bool;
    fn key_hash(&self) -> u64;
}
impl<T: Hash + Eq + 'static> NodeKeyValue for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn key_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|other| self == other)
    }
    fn key_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Clone)]
pub struct NodeKey(Rc<dyn NodeKeyValue>);
impl NodeKey {
    pub fn new<T: Hash + Eq + 'static>(value: T) -> Self {
        Self(Rc::new(value))
    }
}
impl PartialEq for NodeKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.key_hash() == other.0.key_hash() && self.0.key_eq(other.0.as_any())
    }
}
impl Eq for NodeKey {}
impl Hash for NodeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.key_hash().hash(state);
    }
}

pub type DrawHook = Rc<dyn Fn(&mut DrawList, Point)>;
pub type LayoutUpdateHook = Rc<dyn Fn(&Node)>;

pub struct NodeData {
    pub id: super::ElementId,
    pub source_location: Option<ComponentSourceLocation>,
    pub recomposition_metrics: Option<ComponentRecompositionMetrics>,
    children: Vec<Node>,
    children_may_need_z_sort: bool,
    cached_z_sorted_children: Option<Vec<Node>>,
    pub subtree_structure_version: u64,
    parent: NodeWeak,
    is_interaction_enabled: bool,
    is_layout_visible: bool,
    pub accessibility_semantics: Option<AccessibilitySemantics>,
    pub subtree_opacity: f32,
    pub is_dirty: bool,
    pub render_dirty: bool,
    pub frame: Rect,
    pub layout_node: Option<LayoutNodeWeak>,
    pub render_object: Option<RenderObjectWeak>,
    is_hit_testable: bool,
    allows_hit_testing: bool,
    cached_input_revision: u64,
    cached_accepts_subtree_input: bool,
    is_focusable: bool,
    pub automatically_focus_on_pointer_down: bool,
    clips_to_bounds: bool,
    pub is_scroll_viewport: bool,
    pub reveal_request: Option<Reveal>,
    authored_background_color: Option<Color>,
    pub debug_background_color: Option<Color>,
    pub background_gradient: Option<LinearGradient>,
    pub corner_radius: f32,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub backdrop_blur: f32,
    pub shadow: SurfaceShadow,
    pub ambient_shadow: SurfaceShadow,
    authored_foreground_color: Option<Color>,
    pub debug_foreground_color: Option<Color>,
    pub opacity: f32,
    pub z_index: f32,
    stored_draw: Option<DrawHook>,
    draw_identity: Option<AnyRenderContentIdentity>,
    stored_overlay_draw: Option<DrawHook>,
    overlay_draw_identity: Option<AnyRenderContentIdentity>,
    pub layout_did_update: Option<LayoutUpdateHook>,
    pub content_offset: Point,
    pub transform: super::NodeTransform,
    pub cursor: Option<SystemCursor>,
    pub view_tag: Option<String>,
    pub key: Option<NodeKey>,
    pub attachments: BTreeMap<String, Rc<dyn Any>>,
    pub composition_values: BTreeMap<CompositionLocalKey, Rc<dyn Any>>,
    resources: Vec<Rc<dyn NodeResource>>,
}

/// A retained-mode node in the CoveUI scene tree.
///
/// Nodes are reference types — identity is by allocation. Dirty flags propagate
/// upward so the root always knows when a flush is needed. The scene tree is
/// owned and mutated on the host run-loop thread only.
#[derive(Clone)]
pub struct Node(Rc<RefCell<NodeData>>);

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Node {}
impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}
impl std::fmt::Debug for Node {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Node({})", self.id())
    }
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-wide generation bumped whenever an input-eligibility bit changes.
static INPUT_REVISION: AtomicU64 = AtomicU64::new(0);

impl Node {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(NodeData {
            id: super::IdentityAllocator::shared().allocate(),
            source_location: None,
            recomposition_metrics: None,
            children: Vec::new(),
            children_may_need_z_sort: false,
            cached_z_sorted_children: None,
            subtree_structure_version: 0,
            parent: Weak::new(),
            is_interaction_enabled: true,
            is_layout_visible: true,
            accessibility_semantics: None,
            subtree_opacity: 1.0,
            is_dirty: false,
            render_dirty: false,
            frame: Rect::default(),
            layout_node: None,
            render_object: None,
            is_hit_testable: true,
            allows_hit_testing: true,
            cached_input_revision: u64::MAX,
            cached_accepts_subtree_input: false,
            is_focusable: false,
            automatically_focus_on_pointer_down: true,
            clips_to_bounds: false,
            is_scroll_viewport: false,
            reveal_request: None,
            authored_background_color: None,
            debug_background_color: None,
            background_gradient: None,
            corner_radius: 0.0,
            border_color: None,
            border_width: 0.0,
            backdrop_blur: 0.0,
            shadow: SurfaceShadow::default(),
            ambient_shadow: SurfaceShadow::default(),
            authored_foreground_color: None,
            debug_foreground_color: None,
            opacity: 1.0,
            z_index: 0.0,
            stored_draw: None,
            draw_identity: None,
            stored_overlay_draw: None,
            overlay_draw_identity: None,
            layout_did_update: None,
            content_offset: Point::default(),
            transform: super::NodeTransform::IDENTITY,
            cursor: None,
            view_tag: None,
            key: None,
            attachments: BTreeMap::new(),
            composition_values: BTreeMap::new(),
            resources: Vec::new(),
        })))
    }

    pub(crate) fn from_cell(cell: NodeCell) -> Self {
        Self(cell)
    }
    pub(crate) fn cell(&self) -> &NodeCell {
        &self.0
    }
    pub fn upgrade(weak: &NodeWeak) -> Option<Self> {
        weak.upgrade().map(Self)
    }
    pub fn downgrade(&self) -> NodeWeak {
        Rc::downgrade(&self.0)
    }

    // MARK: - Identity

    pub fn id(&self) -> super::ElementId {
        self.0.borrow().id
    }
    pub fn source_location(&self) -> Option<ComponentSourceLocation> {
        self.0.borrow().source_location.clone()
    }
    pub fn set_source_location(&self, location: Option<ComponentSourceLocation>) {
        self.0.borrow_mut().source_location = location;
    }
    pub fn recomposition_metrics(&self) -> Option<ComponentRecompositionMetrics> {
        self.0.borrow().recomposition_metrics.clone()
    }
    pub fn set_recomposition_metrics(&self, metrics: Option<ComponentRecompositionMetrics>) {
        self.0.borrow_mut().recomposition_metrics = metrics;
    }
    pub fn record_recomposition(
        &self,
        milliseconds: f64,
        initial: bool,
        reasons: &[super::RecompositionReason],
    ) {
        let mut data = self.0.borrow_mut();
        data.recomposition_metrics
            .get_or_insert_with(ComponentRecompositionMetrics::new)
            .record(milliseconds, initial, reasons);
    }

    // MARK: - Tree links

    /// Ordered children. Use `add_child` / `remove_child` to mutate.
    pub fn children(&self) -> Vec<Node> {
        self.0.borrow().children.clone()
    }
    pub fn child_count(&self) -> usize {
        self.0.borrow().children.len()
    }
    pub fn parent(&self) -> Option<Node> {
        self.0.borrow().parent.upgrade().map(Node::from_cell)
    }
    pub fn children_may_need_z_sort(&self) -> bool {
        self.0.borrow().children_may_need_z_sort
    }
    pub fn subtree_structure_version(&self) -> u64 {
        self.0.borrow().subtree_structure_version
    }

    /// Children ordered for paint/hit-test, sorted by `zIndex` (stable by
    /// sibling order) when any child uses a non-zero zIndex.
    pub fn render_ordered_children(&self) -> Vec<Node> {
        let (cached, needs_sort, children) = {
            let data = self.0.borrow();
            (
                data.cached_z_sorted_children.clone(),
                data.children_may_need_z_sort,
                data.children.clone(),
            )
        };
        if let Some(cached) = cached {
            return cached;
        }
        if !needs_sort {
            return children;
        }
        let mut indexed: Vec<(usize, Node)> = children.into_iter().enumerate().collect();
        indexed.sort_by(|(left_offset, left), (right_offset, right)| {
            let left_z = left.z_index();
            let right_z = right.z_index();
            if left_z == right_z {
                left_offset.cmp(right_offset)
            } else {
                left_z.total_cmp(&right_z)
            }
        });
        let sorted: Vec<Node> = indexed.into_iter().map(|(_, node)| node).collect();
        self.0.borrow_mut().cached_z_sorted_children = Some(sorted.clone());
        sorted
    }

    pub fn add_child(&self, child: &Node) {
        {
            let mut data = self.0.borrow_mut();
            child.0.borrow_mut().parent = Rc::downgrade(&self.0);
            data.children.push(child.clone());
            if child.0.borrow().z_index != 0.0 {
                data.children_may_need_z_sort = true;
            }
        }
        self.invalidate_z_order();
        self.bump_subtree_structure_version();
        Self::invalidate_input_cache();
        AccessibilityRevision::bump_structure();
        self.mark_render_dirty(InvalidationSource::StructuralChange);
    }

    pub fn remove_child(&self, child: &Node) {
        let (removed, removed_may_need_z_sort) = {
            let mut data = self.0.borrow_mut();
            let before = data.children.len();
            let removed_may_need_z_sort = child.0.borrow().z_index != 0.0;
            data.children
                .retain(|candidate| !Rc::ptr_eq(&candidate.0, &child.0));
            let removed = data.children.len() != before;
            if removed {
                child.0.borrow_mut().parent = Weak::new();
            }
            (removed, removed_may_need_z_sort)
        };
        if !removed {
            return;
        }
        child.release_from_tree();
        if removed_may_need_z_sort {
            self.recompute_children_may_need_z_sort();
        }
        self.bump_subtree_structure_version();
        self.invalidate_z_order();
        Self::invalidate_input_cache();
        AccessibilityRevision::bump_structure();
        self.mark_render_dirty(InvalidationSource::StructuralChange);
    }

    pub fn remove_from_parent(&self) {
        if let Some(parent) = self.parent() {
            parent.remove_child(self);
        }
    }

    /// Reassign this node's children to `ordered` without invoking teardown.
    pub fn reorder_children(&self, ordered: &[Node]) {
        let current = self.children();
        assert_eq!(
            ordered.len(),
            current.len(),
            "reorderChildren: count mismatch"
        );
        let current_ids: HashSet<&Node> = current.iter().collect();
        let next_ids: HashSet<&Node> = ordered.iter().collect();
        assert!(
            current_ids == next_ids,
            "reorderChildren: membership mismatch"
        );
        if current
            .iter()
            .zip(ordered)
            .all(|(left, right)| left == right)
        {
            return;
        }
        self.0.borrow_mut().children = ordered.to_vec();
        self.bump_subtree_structure_version();
        self.invalidate_z_order();
        AccessibilityRevision::bump_structure();
        self.mark_render_dirty(InvalidationSource::StructuralChange);
    }

    pub fn child_z_index_did_change(&self, old_value: f32, new_value: f32) {
        if old_value == 0.0 && new_value != 0.0 {
            self.0.borrow_mut().children_may_need_z_sort = true;
        } else if old_value != 0.0 && new_value == 0.0 {
            self.recompute_children_may_need_z_sort();
        }
        self.invalidate_z_order();
    }

    fn recompute_children_may_need_z_sort(&self) {
        let any = self.children().iter().any(|child| child.z_index() != 0.0);
        self.0.borrow_mut().children_may_need_z_sort = any;
    }

    /// Drop the memoised z-order on this node and its render mirror.
    pub(crate) fn invalidate_z_order(&self) {
        let render_object = {
            self.0.borrow_mut().cached_z_sorted_children = None;
            self.render_object()
        };
        if let Some(object) = render_object {
            object.invalidate_z_order();
        }
    }

    fn bump_subtree_structure_version(&self) {
        let mut cursor = Some(self.clone());
        while let Some(current) = cursor {
            {
                let mut data = current.0.borrow_mut();
                data.subtree_structure_version = data.subtree_structure_version.wrapping_add(1);
            }
            cursor = current.parent();
        }
    }

    // MARK: - Visibility and interaction

    pub fn is_interaction_enabled(&self) -> bool {
        self.0.borrow().is_interaction_enabled
    }
    pub fn set_is_interaction_enabled(&self, value: bool) {
        let changed = {
            let mut data = self.0.borrow_mut();
            let changed = data.is_interaction_enabled != value;
            data.is_interaction_enabled = value;
            changed
        };
        if changed {
            Self::invalidate_input_cache();
            AccessibilityRevision::bump_structure();
        }
    }
    pub fn is_layout_visible(&self) -> bool {
        self.0.borrow().is_layout_visible
    }
    /// Hiding drops the pointer capture and focus this subtree holds; showing
    /// does not restore them.
    pub fn set_layout_visibility(&self, visible: bool) {
        if visible == self.is_layout_visible() {
            return;
        }
        self.0.borrow_mut().is_layout_visible = visible;
        Self::invalidate_input_cache();
        AccessibilityRevision::bump_structure();
        self.mark_render_dirty(InvalidationSource::LayoutChange);
        if visible {
            return;
        }
        if let Some(target) = super::PointerCaptureHolder::current_target()
            && self.contains(&target)
        {
            super::PointerCaptureHolder::release_current();
        }
        if let Some(focused) = super::FocusChainHolder::current_focused()
            && self.contains(&focused)
        {
            super::FocusChainHolder::clear_current();
        }
    }
    fn contains(&self, target: &Node) -> bool {
        let mut cursor = Some(target.clone());
        while let Some(current) = cursor {
            if &current == self {
                return true;
            }
            cursor = current.parent();
        }
        false
    }

    pub fn accessibility(&self) -> Option<AccessibilitySemantics> {
        self.0.borrow().accessibility_semantics.clone()
    }
    pub fn set_accessibility(&self, semantics: Option<AccessibilitySemantics>) {
        let changed = {
            let mut data = self.0.borrow_mut();
            let changed = data.accessibility_semantics != semantics;
            data.accessibility_semantics = semantics;
            changed
        };
        if changed {
            AccessibilityRevision::bump_structure();
        }
    }
    pub fn accessibility_actions(&self) -> super::AccessibilityActions {
        self.attachment::<super::AccessibilityActions>("__accessibility.actions")
            .unwrap_or_default()
    }
    pub fn set_accessibility_actions(&self, actions: super::AccessibilityActions) {
        self.set_attachment("__accessibility.actions", actions);
        AccessibilityRevision::bump_structure();
    }
    pub fn accessibility_value_provider(&self) -> Option<AccessibilityValueProvider> {
        self.attachment::<AccessibilityValueProvider>("__accessibility.valueProvider")
    }
    pub fn set_accessibility_value_provider(&self, provider: Option<AccessibilityValueProvider>) {
        match provider {
            Some(provider) => self.set_attachment("__accessibility.valueProvider", provider),
            None => {
                self.0
                    .borrow_mut()
                    .attachments
                    .remove("__accessibility.valueProvider");
            }
        }
        AccessibilityRevision::bump_structure();
    }
    pub fn resolved_accessibility_value(&self) -> String {
        let value = self
            .accessibility_value_provider()
            .map(|provider| (provider.read)())
            .or_else(|| self.accessibility().map(|semantics| semantics.value))
            .unwrap_or_default();
        if self
            .accessibility()
            .is_some_and(|semantics| semantics.state.is_secure)
        {
            "•".repeat(value.chars().count())
        } else {
            value
        }
    }

    pub fn subtree_opacity(&self) -> f32 {
        self.0.borrow().subtree_opacity
    }
    pub fn set_subtree_opacity(&self, value: f32) {
        let old = {
            let mut data = self.0.borrow_mut();
            let old = data.subtree_opacity;
            if old == value {
                return;
            }
            data.subtree_opacity = value;
            old
        };
        if (old > 0.0) != (value > 0.0) {
            AccessibilityRevision::bump_structure();
        }
        self.mark_render_dirty(InvalidationSource::style_set("subtreeOpacity"));
    }

    // MARK: - Dirty state

    pub fn is_dirty(&self) -> bool {
        self.0.borrow().is_dirty
    }
    pub fn render_dirty(&self) -> bool {
        self.0.borrow().render_dirty
    }
    pub(crate) fn clear_dirty_flags(&self) {
        let mut data = self.0.borrow_mut();
        data.is_dirty = false;
        data.render_dirty = false;
    }

    // MARK: - Geometry

    pub fn frame(&self) -> Rect {
        self.0.borrow().frame
    }
    pub fn set_frame(&self, frame: Rect) {
        if self.0.borrow().frame == frame {
            return;
        }
        self.0.borrow_mut().frame = frame;
        self.invalidate_geometry(InvalidationSource::LayoutChange);
    }
    pub fn layout_node(&self) -> Option<LayoutNode> {
        self.0
            .borrow()
            .layout_node
            .as_ref()
            .and_then(Weak::upgrade)
            .map(LayoutNode::from_cell)
    }
    pub fn set_layout_node(&self, layout_node: Option<&LayoutNode>) {
        self.0.borrow_mut().layout_node = layout_node.map(LayoutNode::downgrade);
    }
    pub fn render_object(&self) -> Option<RenderObject> {
        self.0
            .borrow()
            .render_object
            .as_ref()
            .and_then(Weak::upgrade)
            .map(RenderObject::from_cell)
    }
    pub fn set_render_object(&self, render_object: Option<&RenderObject>) {
        self.0.borrow_mut().render_object = render_object.map(RenderObject::downgrade);
    }

    pub fn is_hit_testable(&self) -> bool {
        self.0.borrow().is_hit_testable
    }
    pub fn set_is_hit_testable(&self, value: bool) {
        if self.0.borrow().is_hit_testable == value {
            return;
        }
        self.0.borrow_mut().is_hit_testable = value;
        self.invalidate_geometry(InvalidationSource::style_set("isHitTestable"));
    }
    pub fn allows_hit_testing(&self) -> bool {
        self.0.borrow().allows_hit_testing
    }
    pub fn set_allows_hit_testing(&self, value: bool) {
        let changed = {
            let mut data = self.0.borrow_mut();
            let changed = data.allows_hit_testing != value;
            data.allows_hit_testing = value;
            changed
        };
        if changed {
            Self::invalidate_input_cache();
            AccessibilityRevision::bump_structure();
        }
    }
    pub fn accepts_subtree_input(&self) -> bool {
        let revision = INPUT_REVISION.load(Ordering::Relaxed);
        {
            let data = self.0.borrow();
            if data.cached_input_revision == revision {
                return data.cached_accepts_subtree_input;
            }
        }
        let value = self.is_layout_visible()
            && self.allows_hit_testing()
            && self.is_interaction_enabled()
            && self
                .parent()
                .is_none_or(|parent| parent.accepts_subtree_input());
        {
            let mut data = self.0.borrow_mut();
            data.cached_input_revision = revision;
            data.cached_accepts_subtree_input = value;
        }
        value
    }
    fn invalidate_input_cache() {
        INPUT_REVISION.fetch_add(1, Ordering::Relaxed);
        super::FocusRevision::bump();
    }
    pub fn is_focusable(&self) -> bool {
        self.0.borrow().is_focusable
    }
    pub fn set_is_focusable(&self, value: bool) {
        let changed = {
            let mut data = self.0.borrow_mut();
            let changed = data.is_focusable != value;
            data.is_focusable = value;
            changed
        };
        if changed {
            super::FocusRevision::bump();
            AccessibilityRevision::bump_structure();
        }
    }
    pub fn automatically_focus_on_pointer_down(&self) -> bool {
        self.0.borrow().automatically_focus_on_pointer_down
    }
    pub fn set_automatically_focus_on_pointer_down(&self, value: bool) {
        self.0.borrow_mut().automatically_focus_on_pointer_down = value;
    }
    pub fn clips_to_bounds(&self) -> bool {
        self.0.borrow().clips_to_bounds
    }
    pub fn set_clips_to_bounds(&self, value: bool) {
        if self.0.borrow().clips_to_bounds == value {
            return;
        }
        self.0.borrow_mut().clips_to_bounds = value;
        self.invalidate_geometry(InvalidationSource::style_set("clipsToBounds"));
    }
    pub fn is_scroll_viewport(&self) -> bool {
        self.0.borrow().is_scroll_viewport
    }
    pub fn set_is_scroll_viewport(&self, value: bool) {
        self.0.borrow_mut().is_scroll_viewport = value;
    }

    // MARK: - Visual

    pub fn reveal_request(&self) -> Option<Reveal> {
        self.0.borrow().reveal_request
    }
    /// Raise or clear this node's frame-level reveal.
    pub fn request_reveal(&self, reveal: Option<Reveal>) {
        if self.0.borrow().reveal_request == reveal {
            return;
        }
        self.0.borrow_mut().reveal_request = reveal;
        self.mark_render_dirty(InvalidationSource::style_set("revealRequest"));
    }
    pub fn background_color(&self) -> Option<Color> {
        let data = self.0.borrow();
        data.debug_background_color
            .or(data.authored_background_color)
    }
    pub fn set_background_color(&self, value: Option<Color>) {
        {
            let mut data = self.0.borrow_mut();
            if data.authored_background_color == value {
                return;
            }
            data.authored_background_color = value;
        }
        self.mark_render_dirty(InvalidationSource::style_set("backgroundColor"));
    }
    pub fn debug_background_color(&self) -> Option<Color> {
        self.0.borrow().debug_background_color
    }
    pub fn set_debug_background_color(&self, value: Option<Color>) {
        {
            let mut data = self.0.borrow_mut();
            if data.debug_background_color == value {
                return;
            }
            data.debug_background_color = value;
        }
        self.mark_render_dirty(InvalidationSource::style_set("debugBackgroundColor"));
    }
    pub fn background_gradient(&self) -> Option<LinearGradient> {
        self.0.borrow().background_gradient
    }
    pub fn set_background_gradient(&self, value: Option<LinearGradient>) {
        if self.0.borrow().background_gradient == value {
            return;
        }
        self.0.borrow_mut().background_gradient = value;
        self.mark_render_dirty(InvalidationSource::style_set("backgroundGradient"));
    }
    pub fn corner_radius(&self) -> f32 {
        self.0.borrow().corner_radius
    }
    pub fn set_corner_radius(&self, value: f32) {
        if self.0.borrow().corner_radius == value {
            return;
        }
        self.0.borrow_mut().corner_radius = value;
        self.mark_render_dirty(InvalidationSource::style_set("cornerRadius"));
    }
    pub fn border_color(&self) -> Option<Color> {
        self.0.borrow().border_color
    }
    pub fn set_border_color(&self, value: Option<Color>) {
        if self.0.borrow().border_color == value {
            return;
        }
        self.0.borrow_mut().border_color = value;
        self.mark_render_dirty(InvalidationSource::style_set("borderColor"));
    }
    pub fn border_width(&self) -> f32 {
        self.0.borrow().border_width
    }
    pub fn set_border_width(&self, value: f32) {
        if self.0.borrow().border_width == value {
            return;
        }
        self.0.borrow_mut().border_width = value;
        self.mark_render_dirty(InvalidationSource::style_set("borderWidth"));
    }
    pub fn backdrop_blur(&self) -> f32 {
        self.0.borrow().backdrop_blur
    }
    pub fn set_backdrop_blur(&self, value: f32) {
        if self.0.borrow().backdrop_blur == value {
            return;
        }
        self.0.borrow_mut().backdrop_blur = value;
        self.mark_render_dirty(InvalidationSource::style_set("backdropBlur"));
    }
    pub fn shadow(&self) -> SurfaceShadow {
        self.0.borrow().shadow
    }
    pub fn set_shadow(&self, value: SurfaceShadow) {
        if self.0.borrow().shadow == value {
            return;
        }
        self.0.borrow_mut().shadow = value;
        self.mark_render_dirty(InvalidationSource::style_set("shadow"));
    }
    pub fn ambient_shadow(&self) -> SurfaceShadow {
        self.0.borrow().ambient_shadow
    }
    pub fn set_ambient_shadow(&self, value: SurfaceShadow) {
        if self.0.borrow().ambient_shadow == value {
            return;
        }
        self.0.borrow_mut().ambient_shadow = value;
        self.mark_render_dirty(InvalidationSource::style_set("ambientShadow"));
    }
    pub fn foreground_color(&self) -> Option<Color> {
        let data = self.0.borrow();
        data.debug_foreground_color
            .or(data.authored_foreground_color)
    }
    pub fn set_foreground_color(&self, value: Option<Color>) {
        {
            let mut data = self.0.borrow_mut();
            if data.authored_foreground_color == value {
                return;
            }
            data.authored_foreground_color = value;
        }
        self.mark_render_dirty(InvalidationSource::style_set("foregroundColor"));
        self.invalidate_inherited_foreground();
    }
    pub fn debug_foreground_color(&self) -> Option<Color> {
        self.0.borrow().debug_foreground_color
    }
    pub fn set_debug_foreground_color(&self, value: Option<Color>) {
        {
            let mut data = self.0.borrow_mut();
            if data.debug_foreground_color == value {
                return;
            }
            data.debug_foreground_color = value;
        }
        self.mark_render_dirty(InvalidationSource::style_set("debugForegroundColor"));
        self.invalidate_inherited_foreground();
    }
    /// The nearest explicit tint, shared by text and monochrome icons.
    pub fn inherited_foreground_color(&self) -> Option<Color> {
        self.foreground_color().or_else(|| {
            self.parent()
                .and_then(|parent| parent.inherited_foreground_color())
        })
    }
    fn invalidate_inherited_foreground(&self) {
        for child in self.children() {
            if child.foreground_color().is_none() {
                child.mark_render_dirty(InvalidationSource::style_set("inheritedForegroundColor"));
                child.invalidate_inherited_foreground();
            }
        }
    }
    pub fn opacity(&self) -> f32 {
        self.0.borrow().opacity
    }
    pub fn set_opacity(&self, value: f32) {
        let old = {
            let mut data = self.0.borrow_mut();
            let old = data.opacity;
            if old == value {
                return;
            }
            data.opacity = value;
            old
        };
        if (old > 0.0) != (value > 0.0) {
            AccessibilityRevision::bump_structure();
        }
        self.mark_render_dirty(InvalidationSource::style_set("opacity"));
    }
    pub fn z_index(&self) -> f32 {
        self.0.borrow().z_index
    }
    pub fn set_z_index(&self, value: f32) {
        let old = {
            let mut data = self.0.borrow_mut();
            let old = data.z_index;
            if old == value {
                return;
            }
            data.z_index = value;
            old
        };
        if let Some(parent) = self.parent() {
            parent.child_z_index_did_change(old, value);
        }
        self.invalidate_geometry(InvalidationSource::style_set("zIndex"));
    }

    // MARK: - Draw hooks

    pub fn draw(&self) -> Option<DrawHook> {
        self.0.borrow().stored_draw.clone()
    }
    pub fn set_draw(&self, draw: Option<DrawHook>) {
        {
            let mut data = self.0.borrow_mut();
            data.stored_draw = draw;
            data.draw_identity = None;
        }
        self.mark_render_dirty(InvalidationSource::style_set("draw"));
    }
    pub fn overlay_draw(&self) -> Option<DrawHook> {
        self.0.borrow().stored_overlay_draw.clone()
    }
    pub fn set_overlay_draw(&self, draw: Option<DrawHook>) {
        {
            let mut data = self.0.borrow_mut();
            data.stored_overlay_draw = draw;
            data.overlay_draw_identity = None;
        }
        self.mark_render_dirty(InvalidationSource::style_set("overlayDraw"));
    }
    /// Refresh a custom painter, invalidating render caches only when its
    /// paint-affecting identity changed.
    pub fn update_draw(
        &self,
        identity: impl PartialEq + 'static,
        draw: impl Fn(&mut DrawList, Point) + 'static,
    ) {
        let next = AnyRenderContentIdentity::new(identity);
        let changed = {
            let mut data = self.0.borrow_mut();
            let changed = data.stored_draw.is_none() || data.draw_identity.as_ref() != Some(&next);
            data.stored_draw = Some(Rc::new(draw));
            data.draw_identity = Some(next);
            changed
        };
        if changed {
            self.mark_render_dirty(InvalidationSource::style_set("draw"));
        }
    }
    pub fn update_overlay_draw(
        &self,
        identity: impl PartialEq + 'static,
        draw: impl Fn(&mut DrawList, Point) + 'static,
    ) {
        let next = AnyRenderContentIdentity::new(identity);
        let changed = {
            let mut data = self.0.borrow_mut();
            let changed = data.stored_overlay_draw.is_none()
                || data.overlay_draw_identity.as_ref() != Some(&next);
            data.stored_overlay_draw = Some(Rc::new(draw));
            data.overlay_draw_identity = Some(next);
            changed
        };
        if changed {
            self.mark_render_dirty(InvalidationSource::style_set("overlayDraw"));
        }
    }
    pub fn layout_did_update(&self) -> Option<LayoutUpdateHook> {
        self.0.borrow().layout_did_update.clone()
    }
    pub fn set_layout_did_update(&self, hook: Option<LayoutUpdateHook>) {
        self.0.borrow_mut().layout_did_update = hook;
    }

    pub fn content_offset(&self) -> Point {
        self.0.borrow().content_offset
    }
    pub fn set_content_offset(&self, value: Point) {
        if self.0.borrow().content_offset == value {
            return;
        }
        self.0.borrow_mut().content_offset = value;
        self.invalidate_geometry(InvalidationSource::style_set("contentOffset"));
    }
    pub fn transform(&self) -> super::NodeTransform {
        self.0.borrow().transform
    }
    pub fn set_transform(&self, value: super::NodeTransform) {
        if self.0.borrow().transform == value {
            return;
        }
        self.0.borrow_mut().transform = value;
        self.invalidate_geometry(InvalidationSource::style_set("transform"));
    }
    pub fn cursor(&self) -> Option<SystemCursor> {
        self.0.borrow().cursor
    }
    pub fn set_cursor(&self, cursor: Option<SystemCursor>) {
        self.0.borrow_mut().cursor = cursor;
    }

    // MARK: - Compose reconciliation

    pub fn view_tag(&self) -> Option<String> {
        self.0.borrow().view_tag.clone()
    }
    pub fn set_view_tag(&self, tag: Option<String>) {
        self.0.borrow_mut().view_tag = tag;
    }
    pub fn key(&self) -> Option<NodeKey> {
        self.0.borrow().key.clone()
    }
    pub fn set_key(&self, key: Option<NodeKey>) {
        self.0.borrow_mut().key = key;
    }
    /// Side table for primitive-owned persistent state that must survive a recompose.
    pub fn attachment<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.0
            .borrow()
            .attachments
            .get(key)
            .and_then(|value| value.downcast_ref::<T>().cloned())
    }
    pub fn set_attachment<T: 'static>(&self, key: &str, value: T) {
        self.0
            .borrow_mut()
            .attachments
            .insert(key.to_owned(), Rc::new(value));
    }
    pub fn remove_attachment(&self, key: &str) {
        self.0.borrow_mut().attachments.remove(key);
    }

    // MARK: - CompositionLocal storage

    pub fn set_composition_value<Value: 'static>(
        &self,
        local: &CompositionLocal<Value>,
        value: Value,
    ) {
        self.0
            .borrow_mut()
            .composition_values
            .insert(local.key(), Rc::new(value));
        self.mark_render_dirty(InvalidationSource::style_set("compositionValue"));
        self.invalidate_composition_paint(local.key());
    }
    fn invalidate_composition_paint(&self, key: CompositionLocalKey) {
        if let Some(object) = self.render_object() {
            object.set_cache_invalid(true);
        }
        for child in self.children() {
            if !child.0.borrow().composition_values.contains_key(&key) {
                child.invalidate_composition_paint(key);
            }
        }
    }
    /// Resolve `local` by walking up the parent chain.
    pub fn composition_value<Value: Clone + 'static>(
        &self,
        local: &CompositionLocal<Value>,
    ) -> Value {
        let mut cursor = Some(self.clone());
        while let Some(node) = cursor {
            if let Some(value) = node
                .0
                .borrow()
                .composition_values
                .get(&local.key())
                .and_then(|raw| raw.downcast_ref::<Value>())
            {
                return value.clone();
            }
            cursor = node.parent();
        }
        local.default_value()
    }
    /// Value provided for `local` directly on this node, without walking
    /// ancestors. Used by surface-theme probing and diagnostics.
    pub fn composition_value_here<Value: Clone + 'static>(
        &self,
        local: &CompositionLocal<Value>,
    ) -> Option<Value> {
        self.0
            .borrow()
            .composition_values
            .get(&local.key())
            .and_then(|raw| raw.downcast_ref::<Value>())
            .cloned()
    }

    // MARK: - Resources

    pub fn add_resource(&self, resource: Rc<dyn NodeResource>) {
        self.0.borrow_mut().resources.push(resource.clone());
        resource.mount(self);
    }
    pub fn first_resource<R: NodeResource + 'static>(&self) -> Option<Rc<R>> {
        self.0
            .borrow()
            .resources
            .iter()
            .find_map(|resource| resource.clone().as_any_rc().downcast::<R>().ok())
    }
    fn release_from_tree(&self) {
        let mut pending = vec![(self.clone(), false)];
        while let Some((node, exiting)) = pending.pop() {
            if exiting {
                node.release_own_resources();
            } else {
                pending.push((node.clone(), true));
                for child in node.children().into_iter().rev() {
                    pending.push((child, false));
                }
            }
        }
    }
    fn release_own_resources(&self) {
        let resources = std::mem::take(&mut self.0.borrow_mut().resources);
        for resource in resources {
            resource.unmount(self);
        }
        if super::PointerCaptureHolder::current_target().is_some_and(|target| &target == self) {
            super::PointerCaptureHolder::release_current();
        }
        if super::FocusChainHolder::current_focused().is_some_and(|focused| &focused == self) {
            super::FocusChainHolder::clear_current();
        }
    }

    // MARK: - Geometry single funnel

    /// The one and only path a geometry / hit-classification mutation takes to
    /// notify caches.
    fn invalidate_geometry(&self, reason: InvalidationSource) {
        AccessibilityRevision::bump_geometry();
        self.mark_render_dirty(reason);
    }

    // MARK: - Dirty propagation

    /// Mark this node dirty and propagate the flag upward to every ancestor.
    pub fn mark_dirty(&self, reason: InvalidationSource) {
        if let Some(log) = InvalidationLogHolder::current() {
            log.record(DirtyReason::new(
                self.id(),
                reason.clone(),
                InvalidationPhase::Layout,
            ));
            log.record(DirtyReason::new(
                self.id(),
                reason.clone(),
                InvalidationPhase::Render,
            ));
        }
        self.propagate_dirty(true, true);
        if let Some(object) = self.render_object() {
            object.invalidate_layer_chain();
        }
    }
    pub fn mark_render_dirty(&self, reason: InvalidationSource) {
        let was_render_dirty = self.render_dirty();
        if let Some(log) = InvalidationLogHolder::current() {
            log.record(DirtyReason::new(
                self.id(),
                reason.clone(),
                InvalidationPhase::Render,
            ));
        }
        self.propagate_dirty(false, true);
        let object = self.render_object();
        if was_render_dirty {
            if let Some(object) = &object {
                if is_layer_classification_invalidation(&reason) {
                    object.refresh_layer_classification();
                }
                object.invalidate_layer_chain();
            }
            return;
        }
        if let Some(object) = &object {
            object.refresh_layer_classification();
            object.invalidate_layer_chain();
        }
    }
    fn propagate_dirty(&self, layout_dirty: bool, render_dirty: bool) {
        let mut cursor = Some(self.clone());
        while let Some(current) = cursor {
            let should_propagate = {
                let data = current.0.borrow();
                (layout_dirty && !data.is_dirty) || (render_dirty && !data.render_dirty)
            };
            {
                let mut data = current.0.borrow_mut();
                if layout_dirty {
                    data.is_dirty = true;
                }
                if render_dirty {
                    data.render_dirty = true;
                }
            }
            if !should_propagate {
                return;
            }
            cursor = current.parent();
        }
    }
}

fn is_layer_classification_invalidation(reason: &InvalidationSource) -> bool {
    let InvalidationSource::StyleSet { field } = reason else {
        return false;
    };
    matches!(
        field.as_str(),
        "clipsToBounds" | "opacity" | "shadow" | "ambientShadow" | "draw" | "overlayDraw"
    )
}
