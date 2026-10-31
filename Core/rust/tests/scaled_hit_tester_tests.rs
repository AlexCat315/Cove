//! Translation of `Tests/CoveUIRuntimeTests/Transforms/HitTestingTests.swift`.
//!
//! The contract for a scale: what is drawn and what is clickable are the same
//! rectangle. When they disagree a control shrinks toward a corner while the
//! part that visibly moved keeps taking clicks, or a button is hit in a margin
//! where nothing is painted.
//!
//! Behaviour checklist:
//! - a shrunken node is not hit in the space it vacated
//! - the drawn rectangle and the hittable one are the same one
//! - a centre anchor moves the hit area, not just the size
//! - a child is hit where it is drawn inside a scaled ancestor
//! - scroll is undone before the scale, matching what is drawn
//! - a collapsed node is not hit at all
//! - `local_point` stays in the node's own unscaled space
//! - a scaled clipping container still rejects hits outside its box

use cove_core::scene::{HitTester, Node, NodeTransform};
use cove_core::{Color, Point, Rect};

/// The Swift `scene(_:)` helper: an 800×600 root.
fn scene(build: impl FnOnce(&Node)) -> Node {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    build(&root);
    root
}

fn hit_id(root: &Node, x: f32, y: f32) -> Option<u64> {
    HitTester::hit_test(root, Point { x, y }).map(|hit| hit.node.id().0)
}

#[test]
fn vacated_space_is_dead() {
    let panel = Node::new();
    let root = scene(|root| {
        panel.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        root.add_child(&panel);
        panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));
    });

    // 200×200 at (100,100) halved about its top-left now covers
    // (100,100)-(200,200).
    assert_eq!(
        hit_id(&root, 150.0, 150.0),
        Some(panel.id().0),
        "inside the shrunken box"
    );
    assert_ne!(
        hit_id(&root, 250.0, 250.0),
        Some(panel.id().0),
        "the vacated corner is dead"
    );
}

#[test]
fn drawn_matches_hittable() {
    let panel = Node::new();
    let root = scene(|root| {
        panel.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        panel.set_background_color(Some(Color::from_rgba8(255, 0, 0, 255)));
        root.add_child(&panel);
        panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));
    });

    let frame = panel.absolute_frame();
    assert_eq!(
        frame,
        Rect::new(100.0, 100.0, 100.0, 100.0),
        "the reported frame matches the drawing"
    );

    assert_eq!(
        hit_id(&root, frame.max_x() - 0.5, frame.max_y() - 0.5),
        Some(panel.id().0)
    );
    assert_ne!(
        hit_id(&root, frame.max_x() + 0.5, frame.max_y() + 0.5),
        Some(panel.id().0),
        "the hit boundary is the reported frame to the pixel",
    );
}

#[test]
fn centre_anchor_moves_the_hit_area() {
    let panel = Node::new();
    let root = scene(|root| {
        panel.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        root.add_child(&panel);
        panel.set_transform(NodeTransform::anchored(0.5, 0.5, 0.5));
    });

    // The box shrinks to (150,150)-(250,250) around the unmoved centre.
    assert_eq!(
        hit_id(&root, 200.0, 200.0),
        Some(panel.id().0),
        "the centre still hits"
    );
    assert_ne!(
        hit_id(&root, 120.0, 120.0),
        Some(panel.id().0),
        "the vacated leading corner is dead"
    );
    assert_ne!(
        hit_id(&root, 280.0, 280.0),
        Some(panel.id().0),
        "the vacated trailing corner is dead"
    );
}

#[test]
fn child_inside_scaled_ancestor() {
    let group = Node::new();
    let child = Node::new();
    let root = scene(|root| {
        group.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        root.add_child(&group);
        child.set_frame(Rect::new(0.0, 0.0, 40.0, 40.0));
        group.add_child(&child);
        group.set_transform(NodeTransform::anchored(2.0, 0.0, 0.0));
    });

    // The child is drawn at (100,100)-(180,180) and must be clicked there.
    assert_eq!(hit_id(&root, 150.0, 150.0), Some(child.id().0));
    assert_ne!(
        hit_id(&root, 220.0, 220.0),
        Some(child.id().0),
        "beyond the scaled child"
    );
}

#[test]
fn scroll_then_scale() {
    let viewport = Node::new();
    let content = Node::new();
    let root = scene(|root| {
        viewport.set_frame(Rect::new(40.0, 20.0, 300.0, 200.0));
        root.add_child(&viewport);
        content.set_frame(Rect::new(0.0, 0.0, 300.0, 900.0));
        viewport.add_child(&content);
        viewport.set_content_offset(Point { x: 0.0, y: 100.0 });
        viewport.set_transform(NodeTransform::anchored(2.0, 0.0, 0.0));
    });

    // The viewport's content starts 200pt above it on screen, because the 100pt
    // of scroll is in content units and the box is drawn at 2×.
    assert_eq!(
        viewport.absolute_frame(),
        Rect::new(40.0, 20.0, 600.0, 400.0)
    );
    assert_eq!(content.absolute_origin(), Point { x: 40.0, y: -180.0 });
    assert_eq!(hit_id(&root, 40.0, 20.0), Some(content.id().0));
}

#[test]
fn collapsed_node_is_unhittable() {
    let panel = Node::new();
    let root = scene(|root| {
        panel.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        root.add_child(&panel);
        panel.set_transform(NodeTransform::anchored(0.0, 0.0, 0.0));
    });

    // A zero scale has no inverse, so a naive implementation falls back to the
    // identity and reports the node as full size — hittable where it is not drawn.
    assert_ne!(
        hit_id(&root, 150.0, 150.0),
        Some(panel.id().0),
        "the interior is dead"
    );
    assert_ne!(
        hit_id(&root, 100.0, 100.0),
        Some(panel.id().0),
        "even the origin is dead"
    );
}

#[test]
fn local_point_is_unscaled() {
    let panel = Node::new();
    let root = scene(|root| {
        panel.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        root.add_child(&panel);
        panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));
    });

    // Window (150, 150) is local (100, 100) — the middle of the box — even
    // though on screen that point sits halfway across the drawn 100pt.
    let hit = HitTester::hit_test(&root, Point { x: 150.0, y: 150.0 }).expect("the panel is hit");
    assert_eq!(hit.node.id(), panel.id());
    assert_eq!(
        hit.local_point,
        Point { x: 100.0, y: 100.0 },
        "scale does not leak into local space"
    );
}

#[test]
fn scaled_clip_still_clips() {
    let panel = Node::new();
    let child = Node::new();
    let root = scene(|root| {
        panel.set_frame(Rect::new(100.0, 100.0, 200.0, 200.0));
        panel.set_clips_to_bounds(true);
        root.add_child(&panel);
        // A child that pokes out past the parent's right edge.
        child.set_frame(Rect::new(180.0, 10.0, 100.0, 40.0));
        panel.add_child(&child);
        panel.set_transform(NodeTransform::anchored(0.5, 0.0, 0.0));
    });

    // Panel clips to (100,100)-(200,200); the child is drawn at
    // (190,105)-(240,125), so everything past x=200 is painted over by nothing
    // and must not be clickable.
    assert_eq!(child.absolute_frame(), Rect::new(190.0, 105.0, 50.0, 20.0));
    assert_eq!(
        hit_id(&root, 195.0, 115.0),
        Some(child.id().0),
        "inside the clip"
    );
    assert_ne!(
        hit_id(&root, 220.0, 115.0),
        Some(child.id().0),
        "the clipped-away half is dead"
    );
}
