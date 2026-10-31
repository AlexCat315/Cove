//! Translation of `Tests/CoveUIRuntimeTests/NodeTransformRenderTests.swift`.
//!
//! `NodeTransformTests` pins the geometry the rest of the system asks a node
//! about; these pin that what comes out of `DrawList` matches it — in both
//! renderers, and through the layer cache that could otherwise serve geometry
//! recorded at the wrong size.
//!
//! Behaviour checklist:
//! - a scaled node paints at the size its geometry reports
//! - a child's fill lands inside its scaled ancestor
//! - a centre anchor pins the middle of the box
//! - scroll is applied before the scale on the same node
//! - corner radius and border width scale with the box
//! - the layer-aware renderer draws the same geometry
//! - changing only the scale re-records the cached layer
//! - a moved layer at an unchanged scale still reuses its cache

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, NodeTransform, RenderTree};
use cove_core::{Color, DrawList, Point, Rect, Vertex};

/// Bounds of everything a render emitted.
fn painted_bounds(list: &DrawList) -> Option<Rect> {
    bounds_of(list.vertices().iter())
}

/// Bounds of the red fills alone, so a red node can be measured inside a tree
/// whose root paints something else. Opaque red is `0xFF0000FF` in `rgba8` byte
/// order, which is the low byte of the packed colour.
fn red_bounds(list: &DrawList) -> Option<Rect> {
    bounds_of(
        list.vertices()
            .iter()
            .filter(|vertex| (vertex.color >> 24) & 0xFF == 0xFF),
    )
}

fn bounds_of<'a>(vertices: impl Iterator<Item = &'a Vertex>) -> Option<Rect> {
    let mut vertices = vertices.peekable();
    let first = *vertices.peek()?;
    let (mut min_x, mut max_x) = (first.x, first.x);
    let (mut min_y, mut max_y) = (first.y, first.y);
    for vertex in vertices {
        min_x = min_x.min(vertex.x);
        max_x = max_x.max(vertex.x);
        min_y = min_y.min(vertex.y);
        max_y = max_y.max(vertex.y);
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

fn filled(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node.set_background_color(Some(Color::from_rgba8(255, 0, 0, 255)));
    node
}

/// A painted but transparent node, so a child's bounds can be read without its
/// ancestors contributing to the union.
fn plain(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

#[test]
fn a_scaled_node_paints_at_the_size_its_geometry_reports() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    let panel = filled(100.0, 40.0, 200.0, 100.0);
    root.add_child(&panel);
    panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));

    let mut list = DrawList::default();
    NodeRenderer::new().render(&root, &mut list);

    assert_eq!(
        red_bounds(&list),
        Some(Rect::new(100.0, 40.0, 100.0, 50.0)),
        "what is drawn and what the rest of the system asks about must agree",
    );
    assert_eq!(panel.absolute_frame(), Rect::new(100.0, 40.0, 100.0, 50.0));
}

#[test]
fn a_childs_fill_lands_inside_its_scaled_ancestor() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    let group = plain(100.0, 100.0, 200.0, 200.0);
    root.add_child(&group);
    let child = filled(0.0, 0.0, 40.0, 40.0);
    group.add_child(&child);
    group.set_transform(NodeTransform::anchored(2.0, 0.0, 0.0));

    let mut list = DrawList::default();
    NodeRenderer::new().render(&root, &mut list);

    assert_eq!(
        red_bounds(&list),
        Some(Rect::new(100.0, 100.0, 80.0, 80.0))
    );
    assert_eq!(child.absolute_frame(), Rect::new(100.0, 100.0, 80.0, 80.0));
}

#[test]
fn a_centre_anchor_pins_the_middle_of_the_box() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    let panel = filled(100.0, 40.0, 200.0, 100.0);
    root.add_child(&panel);
    panel.set_transform(NodeTransform::anchored(0.5, 0.5, 0.5));

    let mut list = DrawList::default();
    NodeRenderer::new().render(&root, &mut list);

    let drawn = red_bounds(&list).expect("the panel painted");
    assert_eq!(drawn, Rect::new(150.0, 65.0, 100.0, 50.0));
    assert_eq!(drawn.x + drawn.width / 2.0, 200.0);
    assert_eq!(drawn.y + drawn.height / 2.0, 90.0);
}

#[test]
fn scroll_is_applied_before_the_scale_on_the_same_node() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    let viewport = filled(40.0, 20.0, 300.0, 200.0);
    root.add_child(&viewport);
    let content = filled(0.0, 0.0, 300.0, 900.0);
    viewport.add_child(&content);
    viewport.set_content_offset(Point { x: 0.0, y: 100.0 });
    viewport.set_transform(NodeTransform::anchored(2.0, 0.0, 0.0));

    let mut list = DrawList::default();
    NodeRenderer::new().render(&root, &mut list);

    // 100pt of scroll inside a box drawn at 2x puts the content 200pt up, at
    // 2x the width. Scaling the scroll instead would halve the speed the offset
    // asked for, and no other test in the tree would notice.
    assert_eq!(
        red_bounds(&list),
        Some(Rect::new(40.0, -180.0, 600.0, 1800.0))
    );
}

#[test]
fn corner_radius_and_border_width_scale_with_the_box() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    let panel = Node::new();
    panel.set_frame(Rect::new(20.0, 20.0, 100.0, 50.0));
    panel.set_corner_radius(10.0);
    panel.set_border_width(2.0);
    panel.set_border_color(Some(Color::WHITE));
    root.add_child(&panel);
    panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));

    let mut list = DrawList::default();
    NodeRenderer::new().render(&root, &mut list);

    // A 10pt corner drawn at half size is a 5pt corner. Every length on the
    // box is measured in the box's units.
    assert_eq!(panel.absolute_frame(), Rect::new(20.0, 20.0, 50.0, 25.0));
    assert_eq!(panel.corner_radius() * 0.5, 5.0);
    assert_eq!(panel.border_width() * 0.5, 1.0);

    // The stroke is centred on the edge, so the painted bounds are the scaled
    // frame expanded by half the *scaled* border width. Left unscaled it would
    // expand by 1.0 and the box would read as thicker the smaller it gets.
    let painted = painted_bounds(&list).expect("the panel painted");
    assert_eq!(
        painted,
        Rect::new(19.5, 19.5, 51.0, 26.0),
        "a 2pt border at half scale paints a 1pt stroke around a 50x25 box",
    );
}

#[test]
fn the_layer_aware_renderer_draws_the_same_geometry() {
    for scale in [1.0_f32, 0.5, 1.5] {
        let root = plain(0.0, 0.0, 400.0, 400.0);
        let group = Node::new();
        group.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        group.set_clips_to_bounds(true);
        root.add_child(&group);
        group.add_child(&filled(0.0, 0.0, 40.0, 40.0));
        group.set_transform(NodeTransform::anchored(scale, 0.5, 0.5));
        let tree = RenderTree::new();
        tree.install(&root);

        let mut direct = DrawList::default();
        NodeRenderer::new().render(&root, &mut direct);

        let mut cached = DrawList::default();
        LayerAwareNodeRenderer::new().render(&tree, &mut cached);

        assert_eq!(
            painted_bounds(&direct),
            painted_bounds(&cached),
            "scale {scale}: a tree drawn through the layer cache must look the same",
        );
    }
}

#[test]
fn changing_only_the_scale_re_records_the_cached_layer() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    let panel = filled(20.0, 20.0, 80.0, 40.0);
    panel.set_clips_to_bounds(true);
    root.add_child(&panel);
    let tree = RenderTree::new();
    tree.install(&root);

    let mut renderer = LayerAwareNodeRenderer::new();
    let mut first = DrawList::default();
    renderer.render(&tree, &mut first);
    // The fill, not everything painted: a shadow's spread is not scaled with
    // the box, and it would swamp a one-pixel difference.
    let before = red_bounds(&first).expect("the panel painted");

    // Anchored top-left so the origin cannot move; only the size changes.
    panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));
    let mut second = DrawList::default();
    renderer.render(&tree, &mut second);
    let after = red_bounds(&second).expect("the panel painted");

    assert_eq!(panel.absolute_origin(), Point { x: 20.0, y: 20.0 });
    assert_eq!(before, Rect::new(20.0, 20.0, 80.0, 40.0));
    assert_eq!(
        after,
        Rect::new(20.0, 20.0, 40.0, 20.0),
        "an origin-only cache check would have served the old size",
    );
}

#[test]
fn a_moved_layer_at_an_unchanged_scale_still_reuses_its_cache() {
    let root = plain(0.0, 0.0, 400.0, 400.0);
    // Clipping promotes it to a layer root, which is what puts it in the
    // per-layer cache this test is about.
    let chip = filled(20.0, 20.0, 60.0, 30.0);
    chip.set_clips_to_bounds(true);
    root.add_child(&chip);
    let tree = RenderTree::new();
    tree.install(&root);

    let mut renderer = LayerAwareNodeRenderer::new();
    let mut first = DrawList::default();
    renderer.render(&tree, &mut first);
    assert_eq!(red_bounds(&first), Some(Rect::new(20.0, 20.0, 60.0, 30.0)));

    chip.set_frame(Rect::new(80.0, 20.0, 60.0, 30.0));
    chip.mark_render_dirty(cove_core::scene::InvalidationSource::StructuralChange);
    let mut second = DrawList::default();
    renderer.render(&tree, &mut second);

    assert_eq!(red_bounds(&second), Some(Rect::new(80.0, 20.0, 60.0, 30.0)));
}
