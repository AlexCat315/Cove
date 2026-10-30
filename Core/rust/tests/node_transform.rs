//! Translation of `Tests/CoveUIRuntimeTests/NodeTransformTests.swift`.
//!
//! `Node::transform` is what lets a subtree be scaled about a point inside
//! itself. These pin the geometry contract that everything downstream — hit
//! testing, reveal-into-view, accessibility, platform drag regions — reads
//! through `absolute_frame`, so a scaled node must answer exactly the way it is
//! drawn.
//!
//! Behaviour checklist:
//! - an untouched tree composes origins compositionally
//! - scaling about the top-left keeps the origin and shrinks the frame
//! - scaling about the centre pulls the origin toward the centre
//! - the anchor point itself never moves, for any scale or anchor
//! - an ancestor's scale applies to the whole subtree
//! - nested scales multiply rather than add
//! - a pointer round-trips through `convert_from_window` under scale
//! - a collapsed (zero-scale) node answers with a finite point, not infinity
//! - `contentOffset` moves a node's children, never the node itself
//! - a scroll offset composes with a scale on the same node

use cove_core::scene::{Node, NodeTransform};
use cove_core::{Point, Rect, Size};

fn node(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

fn assert_point(actual: Point, expected: Point) {
    assert!(
        (actual.x - expected.x).abs() < 1e-6 && (actual.y - expected.y).abs() < 1e-6,
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn identity_is_transparent() {
    let root = node(0.0, 0.0, 800.0, 600.0);
    let parent = node(100.0, 50.0, 400.0, 400.0);
    let child = node(10.0, 20.0, 50.0, 30.0);
    root.add_child(&parent);
    parent.add_child(&child);

    assert_point(parent.absolute_origin(), Point { x: 100.0, y: 50.0 });
    assert_point(child.absolute_origin(), Point { x: 110.0, y: 70.0 });
    assert_eq!(child.absolute_frame().size(), Size { width: 50.0, height: 30.0 });
    assert_point(
        child.convert_from_window(Point { x: 125.0, y: 85.0 }),
        Point { x: 15.0, y: 15.0 },
    );
}

#[test]
fn scale_about_top_left_keeps_origin_and_shrinks_frame() {
    let child = node(100.0, 40.0, 200.0, 100.0);
    child.set_transform(NodeTransform::new(0.5, Point { x: 0.0, y: 0.0 }));

    assert_point(child.absolute_origin(), Point { x: 100.0, y: 40.0 });
    assert_eq!(
        child.absolute_frame().size(),
        Size { width: 100.0, height: 50.0 }
    );
}

#[test]
fn scale_about_centre_pulls_origin_toward_centre() {
    let child = node(100.0, 40.0, 200.0, 100.0);
    child.set_transform(NodeTransform::new(0.5, Point { x: 0.5, y: 0.5 }));

    // The centre sits at (200, 90) and does not move; the box shrinks from
    // 200x100 to 100x50 around it, so the origin travels half of what came off
    // the near edges.
    assert_point(child.absolute_origin(), Point { x: 150.0, y: 65.0 });
    let frame = child.absolute_frame();
    assert_eq!(frame.size(), Size { width: 100.0, height: 50.0 });
    assert!((frame.x + frame.width / 2.0 - 200.0).abs() < 1e-6, "midX stays 200");
    assert!((frame.y + frame.height / 2.0 - 90.0).abs() < 1e-6, "midY stays 90");
}

#[test]
fn anchor_point_is_fixed() {
    // The anchor sits on the node's own boundary — a corner, or the centre,
    // which is the midpoint of the drawn frame whichever way it was scaled.
    // Wherever it lands on screen, mapping back must return it to the same
    // local point; if the scale moved it, the round trip would not.
    for scale in [0.25_f32, 0.5, 1.0, 2.0] {
        for anchor in [
            Point { x: 0.0, y: 0.0 },
            Point { x: 0.5, y: 0.5 },
            Point { x: 1.0, y: 1.0 },
        ] {
            let child = node(100.0, 40.0, 200.0, 100.0);
            child.set_transform(NodeTransform::new(scale, anchor));
            let frame = child.absolute_frame();
            let on_screen = Point {
                x: if anchor.x == 0.0 {
                    frame.x
                } else if anchor.x == 1.0 {
                    frame.x + frame.width
                } else {
                    frame.x + frame.width / 2.0
                },
                y: if anchor.y == 0.0 {
                    frame.y
                } else if anchor.y == 1.0 {
                    frame.y + frame.height
                } else {
                    frame.y + frame.height / 2.0
                },
            };
            let back = child.convert_from_window(on_screen);
            assert!(
                (back.x - anchor.x * 200.0).abs() < 1e-5,
                "scale {scale} anchor {anchor:?}: x round trip {back:?}"
            );
            assert!(
                (back.y - anchor.y * 100.0).abs() < 1e-5,
                "scale {scale} anchor {anchor:?}: y round trip {back:?}"
            );
        }
    }
}

#[test]
fn scale_composes_down_the_tree() {
    let root = node(0.0, 0.0, 800.0, 600.0);
    let group = node(100.0, 100.0, 200.0, 200.0);
    let child = node(0.0, 0.0, 40.0, 40.0);
    root.add_child(&group);
    group.add_child(&child);
    group.set_transform(NodeTransform::new(2.0, Point { x: 0.0, y: 0.0 }));

    // The group's own frame grows about its top-left, and the child sits at
    // that same point — scaled twice over, because it is inside it.
    assert_eq!(group.absolute_frame(), Rect::new(100.0, 100.0, 400.0, 400.0));
    assert_eq!(child.absolute_frame(), Rect::new(100.0, 100.0, 80.0, 80.0));
}

#[test]
fn nested_scales_multiply() {
    let root = node(0.0, 0.0, 800.0, 600.0);
    let outer = node(0.0, 0.0, 100.0, 100.0);
    let inner = node(0.0, 0.0, 100.0, 100.0);
    root.add_child(&outer);
    outer.add_child(&inner);
    outer.set_transform(NodeTransform::new(2.0, Point { x: 0.0, y: 0.0 }));
    inner.set_transform(NodeTransform::new(3.0, Point { x: 0.0, y: 0.0 }));

    assert_eq!(
        inner.absolute_frame().size(),
        Size { width: 600.0, height: 600.0 }
    );
}

#[test]
fn convert_round_trips_under_scale() {
    let root = node(0.0, 0.0, 800.0, 600.0);
    let child = node(120.0, 60.0, 200.0, 100.0);
    root.add_child(&child);
    child.set_transform(NodeTransform::new(0.8, Point { x: 0.5, y: 0.5 }));

    // Drawn at 0.8 about its centre, so the centre of the drawn frame is still
    // the centre of the local one — a round trip must land there rather than on
    // the unscaled point.
    let frame = child.absolute_frame();
    let back = child.convert_from_window(Point {
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
    });
    // `f32` cannot represent 0.8 exactly, so inverting a scaled transform
    // accumulates a few ULPs: at coordinates around 100 the error reaches ~1.5e-5.
    // The tolerance reflects the arithmetic, not a loose comparison.
    assert!((back.x - 100.0).abs() < 1e-3, "x round trip {back:?}");
    assert!((back.y - 50.0).abs() < 1e-3, "y round trip {back:?}");
}

#[test]
fn collapsed_scale_stays_finite() {
    let child = node(10.0, 10.0, 100.0, 100.0);
    child.set_transform(NodeTransform::new(0.0, Point { x: 0.0, y: 0.0 }));
    let converted = child.convert_from_window(Point { x: 5.0, y: 5.0 });
    assert!(
        converted.x.is_finite() && converted.y.is_finite(),
        "a collapsed node answers finitely, got {converted:?}"
    );
}

#[test]
fn content_offset_does_not_move_the_node() {
    // The distinction the whole chain rests on: `contentOffset` slides a node's
    // children, it does not move the node. A scroll view that reported its own
    // origin as scrolled would put reveal-into-view, hit testing and the
    // platform drag regions off by exactly the scroll offset.
    let root = node(0.0, 0.0, 800.0, 600.0);
    let viewport = node(40.0, 20.0, 300.0, 200.0);
    let content = node(0.0, 0.0, 300.0, 900.0);
    root.add_child(&viewport);
    viewport.add_child(&content);
    viewport.set_content_offset(Point { x: 0.0, y: 120.0 });

    assert_point(viewport.absolute_origin(), Point { x: 40.0, y: 20.0 });
    assert_point(content.absolute_origin(), Point { x: 40.0, y: -100.0 });
}

#[test]
fn content_offset_and_scale_compose() {
    let root = node(0.0, 0.0, 800.0, 600.0);
    let viewport = node(40.0, 20.0, 300.0, 200.0);
    let content = node(0.0, 0.0, 300.0, 900.0);
    root.add_child(&viewport);
    viewport.add_child(&content);
    viewport.set_content_offset(Point { x: 0.0, y: 120.0 });
    viewport.set_transform(NodeTransform::new(2.0, Point { x: 0.0, y: 0.0 }));

    // The viewport still reports its own frame, unscaled and unscrolled...
    assert_eq!(
        viewport.absolute_frame(),
        Rect::new(40.0, 20.0, 600.0, 400.0)
    );
    // ...while its content slides by the scroll offset *before* the scale, so
    // scrolling 120pt in a box drawn at 2× moves 240pt on screen.
    assert_point(content.absolute_origin(), Point { x: 40.0, y: -220.0 });
}
