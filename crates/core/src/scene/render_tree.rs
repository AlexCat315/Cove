//! Translation of CoveUIScene/RenderTree.swift.
use super::{ElementId, Node, NodeWeak};
use crate::{Clip, DrawList, Rect};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub type RenderObjectWeak = Weak<RefCell<RenderObjectData>>;

pub struct RenderObjectData {
    pub node: NodeWeak,
    pub parent: RenderObjectWeak,
    tree: Weak<RefCell<RenderTreeData>>,
    pub children: Vec<RenderObject>,
    /// Stable identity inherited from the paired `Node`.
    pub element_id: ElementId,
    /// True when this object begins a composition group that records into its
    /// own `DrawList`.
    pub is_layer_root: bool,
    /// Placement snapshot from the last recording.
    pub last_placement: Option<super::Affine2D>,
    pub cache_invalid: bool,
    pub cached_layer_list: Option<DrawList>,
    pub cached_paint_bounds: Option<Rect>,
    pub cached_layer_uses_local_clips: bool,
    pub last_clip_stack: Vec<Clip>,
    cached_z_sorted_children: Option<Vec<RenderObject>>,
}

/// Retained geometry and layer classification for one scene node.
#[derive(Clone)]
pub struct RenderObject(Rc<RefCell<RenderObjectData>>);
impl PartialEq for RenderObject {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RenderObject {}
impl std::fmt::Debug for RenderObject {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "RenderObject({})", self.element_id())
    }
}
impl RenderObject {
    fn new(node: &Node, tree: Weak<RefCell<RenderTreeData>>) -> Self {
        let object = Self(Rc::new(RefCell::new(RenderObjectData {
            node: node.downgrade(),
            parent: Weak::new(),
            tree,
            children: Vec::new(),
            element_id: node.id(),
            is_layer_root: true,
            last_placement: None,
            cache_invalid: true,
            cached_layer_list: None,
            cached_paint_bounds: None,
            cached_layer_uses_local_clips: false,
            last_clip_stack: Vec::new(),
            cached_z_sorted_children: None,
        })));
        node.set_render_object(Some(&object));
        object.refresh_layer_classification();
        object
    }
    pub(crate) fn from_cell(cell: Rc<RefCell<RenderObjectData>>) -> Self {
        Self(cell)
    }
    pub(crate) fn downgrade(&self) -> RenderObjectWeak {
        Rc::downgrade(&self.0)
    }
    pub fn element_id(&self) -> ElementId {
        self.0.borrow().element_id
    }
    pub fn node(&self) -> Option<Node> {
        self.0.borrow().node.upgrade().map(Node::from_cell)
    }
    pub fn parent(&self) -> Option<RenderObject> {
        self.0.borrow().parent.upgrade().map(Self::from_cell)
    }
    pub fn children(&self) -> Vec<RenderObject> {
        self.0.borrow().children.clone()
    }
    /// Whether the object has children, without cloning them.
    pub fn has_children(&self) -> bool {
        !self.0.borrow().children.is_empty()
    }
    pub fn is_layer_root(&self) -> bool {
        self.0.borrow().is_layer_root
    }
    pub fn cache_invalid(&self) -> bool {
        self.0.borrow().cache_invalid
    }
    pub fn set_cache_invalid(&self, invalid: bool) {
        self.0.borrow_mut().cache_invalid = invalid;
    }
    pub fn cached_layer_list(&self) -> Option<DrawList> {
        self.0.borrow().cached_layer_list.clone()
    }
    pub fn cached_batch_count(&self) -> Option<usize> {
        self.0
            .borrow()
            .cached_layer_list
            .as_ref()
            .map(|list| list.batches().len())
    }
    pub fn set_cached_layer_list(&self, list: Option<DrawList>) {
        self.0.borrow_mut().cached_layer_list = list;
    }
    pub fn cached_paint_bounds(&self) -> Option<Rect> {
        self.0.borrow().cached_paint_bounds
    }
    pub fn set_cached_paint_bounds(&self, bounds: Option<Rect>) {
        self.0.borrow_mut().cached_paint_bounds = bounds;
    }
    pub fn last_placement(&self) -> Option<super::Affine2D> {
        self.0.borrow().last_placement
    }
    pub fn set_last_placement(&self, placement: Option<super::Affine2D>) {
        self.0.borrow_mut().last_placement = placement;
    }
    pub fn last_clip_stack(&self) -> Vec<Clip> {
        self.0.borrow().last_clip_stack.clone()
    }
    pub fn last_clip_count(&self) -> usize {
        self.0.borrow().last_clip_stack.len()
    }
    /// Compares the recorded clip stack without cloning it.
    pub fn last_clip_stack_eq(&self, clips: &[Clip]) -> bool {
        self.0.borrow().last_clip_stack == clips
    }
    pub fn set_last_clip_stack(&self, clips: Vec<Clip>) {
        self.0.borrow_mut().last_clip_stack = clips;
    }
    pub fn cached_layer_uses_local_clips(&self) -> bool {
        self.0.borrow().cached_layer_uses_local_clips
    }
    pub fn set_cached_layer_uses_local_clips(&self, value: bool) {
        self.0.borrow_mut().cached_layer_uses_local_clips = value;
    }
    pub(crate) fn set_parent(&self, parent: Option<&RenderObject>) {
        self.0.borrow_mut().parent = parent.map(Self::downgrade).unwrap_or_default();
    }
    pub(crate) fn set_children(&self, children: Vec<RenderObject>) {
        self.0.borrow_mut().children = children;
    }
    /// Drop the memoised z-order.
    pub fn invalidate_z_order(&self) {
        let tree = {
            self.0.borrow_mut().cached_z_sorted_children = None;
            self.0.borrow().tree.upgrade()
        };
        if let Some(tree) = tree {
            tree.borrow_mut().flat_order_cache = None;
        }
    }
    /// Children ordered for paint/hit-test, sorted by `zIndex` (stable by
    /// sibling order) when any child uses a non-zero zIndex.
    pub fn render_ordered_children(&self) -> Vec<RenderObject> {
        if let Some(cached) = self.0.borrow().cached_z_sorted_children.clone() {
            return cached;
        }
        let children = self.children();
        let mut indexed: Vec<(usize, RenderObject)> = children.into_iter().enumerate().collect();
        indexed.sort_by(|(left_offset, left), (right_offset, right)| {
            let left_z = left.node().map_or(0.0, |node| node.z_index());
            let right_z = right.node().map_or(0.0, |node| node.z_index());
            if left_z == right_z {
                left_offset.cmp(right_offset)
            } else {
                left_z.total_cmp(&right_z)
            }
        });
        let sorted: Vec<RenderObject> = indexed.into_iter().map(|(_, object)| object).collect();
        self.0.borrow_mut().cached_z_sorted_children = Some(sorted.clone());
        sorted
    }
    /// Walk parents and mark every layer-root cache on the path invalid.
    pub fn invalidate_layer_chain(&self) {
        let mut cursor = Some(self.clone());
        while let Some(current) = cursor {
            if current.is_layer_root() {
                current.set_cache_invalid(true);
            }
            cursor = current.parent();
        }
    }
    /// Re-evaluate `is_layer_root` from the bound node's current properties.
    pub fn refresh_layer_classification(&self) {
        let Some(node) = self.node() else {
            self.0.borrow_mut().is_layer_root = true;
            return;
        };
        let layer_root = self.parent().is_none()
            || node.clips_to_bounds()
            || node.opacity() < 1.0
            || !node.shadow().is_empty()
            || !node.ambient_shadow().is_empty()
            || node.draw().is_some()
            || node.overlay_draw().is_some();
        self.0.borrow_mut().is_layer_root = layer_root;
    }
}

/// Retained render records and a memoised linear paint plan.
#[derive(Clone, Debug, Default)]
pub struct PaintPlan {
    pub objects: Vec<RenderObject>,
    pub parents: Vec<isize>,
    /// Exclusive subtree bounds let cached or culled branches skip in O(1).
    pub subtree_ends: Vec<usize>,
}

#[derive(Default)]
pub(crate) struct RenderTreeData {
    pub root: Option<RenderObject>,
    by_node: HashMap<Node, RenderObject>,
    /// Shared: every render of an unchanged tree reuses the same plan
    /// without copying it.
    pub flat_order_cache: Option<Rc<PaintPlan>>,
}

#[derive(Clone)]
pub struct RenderTree {
    data: Rc<RefCell<RenderTreeData>>,
}
impl Default for RenderTree {
    fn default() -> Self {
        Self::new()
    }
}
impl RenderTree {
    pub fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(RenderTreeData::default())),
        }
    }
    fn cell(&self) -> Weak<RefCell<RenderTreeData>> {
        Rc::downgrade(&self.data)
    }
    pub fn root(&self) -> Option<RenderObject> {
        self.data.borrow().root.clone()
    }
    pub fn render_object(&self, node: &Node) -> Option<RenderObject> {
        self.data.borrow().by_node.get(node).cloned()
    }
    pub fn object_count(&self) -> usize {
        self.data.borrow().by_node.len()
    }
    /// Replace the root and rebuild the entire mirror from `node`.
    pub fn install(&self, root_node: &Node) {
        if let Some(root) = self.root() {
            self.tear_down_object(&root);
        }
        {
            let mut data = self.data.borrow_mut();
            data.by_node.clear();
            data.flat_order_cache = None;
        }
        let object = RenderObject::new(root_node, self.cell());
        {
            let mut data = self.data.borrow_mut();
            data.by_node.insert(root_node.clone(), object.clone());
            data.root = Some(object.clone());
        }
        self.rebuild_subtree(&object);
    }
    /// Re-mirror the children of a node after a structural reconcile.
    pub fn reconcile_children(&self, parent_node: &Node) {
        let Some(parent_object) = self.render_object(parent_node) else {
            return;
        };
        // Children are keyed by the stable `ElementId` their node carries, so
        // the map key has no interior mutability and cannot be invalidated by
        // a later node mutation.
        let existing: HashMap<ElementId, RenderObject> = parent_object
            .children()
            .into_iter()
            .filter_map(|child| child.node().map(|node| (node.id(), child)))
            .collect();
        let mut rebuilt = Vec::new();
        let mut seen = Vec::new();
        for child_node in parent_node.children() {
            seen.push(child_node.id());
            if let Some(kept) = existing.get(&child_node.id()) {
                kept.set_parent(Some(&parent_object));
                rebuilt.push(kept.clone());
            } else {
                let made = RenderObject::new(&child_node, self.cell());
                made.set_parent(Some(&parent_object));
                self.data
                    .borrow_mut()
                    .by_node
                    .insert(child_node.clone(), made.clone());
                rebuilt.push(made.clone());
                self.rebuild_subtree(&made);
            }
        }
        for (id, object) in existing {
            if !seen.contains(&id) {
                self.tear_down_object(&object);
            }
        }
        parent_object.set_children(rebuilt.clone());
        parent_object.invalidate_z_order();
        self.data.borrow_mut().flat_order_cache = None;
        for object in &rebuilt {
            object.refresh_layer_classification();
            object.set_cache_invalid(true);
        }
        parent_object.set_cache_invalid(true);
    }
    /// Drop a single node's render object (and its descendants).
    pub fn tear_down(&self, node: &Node) {
        let Some(object) = self.render_object(node) else {
            return;
        };
        if let Some(parent) = object.parent() {
            let mut children = parent.children();
            children.retain(|child| child != &object);
            parent.set_children(children);
            parent.set_cache_invalid(true);
            parent.invalidate_z_order();
        }
        self.tear_down_object(&object);
        let mut data = self.data.borrow_mut();
        if data.root.as_ref() == Some(&object) {
            data.root = None;
        }
    }
    fn tear_down_object(&self, object: &RenderObject) {
        self.data.borrow_mut().flat_order_cache = None;
        let mut pending = vec![object.clone()];
        while let Some(current) = pending.pop() {
            pending.extend(current.children());
            current.set_children(Vec::new());
            if let Some(node) = current.node() {
                self.data.borrow_mut().by_node.remove(&node);
                if node.render_object().as_ref() == Some(&current) {
                    node.set_render_object(None);
                }
            }
        }
    }
    fn rebuild_subtree(&self, parent: &RenderObject) {
        let mut pending = vec![parent.clone()];
        while let Some(object) = pending.pop() {
            let Some(node) = object.node() else { continue };
            let children: Vec<RenderObject> = node
                .children()
                .into_iter()
                .map(|child| {
                    let made = RenderObject::new(&child, self.cell());
                    made.set_parent(Some(&object));
                    self.data.borrow_mut().by_node.insert(child, made.clone());
                    made
                })
                .collect();
            object.set_children(children.clone());
            object.refresh_layer_classification();
            pending.extend(children.into_iter().rev());
        }
    }
    /// Discard derived geometry without destroying live scene/state ownership.
    pub fn trim_caches(&self) {
        let objects: Vec<RenderObject> = self.data.borrow().by_node.values().cloned().collect();
        for object in objects {
            object.set_cached_layer_list(None);
            object.set_cached_paint_bounds(None);
            object.set_cache_invalid(true);
        }
    }
    /// Snapshot of every layer-root render object in pre-order.
    pub fn layer_roots(&self) -> Vec<RenderObject> {
        let Some(root) = self.root() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut pending = vec![root];
        while let Some(object) = pending.pop() {
            if object.is_layer_root() {
                out.push(object.clone());
            }
            pending.extend(object.children().into_iter().rev());
        }
        out
    }
    /// Flat, depth-first, parent-before-child list in final paint order.
    pub fn flat_draw_order(&self) -> Vec<RenderObject> {
        self.paint_plan().objects.clone()
    }
    pub fn paint_plan(&self) -> Rc<PaintPlan> {
        if let Some(cached) = self.data.borrow().flat_order_cache.clone() {
            return cached;
        }
        let mut plan = PaintPlan::default();
        let Some(root) = self.root() else {
            let plan = Rc::new(plan);
            self.data.borrow_mut().flat_order_cache = Some(plan.clone());
            return plan;
        };
        let capacity = self.data.borrow().by_node.len();
        plan.objects.reserve(capacity);
        plan.parents.reserve(capacity);
        plan.subtree_ends.reserve(capacity);
        let mut pending: Vec<(RenderObject, isize, Option<usize>)> = vec![(root, -1, None)];
        while let Some((object, parent, exit)) = pending.pop() {
            if let Some(index) = exit {
                plan.subtree_ends[index] = plan.objects.len();
                continue;
            }
            let index = plan.objects.len();
            plan.objects.push(object.clone());
            plan.parents.push(parent);
            plan.subtree_ends.push(index + 1);
            let children = if object
                .node()
                .is_some_and(|node| node.children_may_need_z_sort())
            {
                object.render_ordered_children()
            } else {
                object.children()
            };
            if !children.is_empty() {
                pending.push((object, parent, Some(index)));
                for child in children.into_iter().rev() {
                    pending.push((child, index as isize, None));
                }
            }
        }
        let plan = Rc::new(plan);
        self.data.borrow_mut().flat_order_cache = Some(plan.clone());
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_mirrors_the_tree_and_classifies_layers() {
        let root = Node::new();
        let child = Node::new();
        root.add_child(&child);
        let tree = RenderTree::new();
        tree.install(&root);
        assert_eq!(tree.object_count(), 2);
        assert!(tree.render_object(&root).unwrap().is_layer_root());
        assert!(!tree.render_object(&child).unwrap().is_layer_root());
        child.set_clips_to_bounds(true);
        tree.render_object(&child)
            .unwrap()
            .refresh_layer_classification();
        assert!(tree.render_object(&child).unwrap().is_layer_root());
    }

    #[test]
    fn reconcile_keeps_existing_objects_and_drops_removed_ones() {
        let root = Node::new();
        let first = Node::new();
        let second = Node::new();
        root.add_child(&first);
        root.add_child(&second);
        let tree = RenderTree::new();
        tree.install(&root);
        let kept = tree.render_object(&first).unwrap();
        root.remove_child(&second);
        tree.reconcile_children(&root);
        assert_eq!(tree.object_count(), 2);
        assert_eq!(tree.render_object(&first), Some(kept));
        assert!(tree.render_object(&second).is_none());
        assert_eq!(tree.flat_draw_order().len(), 2);
    }

    #[test]
    fn paint_plan_respects_z_index() {
        let root = Node::new();
        let low = Node::new();
        low.set_z_index(1.0);
        let high = Node::new();
        high.set_z_index(5.0);
        root.add_child(&low);
        root.add_child(&high);
        let tree = RenderTree::new();
        tree.install(&root);
        let order = tree.flat_draw_order();
        assert_eq!(order[1].element_id(), low.id());
        assert_eq!(order[2].element_id(), high.id());
    }
}
