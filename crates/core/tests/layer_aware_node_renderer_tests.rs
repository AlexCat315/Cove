//! Translation of `Tests/CoveUIRuntimeTests/LayerAwareNodeRendererTests.swift`.
//!
//! `LayerAwareNodeRenderer` caches a subtree's draw list per layer root. These
//! pin that the cache cannot change what is drawn: output must match the
//! uncached `NodeRenderer` for geometry, colours, clips and pixel density, and a
//! cache entry that is still valid must be reused rather than rebuilt.
//!
//! Behaviour checklist:
//! - a painter's local clip follows its geometry when a cached leaf scrolls in
//! - output matches `NodeRenderer` for the same tree
//! - retained geometry uses the current pixel density and refreshes on change
//! - a subtree fade preserves RGB and matches cached vs uncached rendering
//! - a second composite reuses the cached layer list when nothing changed
//! - temporary backgrounds invalidate cached layers and restore authored values
//! - paint changes invalidate retained caches before dirty flags are flushed
//! - a new painter identity is visible on retry without a successful flush
//! - changed containers render directly before caching a stable frame
//! - flattening a stable layer does not duplicate nested container caches
//! - a non-layer descendant's dirty flag invalidates the enclosing layer
//! - a nested layer's dirty flag invalidates ancestor layer caches
//! - a new composite re-records a dirty layer
//! - opacity promotes a node to a layer root and refreshes its classification
//! - a stable painter identity preserves its retained layer cache
//! - a changed painter identity invalidates only its retained path
//! - scrolling translates a stable leaf painter without rebuilding its cache
//! - scrolling a painter-dense tree does not rebuild every leaf
//! - initially offscreen leaves do not enter ancestor caches
//! - offscreen cached leaves are culled and keep their overflow paint
//! - a clipped offscreen subtree avoids painter work until it enters the viewport

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, NodeTree, RenderTree};
use cove_core::{Color, DrawList, Point, Rect};

/// Keeps every `Node` alive for the test: `RenderObject` holds the node weakly,
/// so dropping a node also drops the object's binding.
struct Tree {
    node_tree: NodeTree,
    render: RenderTree,
    root: Node,
    a: Node,
    b: Node,
}

impl Tree {
    fn new() -> Self {
        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

        let a = Node::new();
        a.set_frame(Rect::new(10.0, 10.0, 100.0, 100.0));
        a.set_background_color(Some(Color::from_rgba8(255, 0, 0, 255)));
        root.add_child(&a);

        let b = Node::new();
        b.set_frame(Rect::new(5.0, 5.0, 50.0, 50.0));
        // Clipping promotes this node to a layer root.
        b.set_clips_to_bounds(true);
        b.set_background_color(Some(Color::from_rgba8(0, 255, 0, 255)));
        a.add_child(&b);

        let mut node_tree = NodeTree::new();
        node_tree.set_root(Some(root.clone()));

        let render = RenderTree::new();
        render.install(&root);
        Self {
            node_tree,
            render,
            root,
            a,
            b,
        }
    }

    fn object_of(&self, node: &Node) -> cove_core::scene::RenderObject {
        self.render
            .render_object(node)
            .expect("every installed node has a render object")
    }

    fn root_object(&self) -> cove_core::scene::RenderObject {
        self.render.root().expect("an installed tree has a root")
    }
}

fn alpha(vertex: &cove_core::Vertex) -> u32 {
    vertex.color >> 24
}

fn positions(list: &DrawList) -> Vec<(f32, f32)> {
    list.vertices().iter().map(|v| (v.x, v.y)).collect()
}

fn colors(list: &DrawList) -> Vec<u32> {
    list.vertices().iter().map(|v| v.color).collect()
}

#[test]
fn local_painter_clip_moves_with_leaf() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_clips_to_bounds(true);

    let leaf = Node::new();
    leaf.set_frame(Rect::new(0.0, 140.0, 20.0, 20.0));
    leaf.set_draw(Some(Rc::new(|list: &mut DrawList, origin: Point| {
        list.push_clip_rect(Rect::new(origin.x + 5.0, origin.y + 5.0, 10.0, 10.0));
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 20.0, 20.0), Color::WHITE);
        list.pop_clip_rect();
    })));
    root.add_child(&leaf);

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    // Warm the cache so the scrolled frame reuses a retained layer.
    for _ in 0..3 {
        renderer.render(&tree, &mut DrawList::default());
    }

    root.set_content_offset(Point { x: 0.0, y: 100.0 });
    let mut actual = DrawList::default();
    renderer.render(&tree, &mut actual);

    let mut expected = DrawList::default();
    NodeRenderer.render(&root, &mut expected);

    assert!(!actual.indices().is_empty(), "the scrolled leaf still drew");
    let actual_scissors: Vec<Option<Rect>> = actual.batches().iter().map(|b| b.scissor).collect();
    let expected_scissors: Vec<Option<Rect>> =
        expected.batches().iter().map(|b| b.scissor).collect();
    assert_eq!(
        actual_scissors, expected_scissors,
        "cached clips match the uncached render after scrolling"
    );
    assert!(
        actual.batches().iter().any(|batch| batch
            .scissor
            .is_some_and(|s| (s.y - 45.0).abs() < 1e-3 && (s.height - 10.0).abs() < 1e-3)),
        "the painter's local clip moved with the leaf to y=45",
    );
}

#[test]
fn parity_with_node_renderer() {
    let t = Tree::new();

    let mut reference = DrawList::default();
    NodeRenderer.render(&t.root, &mut reference);

    let mut layer = DrawList::default();
    LayerAwareNodeRenderer::new().render(&t.render, &mut layer);

    assert_eq!(layer.vertices().len(), reference.vertices().len());
    assert_eq!(layer.indices().len(), reference.indices().len());
    let first = |list: &DrawList| {
        let v = &list.vertices()[0];
        (v.x, v.y)
    };
    let last = |list: &DrawList| {
        let v = list.vertices().last().unwrap();
        (v.x, v.y)
    };
    assert_eq!(first(&layer), first(&reference), "first vertex matches");
    assert_eq!(last(&layer), last(&reference), "last vertex matches");
}

#[test]
fn pixel_density_parity() {
    let t = Tree::new();
    t.a.set_frame(Rect::new(10.26, 10.26, 100.0, 100.0));
    let mut renderer = LayerAwareNodeRenderer::new();

    for scale in [1.0_f32, 2.0, 2.0, 1.0] {
        let mut reference = DrawList::default();
        reference.pixel_scale = scale;
        NodeRenderer.render(&t.root, &mut reference);

        // The second pass at the same density must hit the retained geometry.
        for pass in 0..2 {
            let mut actual = DrawList::default();
            actual.pixel_scale = scale;
            renderer.render(&t.render, &mut actual);
            assert_eq!(
                positions(&actual),
                positions(&reference),
                "density {scale} pass {pass} matches the uncached render"
            );
        }
        assert_eq!(
            t.root_object()
                .cached_layer_list()
                .map(|list| list.pixel_scale),
            Some(scale),
            "the retained list records the density it was built at"
        );
    }
}

#[test]
fn subtree_fade_parity() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    let cached_leaf = t
        .object_of(&t.b)
        .cached_layer_list()
        .expect("the clipping node has a cached layer");

    t.a.set_subtree_opacity(0.5);
    let mut faded = DrawList::default();
    renderer.render(&t.render, &mut faded);
    let mut reference = DrawList::default();
    NodeRenderer.render(&t.root, &mut reference);

    assert_eq!(
        colors(&faded),
        colors(&reference),
        "faded output matches uncached"
    );
    assert!(
        faded.vertices().iter().all(|v| alpha(v) == 128),
        "a 0.5 subtree fade halves every alpha"
    );
    assert_eq!(
        faded.vertices()[0].color & 0x00ff_ffff,
        255,
        "RGB is preserved: straight-alpha fade, not a premultiplied one"
    );
    // A fade is applied at composite time, so the retained leaf list is not
    // rebuilt — only the composite pass changes alpha.
    let after_fade = t
        .object_of(&t.b)
        .cached_layer_list()
        .expect("the cached leaf survives a subtree fade");
    assert!(
        after_fade.geometry_shares_storage(&cached_leaf),
        "a subtree fade does not rebuild the retained leaf list"
    );
    assert_eq!(
        colors(&after_fade),
        colors(&cached_leaf),
        "the retained list keeps its original colours; the fade happens at composite"
    );

    t.a.set_subtree_opacity(1.0);
    let mut restored = DrawList::default();
    renderer.render(&t.render, &mut restored);
    assert!(
        restored.vertices().iter().all(|v| alpha(v) == 255),
        "removing the fade restores full alpha"
    );
}

#[test]
fn cache_reuse() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();

    let mut first = DrawList::default();
    renderer.render(&t.render, &mut first);

    let leaf = t.object_of(&t.b);
    assert!(
        leaf.is_layer_root(),
        "clipping promotes the leaf to a layer root"
    );
    assert!(
        !leaf.cache_invalid(),
        "a fresh composite leaves the cache valid"
    );
    let cached = leaf
        .cached_layer_list()
        .expect("the leaf recorded a layer list");

    let mut second = DrawList::default();
    renderer.render(&t.render, &mut second);

    let reused = leaf
        .cached_layer_list()
        .expect("the second composite keeps the retained layer list");
    assert!(
        reused.geometry_shares_storage(&cached),
        "the second composite reuses the same layer list"
    );
    assert_eq!(second.vertices().len(), first.vertices().len());
    assert_eq!(second.indices().len(), first.indices().len());
}

#[test]
fn debug_styles_invalidate_cached_layer() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());

    let white = Color::WHITE.rgba8();
    let black = Color::BLACK.rgba8();

    t.b.set_debug_background_color(Some(Color::WHITE));
    let mut list = DrawList::default();
    renderer.render(&t.render, &mut list);
    assert!(
        list.vertices().iter().any(|vertex| vertex.color == white),
        "the temporary background paints over the authored colour",
    );

    t.b.set_background_color(Some(Color::BLACK));
    let mut list = DrawList::default();
    renderer.render(&t.render, &mut list);
    assert!(
        list.vertices().iter().any(|vertex| vertex.color == white),
        "the temporary background still wins while it is installed",
    );

    t.b.set_debug_background_color(None);
    let mut list = DrawList::default();
    renderer.render(&t.render, &mut list);
    assert!(
        list.vertices().iter().any(|vertex| vertex.color == black),
        "clearing the temporary background restores the authored colour",
    );
    assert!(
        !list.vertices().iter().any(|vertex| vertex.color == white),
        "the temporary colour no longer appears",
    );
}

#[test]
fn changes_between_unflushed_passes() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    // A failed surface acquisition can leave node dirty flags set even though
    // composing the frame has already made the layer caches valid.
    renderer.render(&t.render, &mut DrawList::default());
    renderer.render(&t.render, &mut DrawList::default());

    assert!(
        t.b.render_dirty(),
        "an unflushed dirty flag survives a composite"
    );
    let root = t.root_object();
    assert!(
        root.cached_layer_list().is_some(),
        "the root retained a layer list"
    );
    assert!(!root.cache_invalid(), "the retained root cache is valid");

    t.b.set_background_color(Some(Color::from_rgba8(0, 0, 255, 255)));
    assert!(
        root.cache_invalid(),
        "a paint change invalidates the enclosing cache"
    );
    assert!(
        t.object_of(&t.b).cache_invalid(),
        "the painted leaf is invalidated too",
    );

    let mut actual = DrawList::default();
    let mut expected = DrawList::default();
    renderer.render(&t.render, &mut actual);
    NodeRenderer.render(&t.root, &mut expected);
    assert_eq!(
        colors(&actual),
        colors(&expected),
        "the refreshed cache matches the uncached render",
    );
}

#[test]
fn painter_changes_between_unflushed_passes() {
    let t = Tree::new();
    let paint = |width: f32, color: Color| {
        move |list: &mut DrawList, origin: Point| {
            let _ = list.add_rect(Rect::new(origin.x, origin.y, width, width), color);
        }
    };
    t.b.update_draw(1_u32, paint(8.0, Color::WHITE));
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    renderer.render(&t.render, &mut DrawList::default());

    t.b.update_draw(2_u32, paint(12.0, Color::BLACK));
    assert!(
        t.root_object().cache_invalid(),
        "a new painter identity invalidates before any flush"
    );

    let mut actual = DrawList::default();
    let mut expected = DrawList::default();
    renderer.render(&t.render, &mut actual);
    NodeRenderer.render(&t.root, &mut expected);
    assert_eq!(
        actual.vertices().iter().map(|v| v.x).collect::<Vec<_>>(),
        expected.vertices().iter().map(|v| v.x).collect::<Vec<_>>(),
    );
    assert_eq!(
        colors(&actual),
        colors(&expected),
        "the new painter is visible on the retry pass",
    );
}

#[test]
fn container_cache_is_deferred_until_stable() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    let root = t.root_object();

    renderer.render(&t.render, &mut DrawList::default());
    assert!(
        root.cached_layer_list().is_none(),
        "a changed container renders directly instead of caching",
    );
    assert!(
        !root.cache_invalid(),
        "the direct render leaves the object valid"
    );

    renderer.render(&t.render, &mut DrawList::default());
    let stable = root
        .cached_layer_list()
        .expect("the second pass caches the stable container");

    renderer.render(&t.render, &mut DrawList::default());
    let reused = root
        .cached_layer_list()
        .expect("the third pass still holds a cache");
    assert!(
        reused.geometry_shares_storage(&stable),
        "the third pass reuses the stable cache",
    );
}

#[test]
fn nested_container_cache_storage() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let mut containers = vec![root.clone()];
    for _ in 0..24 {
        let child = Node::new();
        child.set_frame(Rect::new(1.0, 1.0, 150.0, 150.0));
        child.set_clips_to_bounds(true);
        child.set_background_color(Some(Color {
            r: 0.2,
            g: 0.3,
            b: 0.4,
            a: 1.0,
        }));
        containers.last().unwrap().add_child(&child);
        containers.push(child);
    }

    let render = RenderTree::new();
    render.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();

    for opacity in [1.0_f32, 0.5, 1.0] {
        containers[12].set_subtree_opacity(opacity);
        for _ in 0..3 {
            let mut reference = DrawList::default();
            let mut actual = DrawList::default();
            NodeRenderer.render(&root, &mut reference);
            renderer.render(&render, &mut actual);
            assert_eq!(
                positions(&actual),
                positions(&reference),
                "positions match the uncached render at opacity {opacity}",
            );
            assert_eq!(
                colors(&actual),
                colors(&reference),
                "colours match the uncached render at opacity {opacity}",
            );
            assert_eq!(
                actual.indices(),
                reference.indices(),
                "indices match the uncached render at opacity {opacity}",
            );
        }
        assert!(
            render
                .root()
                .and_then(|object| object.cached_layer_list())
                .is_some(),
            "the root is cached at opacity {opacity}"
        );
        // A stable branch may retain a prior cache after an ancestor edit, but
        // rebuilding the root must not allocate one at every depth.
        let cached_containers = containers[1..containers.len() - 1]
            .iter()
            .filter(|container| {
                render
                    .render_object(container)
                    .is_some_and(|object| object.cached_layer_list().is_some())
            })
            .count();
        assert!(
            cached_containers <= 1,
            "at most one nested container keeps a cache, got {cached_containers}",
        );
    }
}

#[test]
fn dirty_bubbles_to_enclosing_layer() {
    // Root is its own layer; child A is non-layer; leaf B is its own layer.
    // Mutating A (non-layer) invalidates ROOT (the nearest layer root above A)
    // but not leaf B (a separate layer).
    let mut t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    t.node_tree.flush();

    let root = t.root_object();
    let leaf = t.object_of(&t.b);
    assert!(!root.cache_invalid());
    assert!(!leaf.cache_invalid());

    t.a.set_background_color(Some(Color::from_rgba8(0, 0, 255, 255)));

    assert!(root.cache_invalid(), "the enclosing layer is invalidated");
    assert!(!leaf.cache_invalid(), "a separate layer below is untouched");
}

#[test]
fn dirty_nested_layer_invalidates_ancestor_layer_caches() {
    let mut t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    t.node_tree.flush();

    let root = t.root_object();
    let leaf = t.object_of(&t.b);
    assert!(!root.cache_invalid());
    assert!(!leaf.cache_invalid());

    t.b.set_content_offset(Point { x: 0.0, y: 12.0 });

    assert!(leaf.cache_invalid(), "the nested layer is invalidated");
    assert!(
        root.cache_invalid(),
        "the ancestor cache is invalidated too"
    );
}

#[test]
fn re_records_after_invalidation() {
    let mut t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    t.node_tree.flush();

    let leaf = t.object_of(&t.b);
    let first = leaf
        .cached_layer_list()
        .expect("the leaf recorded a layer list");

    t.b.set_background_color(Some(Color::from_rgba8(128, 128, 128, 255)));
    assert!(
        leaf.cache_invalid(),
        "the paint change invalidates the leaf"
    );

    renderer.render(&t.render, &mut DrawList::default());
    let second = leaf
        .cached_layer_list()
        .expect("the leaf re-recorded a layer list");
    assert_ne!(
        colors(&second),
        colors(&first),
        "the new composite re-recorded the dirty layer",
    );
    assert!(!leaf.cache_invalid(), "the fresh cache is valid again");
}

#[test]
fn opacity_promotes_and_invalidates_classification() {
    let t = Tree::new();
    let a = t.object_of(&t.a);
    assert!(!a.is_layer_root(), "a plain container is not a layer root");

    t.a.set_opacity(0.5);
    assert!(
        a.is_layer_root(),
        "opacity promotes the node to a layer root"
    );
}

#[test]
fn stable_painter_identity_preserves_cache() {
    let mut t = Tree::new();
    t.b.update_draw("same", |list: &mut DrawList, origin: Point| {
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 4.0, 4.0), Color::WHITE);
    });
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());

    let object = t.object_of(&t.b);
    assert!(object.is_layer_root());
    let first = object
        .cached_layer_list()
        .expect("the leaf recorded a layer list");

    t.node_tree.flush();
    t.b.update_draw("same", |_: &mut DrawList, _: Point| {});

    assert!(!t.root.render_dirty(), "the root is not dirty");
    assert!(
        !object.cache_invalid(),
        "re-authoring the same identity is not a change"
    );
    renderer.render(&t.render, &mut DrawList::default());
    let reused = object
        .cached_layer_list()
        .expect("the cache survives a re-author");
    assert!(
        reused.geometry_shares_storage(&first),
        "the stable identity keeps its retained cache",
    );
}

#[test]
fn changed_painter_identity_invalidates() {
    let mut t = Tree::new();
    t.a.update_draw(1_u32, |_: &mut DrawList, _: Point| {});
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    t.node_tree.flush();

    let object = t.object_of(&t.a);
    t.a.update_draw(2_u32, |_: &mut DrawList, _: Point| {});

    assert!(object.cache_invalid(), "the changed path is invalidated");
    assert!(
        t.root_object().cache_invalid(),
        "the enclosing cache is invalidated"
    );
    assert!(t.root.render_dirty(), "the node is marked render-dirty");
}

#[test]
fn scrolling_translates_stable_leaf_painter() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let mut node_tree = NodeTree::new();
    node_tree.set_root(Some(root.clone()));

    let scroll = Node::new();
    scroll.set_frame(Rect::new(10.0, 10.0, 100.0, 80.0));
    scroll.set_clips_to_bounds(true);
    root.add_child(&scroll);

    let leaf = Node::new();
    leaf.set_frame(Rect::new(4.0, 30.0, 40.0, 20.0));
    leaf.update_draw("stable", |list: &mut DrawList, origin: Point| {
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 40.0, 20.0), Color::WHITE);
    });
    scroll.add_child(&leaf);

    let render_tree = RenderTree::new();
    render_tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&render_tree, &mut DrawList::default());
    node_tree.flush();

    let leaf_object = render_tree
        .render_object(&leaf)
        .expect("the leaf has a render object");
    let first = leaf_object
        .cached_layer_list()
        .expect("the leaf recorded a layer list");

    scroll.set_content_offset(Point { x: 0.0, y: 12.0 });

    let mut actual = DrawList::default();
    renderer.render(&render_tree, &mut actual);
    let mut expected = DrawList::default();
    NodeRenderer.render(&root, &mut expected);

    let reused = leaf_object
        .cached_layer_list()
        .expect("the translated leaf keeps its cache");
    assert!(
        reused.geometry_shares_storage(&first),
        "scrolling translates the retained list instead of rebuilding it",
    );
    assert_eq!(positions(&actual), positions(&expected));
    let actual_scissors: Vec<Option<Rect>> = actual.batches().iter().map(|b| b.scissor).collect();
    let expected_scissors: Vec<Option<Rect>> =
        expected.batches().iter().map(|b| b.scissor).collect();
    assert_eq!(actual_scissors, expected_scissors);
}

#[test]
fn painter_dense_scroll_stress() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 1280.0, 720.0));
    let scroll = Node::new();
    scroll.set_frame(Rect::new(0.0, 0.0, 1280.0, 720.0));
    scroll.set_clips_to_bounds(true);
    root.add_child(&scroll);

    let painter_calls = Rc::new(Cell::new(0_usize));
    for row in 0..300_usize {
        let leaf = Node::new();
        leaf.set_frame(Rect::new(12.0, row as f32 * 28.0, 900.0, 24.0));
        let calls = painter_calls.clone();
        leaf.update_draw(row, move |list: &mut DrawList, origin: Point| {
            calls.set(calls.get() + 1);
            for column in 0..20 {
                let _ = list.add_glyph_quad(
                    Rect::new(origin.x + column as f32 * 10.0, origin.y, 8.0, 16.0),
                    1,
                    Color::WHITE,
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 1.0, y: 1.0 },
                );
            }
        });
        scroll.add_child(&leaf);
    }

    let render_tree = RenderTree::new();
    render_tree.install(&root);
    let mut retained = LayerAwareNodeRenderer::new();
    retained.render(&render_tree, &mut DrawList::default());
    let calls_after_warmup = painter_calls.get();
    assert!(
        calls_after_warmup > 0,
        "the warm-up pass paints the visible leaves"
    );

    let mut retained_frame = DrawList::default();
    for frame in 0..30 {
        scroll.set_content_offset(Point {
            x: 0.0,
            y: frame as f32 * 3.0,
        });
        retained_frame.reset();
        retained.render(&render_tree, &mut retained_frame);
    }
    assert_eq!(
        painter_calls.get(),
        calls_after_warmup,
        "retained layers do not repaint while only the scroll offset changes",
    );

    let legacy = NodeRenderer::new();
    let mut legacy_frame = DrawList::default();
    for frame in 0..30 {
        scroll.set_content_offset(Point {
            x: 0.0,
            y: frame as f32 * 3.0,
        });
        legacy_frame.reset();
        legacy.render(&root, &mut legacy_frame);
    }

    let visible_legacy: Vec<(f32, f32)> = legacy_frame
        .vertices()
        .iter()
        .filter(|v| v.y >= 0.0 && v.y <= 720.0)
        .map(|v| (v.x, v.y))
        .collect();
    let visible_retained: Vec<(f32, f32)> = retained_frame
        .vertices()
        .iter()
        .filter(|v| v.y >= 0.0 && v.y <= 720.0)
        .map(|v| (v.x, v.y))
        .collect();
    assert_eq!(visible_retained, visible_legacy, "visible geometry matches");
    assert!(
        retained_frame.vertices().len() < legacy_frame.vertices().len() / 5,
        "culling keeps the retained frame far smaller: {} vs {}",
        retained_frame.vertices().len(),
        legacy_frame.vertices().len(),
    );
    assert_eq!(
        painter_calls.get(),
        calls_after_warmup + 300 * 30,
        "the uncached renderer repaints every leaf on every frame",
    );
}

#[test]
fn initially_offscreen_leaf() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_clips_to_bounds(true);
    let leaf = Node::new();
    leaf.set_frame(Rect::new(0.0, 140.0, 20.0, 20.0));
    let paints = Rc::new(Cell::new(0_usize));
    let counter = paints.clone();
    leaf.set_draw(Some(Rc::new(move |list: &mut DrawList, origin: Point| {
        counter.set(counter.get() + 1);
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 20.0, 20.0), Color::WHITE);
    })));
    root.add_child(&leaf);

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    for pass in 0..3 {
        let mut frame = DrawList::default();
        renderer.render(&tree, &mut frame);
        assert!(
            frame.vertices().is_empty(),
            "offscreen pass {pass} draws nothing"
        );
    }
    assert_eq!(paints.get(), 1, "the offscreen leaf painted once");

    root.set_content_offset(Point { x: 0.0, y: 100.0 });
    let mut visible = DrawList::default();
    renderer.render(&tree, &mut visible);
    assert_eq!(visible.vertices().len(), 4, "the leaf reappears");
    assert_eq!(paints.get(), 1, "reappearing does not repaint it");
}

#[test]
fn cached_leaf_visibility() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_clips_to_bounds(true);
    let leaf = Node::new();
    leaf.set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));
    let paints = Rc::new(Cell::new(0_usize));
    let counter = paints.clone();
    leaf.set_draw(Some(Rc::new(move |list: &mut DrawList, origin: Point| {
        counter.set(counter.get() + 1);
        // The painter intentionally extends beyond its frame.
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 20.0, 60.0), Color::WHITE);
    })));
    root.add_child(&leaf);

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&tree, &mut DrawList::default());

    root.set_content_offset(Point { x: 0.0, y: 100.0 });
    let mut outside = DrawList::default();
    renderer.render(&tree, &mut outside);
    assert!(outside.vertices().is_empty(), "the leaf is scrolled away");

    root.set_content_offset(Point { x: 0.0, y: 30.0 });
    let mut overflow = DrawList::default();
    renderer.render(&tree, &mut overflow);
    assert!(
        !overflow.vertices().is_empty(),
        "the frame is outside but the actual painter is visible",
    );
    assert_eq!(paints.get(), 1, "the cached overflow paint is reused");

    root.set_content_offset(Point::default());
    let mut returned = DrawList::default();
    renderer.render(&tree, &mut returned);
    assert_eq!(returned.vertices().len(), 4);
    assert_eq!(paints.get(), 1);
}

#[test]
fn clipped_subtree_visibility() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    let clipped = Node::new();
    clipped.set_frame(Rect::new(0.0, 120.0, 100.0, 80.0));
    clipped.set_clips_to_bounds(true);
    let child = Node::new();
    child.set_frame(Rect::new(0.0, 0.0, 50.0, 50.0));
    let paints = Rc::new(Cell::new(0_usize));
    let counter = paints.clone();
    child.set_draw(Some(Rc::new(move |list: &mut DrawList, origin: Point| {
        counter.set(counter.get() + 1);
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 50.0, 50.0), Color::WHITE);
    })));
    root.add_child(&clipped);
    clipped.add_child(&child);

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    let mut first = DrawList::default();
    renderer.render(&tree, &mut first);
    assert_eq!(
        paints.get(),
        0,
        "the offscreen subtree does no painter work"
    );
    assert!(first.vertices().is_empty());

    root.set_content_offset(Point { x: 0.0, y: 100.0 });
    let mut next = DrawList::default();
    renderer.render(&tree, &mut next);
    assert_eq!(paints.get(), 1, "entering the viewport paints once");
    assert_eq!(next.vertices().len(), 4);
}
