//! Translation of `Tests/CoveUIRuntimeTests/GeometryFunnelReproTests.swift`.
//!
//! Hit-testing walks the live `Node` tree every time, so there is no cached
//! classification to go stale: a geometry or hit-classification change made the
//! way the layout pass and runtime make it — a plain property write — is
//! reflected on the very next hit test, with nothing to invalidate.
//!
//! Behaviour checklist:
//! - toggling `clips_to_bounds` on a parent immediately gates hit-testing of
//!   children that overflow its frame
//! - flipping `is_hit_testable` to false lets the point fall through to the parent
//! - a pure layout move is reflected immediately

use cove_core::scene::{HitTester, Node};
use cove_core::{Point, Rect};

fn hit_id(root: &Node, x: f32, y: f32) -> Option<u64> {
    HitTester::hit_test(root, Point { x, y }).map(|hit| hit.node.id().0)
}

#[test]
fn clips_to_bounds_toggles_hit_testing() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let p = Node::new();
    p.set_frame(Rect::new(50.0, 50.0, 40.0, 40.0));
    // `c` overflows `p`: in p-local it spans 20..60 but p is only 40 wide.
    let c = Node::new();
    c.set_frame(Rect::new(20.0, 20.0, 40.0, 40.0));
    root.add_child(&p);
    p.add_child(&c);

    // Point over the overflowing part of `c` (window 100,100): inside `c`
    // (window 70..110) but outside `p` (window 50..90).
    assert_eq!(hit_id(&root, 100.0, 100.0), Some(c.id().0), "the overflowing child is hit");

    p.set_clips_to_bounds(true);

    let after = hit_id(&root, 100.0, 100.0);
    assert_ne!(after, Some(c.id().0), "clipping removes the child from the hit path");
    assert_eq!(after, Some(root.id().0), "the point falls through to the root");
}

#[test]
fn is_hit_testable_flip_updates_hit_testing() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let a = Node::new();
    a.set_frame(Rect::new(10.0, 10.0, 80.0, 80.0));
    let b = Node::new();
    b.set_frame(Rect::new(5.0, 5.0, 30.0, 30.0));
    root.add_child(&a);
    a.add_child(&b);

    assert_eq!(hit_id(&root, 20.0, 20.0), Some(b.id().0), "the child is hit first");

    b.set_is_hit_testable(false);

    assert_eq!(hit_id(&root, 20.0, 20.0), Some(a.id().0), "the point falls through to the parent");
}

#[test]
fn frame_move_updates_hit_testing() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let a = Node::new();
    a.set_frame(Rect::new(10.0, 10.0, 80.0, 80.0));
    let b = Node::new();
    b.set_frame(Rect::new(5.0, 5.0, 30.0, 30.0));
    root.add_child(&a);
    a.add_child(&b);

    assert_eq!(hit_id(&root, 20.0, 20.0), Some(b.id().0), "the child is hit before the move");

    b.set_frame(Rect::new(100.0, 100.0, 30.0, 30.0));

    assert_eq!(hit_id(&root, 20.0, 20.0), Some(a.id().0), "a pure layout move reflows immediately");
}
