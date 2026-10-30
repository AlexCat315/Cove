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

use cove_core::scene::{
    LayerAwareNodeRenderer, Node, NodeRenderer, RenderTree,
};
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
        actual
            .batches()
            .iter()
            .any(|batch| batch.scissor.is_some_and(|s| (s.y - 45.0).abs() < 1e-3 && (s.height - 10.0).abs() < 1e-3)),
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
            t.render.root().and_then(|root| root.cached_layer_list()).map(|list| list.pixel_scale),
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
    assert_eq!(faded_colors, reference_colors, "faded output matches uncached");
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
        after_fade.vertices().iter().map(|v| v.color).collect::<Vec<_>>(),
        cached_leaf.vertices().iter().map(|v| v.color).collect::<Vec<_>>(),
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
