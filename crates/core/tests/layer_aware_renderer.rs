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

use std::rc::Rc;

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, RenderTree};
use cove_core::{Color, DrawList, Point, Rect};

/// Keeps every `Node` alive for the test: `RenderObject` holds the node weakly,
/// so dropping a node also drops the object's binding.
struct Tree {
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

        let render = RenderTree::new();
        render.install(&root);
        Self { render, root, a, b }
    }
}

fn alpha(vertex: &cove_core::Vertex) -> u32 {
    vertex.color >> 24
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
            let actual_positions: Vec<(f32, f32)> =
                actual.vertices().iter().map(|v| (v.x, v.y)).collect();
            let reference_positions: Vec<(f32, f32)> =
                reference.vertices().iter().map(|v| (v.x, v.y)).collect();
            assert_eq!(
                actual_positions, reference_positions,
                "density {scale} pass {pass} matches the uncached render"
            );
        }
        assert_eq!(
            t.render
                .root()
                .and_then(|root| root.cached_layer_list())
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
        .render
        .render_object(&t.b)
        .and_then(|object| object.cached_layer_list())
        .expect("the clipping node has a cached layer");

    t.a.set_subtree_opacity(0.5);
    let mut faded = DrawList::default();
    renderer.render(&t.render, &mut faded);
    let mut reference = DrawList::default();
    NodeRenderer.render(&t.root, &mut reference);

    let faded_colors: Vec<u32> = faded.vertices().iter().map(|v| v.color).collect();
    let reference_colors: Vec<u32> = reference.vertices().iter().map(|v| v.color).collect();
    assert_eq!(
        faded_colors, reference_colors,
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
    // A fade is applied at composite time, so the retained leaf list keeps the
    // geometry it was built with — only the composite pass changes alpha.
    let after_fade = t
        .render
        .render_object(&t.b)
        .and_then(|object| object.cached_layer_list())
        .expect("the cached leaf survives a subtree fade");
    assert_eq!(
        after_fade.vertices().len(),
        cached_leaf.vertices().len(),
        "a subtree fade does not rebuild the retained leaf list"
    );
    assert_eq!(
        after_fade
            .vertices()
            .iter()
            .map(|v| v.color)
            .collect::<Vec<_>>(),
        cached_leaf
            .vertices()
            .iter()
            .map(|v| v.color)
            .collect::<Vec<_>>(),
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

    let leaf = t
        .render
        .render_object(&t.b)
        .expect("the leaf has a render object");
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

    assert_eq!(
        leaf.cached_layer_list().map(|list| list.vertices().len()),
        Some(cached.vertices().len()),
        "the second composite keeps the retained layer list"
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
    renderer.render(&t.render, &mut DrawList::default());
    renderer.render(&t.render, &mut DrawList::default());

    assert!(
        t.b.render_dirty(),
        "an unflushed dirty flag survives a composite"
    );
    let root = t
        .render
        .root()
        .expect("an installed render tree has a root");
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
        t.render
            .render_object(&t.b)
            .is_some_and(|object| object.cache_invalid()),
        "the painted leaf is invalidated too",
    );

    let mut actual = DrawList::default();
    let mut expected = DrawList::default();
    renderer.render(&t.render, &mut actual);
    NodeRenderer.render(&t.root, &mut expected);
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
    t.b.update_draw(1, paint(8.0, Color::WHITE));
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());
    renderer.render(&t.render, &mut DrawList::default());

    t.b.update_draw(2, paint(12.0, Color::BLACK));
    let root = t
        .render
        .root()
        .expect("an installed render tree has a root");
    assert!(
        root.cache_invalid(),
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
        "the new painter is visible on the retry pass",
    );
}

#[test]
fn container_cache_is_deferred_until_stable() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    let root = t
        .render
        .root()
        .expect("an installed render tree has a root");

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
    assert_eq!(
        root.cached_layer_list().map(|list| list.vertices().len()),
        Some(stable.vertices().len()),
        "the third pass reuses the stable cache",
    );
}

#[test]
fn dirty_bubbles_to_enclosing_layer() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());

    let root = t
        .render
        .root()
        .expect("an installed render tree has a root");
    let leaf = t
        .render
        .render_object(&t.b)
        .expect("the leaf has a render object");
    assert!(!root.cache_invalid());
    assert!(!leaf.cache_invalid());

    // A is not a layer root, so its dirty flag invalidates the nearest enclosing
    // layer (the root) and leaves the separate leaf layer alone.
    t.a.set_background_color(Some(Color::from_rgba8(0, 0, 255, 255)));
    assert!(root.cache_invalid(), "the enclosing layer is invalidated");
    assert!(!leaf.cache_invalid(), "a separate layer below is untouched");
}

#[test]
fn dirty_nested_layer_invalidates_ancestor_caches() {
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());

    let root = t
        .render
        .root()
        .expect("an installed render tree has a root");
    let leaf = t
        .render
        .render_object(&t.b)
        .expect("the leaf has a render object");
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
    let t = Tree::new();
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());

    let leaf = t
        .render
        .render_object(&t.b)
        .expect("the leaf has a render object");
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
        second
            .vertices()
            .iter()
            .map(|v| v.color)
            .collect::<Vec<_>>(),
        first.vertices().iter().map(|v| v.color).collect::<Vec<_>>(),
        "the new composite re-recorded the dirty layer",
    );
    assert!(!leaf.cache_invalid(), "the fresh cache is valid again");
}

#[test]
fn opacity_promotes_and_invalidates_classification() {
    let t = Tree::new();
    let a = t.render.render_object(&t.a).expect("A has a render object");
    assert!(!a.is_layer_root(), "a plain container is not a layer root");

    t.a.set_opacity(0.5);
    assert!(
        a.is_layer_root(),
        "opacity promotes the node to a layer root"
    );
}

#[test]
fn stable_painter_identity_preserves_cache() {
    let t = Tree::new();
    t.b.update_draw("same", |list: &mut DrawList, origin: Point| {
        let _ = list.add_rect(Rect::new(origin.x, origin.y, 4.0, 4.0), Color::WHITE);
    });
    let mut renderer = LayerAwareNodeRenderer::new();
    renderer.render(&t.render, &mut DrawList::default());

    let object = t
        .render
        .render_object(&t.b)
        .expect("the leaf has a render object");
    assert!(object.is_layer_root());
    let first = object
        .cached_layer_list()
        .expect("the leaf recorded a layer list");

    t.b.update_draw("same", |_: &mut DrawList, _: Point| {});
    assert!(
        !object.cache_invalid(),
        "re-authoring the same identity is not a change"
    );

    renderer.render(&t.render, &mut DrawList::default());
    assert_eq!(
        object.cached_layer_list().map(|list| list.vertices().len()),
        Some(first.vertices().len()),
        "the stable identity keeps its retained cache",
    );
}
