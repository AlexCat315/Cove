//! Translation of `Tests/Headless/Tests/CoveUISceneTests/FlatRenderPlanTests.swift`.
//!
//! The flat render plan keeps depth-first ranges in flat vectors so paint order,
//! cached-subtree skipping and teardown never recurse.
//!
//! Behaviour checklist:
//! - iterative resource teardown preserves child-before-parent and sibling order
//! - deep render plans install, paint, skip cached subtrees and tear down
//!   without recursive traversal
//! - a cached plan observes `z_index` changes, including returning to zero
//! - subtree ranges survive reconcile, reorder, teardown and root replacement
//! - linear enter/exit ordering preserves clips, fades, overlays, scales and
//!   cached siblings
//! - translated cached leaves remain clipped as they cross rounded corners

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_core::scene::{
    DrawHook, ElementId, InvalidationSource, LayerAwareNodeRenderer, Node, NodeRenderer,
    NodeResource, NodeTransform, NodeTree, PaintPlan, RenderTree,
};
use cove_core::{Color, DrawList, Point, Rect};

struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn locked<T>(body: impl FnOnce() -> T) -> T {
        thread_local! {
            static HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        if HELD.get() {
            return body();
        }
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                HELD.set(false);
            }
        }
        HELD.set(true);
        let _release = Release;
        body()
    }
}

/// Releases on unmount, the Swift `Resource`.
struct RecordingResource {
    unmounted: Rc<dyn Fn()>,
}

impl NodeResource for RecordingResource {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, _node: &Node) {
        (self.unmounted)();
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

fn framed(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

fn order(tree: &RenderTree) -> Vec<ElementId> {
    tree.flat_draw_order()
        .iter()
        .map(|object| object.element_id())
        .collect()
}

fn plan(tree: &RenderTree) -> PaintPlan {
    tree.paint_plan()
}

#[test]
fn resource_order() {
    GlobalTestLock::locked(|| {
        let root = Node::new();
        let group = Node::new();
        let first = Node::new();
        let last = Node::new();
        let grandchild = Node::new();
        root.add_child(&group);
        group.add_child(&first);
        group.add_child(&last);
        first.add_child(&grandchild);

        let released: Rc<RefCell<Vec<ElementId>>> = Rc::new(RefCell::new(Vec::new()));
        for node in [&group, &first, &last, &grandchild] {
            let id = node.id();
            let log = released.clone();
            node.add_resource(Rc::new(RecordingResource {
                unmounted: Rc::new(move || log.borrow_mut().push(id)),
            }));
        }

        root.remove_child(&group);
        assert_eq!(
            *released.borrow(),
            [grandchild.id(), first.id(), last.id(), group.id()],
            "children release before their parent, siblings in paint order",
        );
    });
}

#[test]
fn deep_tree() {
    GlobalTestLock::locked(|| {
        let leaf = framed(0.0, 0.0, 8.0, 8.0);
        leaf.set_background_color(Some(Color::WHITE));
        let mut nodes = vec![leaf.clone()];
        for _ in 0..2048 {
            let parent = framed(0.0, 0.0, 20.0, 20.0);
            parent.add_child(nodes.last().expect("the chain is non-empty"));
            nodes.push(parent);
        }
        let root = nodes.last().cloned().expect("the chain has a root");

        let tree = RenderTree::new();
        tree.install(&root);
        let count = nodes.len();
        assert!(
            plan(&tree).subtree_ends.iter().all(|end| *end == count),
            "every ancestor's range covers the whole chain",
        );

        let mut renderer = LayerAwareNodeRenderer::new();
        for pass in 0..3 {
            let mut list = DrawList::default();
            renderer.render(&tree, &mut list);
            assert_eq!(
                list.vertices().len(),
                4,
                "pass {pass} paints the leaf quad once"
            );
        }

        let mut scene = NodeTree::new();
        scene.set_root(Some(root.clone()));
        scene.flush();
        leaf.mark_dirty(InvalidationSource::Unknown);
        assert!(
            nodes
                .iter()
                .all(|node| node.is_dirty() && node.render_dirty()),
            "the dirty flag reaches every ancestor",
        );
        scene.flush();
        assert!(
            nodes
                .iter()
                .all(|node| !node.is_dirty() && !node.render_dirty()),
            "flush clears the whole chain iteratively",
        );

        tree.install(&root);
        assert_eq!(
            tree.flat_draw_order().len(),
            count,
            "the flat order covers every node"
        );
        tree.tear_down(&root);
        assert!(
            tree.flat_draw_order().is_empty(),
            "teardown empties the flat order"
        );
        assert_eq!(
            tree.object_count(),
            0,
            "teardown releases every render object"
        );

        // Release scene ownership from the top, keeping every node alive until
        // it is detached.
        for node in nodes.iter().rev() {
            for child in node.children() {
                node.remove_child(&child);
            }
        }
    });
}

#[test]
fn z_order_invalidation() {
    GlobalTestLock::locked(|| {
        let root = framed(0.0, 0.0, 200.0, 200.0);
        let a = framed(0.0, 0.0, 40.0, 40.0);
        a.set_background_color(Some(Color::WHITE));
        let b = framed(0.0, 0.0, 40.0, 40.0);
        b.set_background_color(Some(Color::BLACK));
        let grandchild = Node::new();
        root.add_child(&a);
        root.add_child(&b);
        a.add_child(&grandchild);

        let tree = RenderTree::new();
        tree.install(&root);
        let root_id = root.id();
        let a_id = a.id();
        let b_id = b.id();
        let grandchild_id = grandchild.id();

        assert_eq!(
            order(&tree),
            [root_id, a_id, grandchild_id, b_id],
            "default order is tree order"
        );

        a.set_z_index(1.0);
        assert_eq!(
            order(&tree),
            [root_id, b_id, a_id, grandchild_id],
            "a raised child paints last with its subtree"
        );

        a.set_z_index(0.0);
        assert_eq!(
            order(&tree),
            [root_id, a_id, grandchild_id, b_id],
            "returning to zero restores tree order"
        );

        for z in [1.0_f32, -1.0, 0.0] {
            a.set_z_index(z);
            let mut actual = DrawList::default();
            let mut expected = DrawList::default();
            LayerAwareNodeRenderer::new().render(&tree, &mut actual);
            NodeRenderer.render(&root, &mut expected);
            assert_eq!(
                actual
                    .vertices()
                    .iter()
                    .map(|v| v.color)
                    .collect::<Vec<_>>(),
                expected
                    .vertices()
                    .iter()
                    .map(|v| v.color)
                    .collect::<Vec<_>>(),
                "z_index {z} matches the uncached render",
            );
        }
    });
}

#[test]
fn structural_invalidation() {
    GlobalTestLock::locked(|| {
        let root = Node::new();
        let a = Node::new();
        let b = Node::new();
        let child = Node::new();
        root.add_child(&a);
        root.add_child(&b);
        a.add_child(&child);

        let tree = RenderTree::new();
        tree.install(&root);
        assert_eq!(
            plan(&tree).subtree_ends,
            [4, 3, 3, 4],
            "initial ranges are depth-first exclusive ends"
        );

        root.reorder_children(&[b.clone(), a.clone()]);
        tree.reconcile_children(&root);
        assert_eq!(
            order(&tree),
            [root.id(), b.id(), a.id(), child.id()],
            "the reorder is reflected"
        );
        assert_eq!(
            plan(&tree).subtree_ends,
            [4, 2, 4, 4],
            "ranges follow the new order"
        );

        root.remove_child(&a);
        tree.reconcile_children(&root);
        assert_eq!(
            plan(&tree).subtree_ends,
            [2, 2],
            "removal shrinks the ranges"
        );
        assert!(
            tree.render_object(&child).is_none(),
            "the orphaned subtree is dropped"
        );

        tree.tear_down(&b);
        assert_eq!(order(&tree), [root.id()], "teardown drops the branch");

        tree.install(&child);
        assert_eq!(
            order(&tree),
            [child.id()],
            "a replacement root installs cleanly"
        );
    });
}

#[test]
fn painter_parity() {
    GlobalTestLock::locked(|| {
        let root = framed(0.0, 0.0, 400.0, 400.0);
        let clipped = framed(20.0, 20.0, 100.0, 100.0);
        clipped.set_clips_to_bounds(true);
        clipped.set_corner_radius(16.0);
        clipped.set_opacity(0.7);
        clipped.set_subtree_opacity(0.6);
        clipped.set_background_color(Some(Color::WHITE));
        clipped.set_backdrop_blur(8.0);
        let leaf = framed(0.0, 0.0, 120.0, 120.0);
        leaf.set_draw(Some(Rc::new(|list: &mut DrawList, origin: Point| {
            let _ = list.add_rect(Rect::new(origin.x, origin.y, 120.0, 120.0), Color::BLACK);
        }) as DrawHook));
        clipped.set_overlay_draw(Some(Rc::new(|list: &mut DrawList, origin: Point| {
            let _ = list.add_rect(
                Rect::new(origin.x + 20.0, origin.y + 20.0, 10.0, 10.0),
                Color::WHITE,
            );
        })));
        let sibling = framed(200.0, 20.0, 40.0, 40.0);
        sibling.set_background_color(Some(Color::BLACK));
        root.add_child(&clipped);
        clipped.add_child(&leaf);
        root.add_child(&sibling);

        let tree = RenderTree::new();
        tree.install(&root);
        let mut renderer = LayerAwareNodeRenderer::new();
        for scale in [1.0_f32, 0.75, 1.0] {
            clipped.set_transform(NodeTransform::anchored(scale, 0.5, 0.5));
            for pass in 0..3 {
                let mut actual = DrawList::default();
                let mut expected = DrawList::default();
                renderer.render(&tree, &mut actual);
                NodeRenderer.render(&root, &mut expected);
                assert_eq!(
                    actual
                        .vertices()
                        .iter()
                        .map(|v| (v.x, v.y, v.u, v.v))
                        .collect::<Vec<_>>(),
                    expected
                        .vertices()
                        .iter()
                        .map(|v| (v.x, v.y, v.u, v.v))
                        .collect::<Vec<_>>(),
                    "scale {scale} pass {pass} agrees on geometry",
                );
                assert_eq!(
                    actual
                        .vertices()
                        .iter()
                        .map(|v| v.color)
                        .collect::<Vec<_>>(),
                    expected
                        .vertices()
                        .iter()
                        .map(|v| v.color)
                        .collect::<Vec<_>>(),
                    "scale {scale} pass {pass} agrees on colour",
                );
                assert_eq!(
                    actual.indices(),
                    expected.indices(),
                    "scale {scale} pass {pass} agrees on indices"
                );
                assert_eq!(
                    actual
                        .batches()
                        .iter()
                        .filter_map(|batch| batch.backdrop)
                        .collect::<Vec<_>>(),
                    expected
                        .batches()
                        .iter()
                        .filter_map(|batch| batch.backdrop)
                        .collect::<Vec<_>>(),
                    "scale {scale} pass {pass} agrees on backdrops",
                );
            }
        }
    });
}

#[test]
fn rounded_scroll_parity() {
    GlobalTestLock::locked(|| {
        let root = framed(0.0, 0.0, 100.0, 100.0);
        root.set_clips_to_bounds(true);
        root.set_corner_radius(24.0);
        let leaf = framed(2.0, 40.0, 20.0, 20.0);
        leaf.set_draw(Some(Rc::new(|list: &mut DrawList, origin: Point| {
            let _ = list.add_rect(Rect::new(origin.x, origin.y, 20.0, 20.0), Color::WHITE);
        }) as DrawHook));
        root.add_child(&leaf);

        let tree = RenderTree::new();
        tree.install(&root);
        let mut renderer = LayerAwareNodeRenderer::new();
        for offset in [0.0_f32, 4.0, 20.0, 32.0, 42.0, 100.0, 4.0] {
            root.set_content_offset(Point { x: 0.0, y: offset });
            for pass in 0..3 {
                let mut actual = DrawList::default();
                let mut expected = DrawList::default();
                renderer.render(&tree, &mut actual);
                NodeRenderer.render(&root, &mut expected);
                // Offscreen cached leaves may omit unreferenced vertices, so
                // compare the vertices the indices actually reference.
                let referenced = |list: &DrawList| -> Vec<(f32, f32, u32)> {
                    list.indices()
                        .iter()
                        .map(|index| {
                            let vertex = &list.vertices()[*index as usize];
                            (vertex.x, vertex.y, vertex.color)
                        })
                        .collect()
                };
                assert_eq!(
                    referenced(&actual),
                    referenced(&expected),
                    "offset {offset} pass {pass} stays clipped across the rounded corner",
                );
            }
        }
    });
}
