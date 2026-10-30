//! Translation of `Tests/CoveUIRuntimeTests/HitTesterTests.swift`.
//!
//! Behaviour checklist:
//! - a point inside a leaf returns that leaf, with a root→leaf path
//! - a point outside everything returns `None`
//! - the last child wins (paint order)
//! - a non-hit-testable node is skipped but its children are still tested
//! - `clips_to_bounds` rejects child hits outside the frame
//! - the reported local point is in the hit node's coordinate space
//! - `content_offset` shifts child hit regions

use cove_core::scene::{HitTester, Node};
use cove_core::{Point, Rect};

fn make_node(frame: Rect, hit_testable: bool, clips: bool) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    node.set_is_hit_testable(hit_testable);
    node.set_clips_to_bounds(clips);
    node
}

fn plain(frame: Rect) -> Node {
    make_node(frame, true, false)
}

fn same(actual: Option<&Node>, expected: &Node) -> bool {
    actual.is_some_and(|node| node.id() == expected.id())
}

#[test]
fn leaf_hit() {
    let root = plain(Rect::new(0.0, 0.0, 100.0, 100.0));
    let leaf = plain(Rect::new(10.0, 10.0, 20.0, 20.0));
    root.add_child(&leaf);

    let result = HitTester::hit_test(&root, Point { x: 15.0, y: 15.0 }).expect("point is inside the leaf");
    assert!(same(Some(&result.node), &leaf), "the leaf is the deepest hit");
    assert_eq!(result.path.len(), 2, "path runs root -> leaf");
    assert!(same(result.path.first(), &root));
    assert!(same(result.path.last(), &leaf));
}

#[test]
fn miss() {
    let root = plain(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert!(
        HitTester::hit_test(&root, Point { x: 200.0, y: 200.0 }).is_none(),
        "a point outside the root hits nothing"
    );
}

#[test]
fn z_order() {
    let root = plain(Rect::new(0.0, 0.0, 100.0, 100.0));
    let bottom = plain(Rect::new(10.0, 10.0, 50.0, 50.0));
    let top = plain(Rect::new(10.0, 10.0, 50.0, 50.0));
    root.add_child(&bottom);
    root.add_child(&top);

    let result = HitTester::hit_test(&root, Point { x: 20.0, y: 20.0 }).expect("overlapping children");
    assert!(same(Some(&result.node), &top), "the last child paints on top");
}

#[test]
fn pass_through_parent() {
    let root = make_node(Rect::new(0.0, 0.0, 100.0, 100.0), false, false);
    let leaf = plain(Rect::new(10.0, 10.0, 20.0, 20.0));
    root.add_child(&leaf);

    let hit_on_leaf =
        HitTester::hit_test(&root, Point { x: 15.0, y: 15.0 }).expect("children of a pass-through still hit");
    assert!(same(Some(&hit_on_leaf.node), &leaf));

    // The point is inside the root but not the leaf, and the root does not
    // accept hits, so nothing is reported.
    assert!(
        HitTester::hit_test(&root, Point { x: 80.0, y: 80.0 }).is_none(),
        "a non-hit-testable root cannot itself be hit"
    );
}

#[test]
fn clipping() {
    let root = make_node(Rect::new(0.0, 0.0, 100.0, 100.0), true, true);
    // The child extends past the parent's right edge.
    let leaf = plain(Rect::new(80.0, 10.0, 80.0, 20.0));
    root.add_child(&leaf);

    // (150, 15) is inside the leaf in its own coordinates, but the parent
    // clips, so the whole subtree is rejected.
    assert!(
        HitTester::hit_test(&root, Point { x: 150.0, y: 15.0 }).is_none(),
        "clips_to_bounds rejects hits outside the frame"
    );
}

#[test]
fn local_coordinates() {
    let root = plain(Rect::new(0.0, 0.0, 200.0, 200.0));
    let leaf = plain(Rect::new(50.0, 60.0, 100.0, 100.0));
    root.add_child(&leaf);

    let result = HitTester::hit_test(&root, Point { x: 70.0, y: 90.0 }).expect("inside the leaf");
    assert!(same(Some(&result.node), &leaf));
    assert!(
        (result.local_point.x - 20.0).abs() < 1e-5 && (result.local_point.y - 30.0).abs() < 1e-5,
        "local point is (20, 30), got {:?}",
        result.local_point
    );
}

#[test]
fn content_offset_affects_hit_test() {
    let root = plain(Rect::new(0.0, 0.0, 200.0, 200.0));
    let child = plain(Rect::new(0.0, 30.0, 120.0, 30.0));
    root.add_child(&child);

    // Before scrolling, y=5 sits above the child.
    let before = HitTester::hit_test(&root, Point { x: 10.0, y: 5.0 });
    assert!(
        !before.as_ref().is_some_and(|result| result.node.id() == child.id()),
        "unscrolled, y=5 misses the child"
    );

    // Scrolling the content down by 30 moves the child to the top visually.
    root.set_content_offset(Point { x: 0.0, y: 30.0 });

    let scrolled = HitTester::hit_test(&root, Point { x: 10.0, y: 5.0 }).expect("the child moved up");
    assert!(same(Some(&scrolled.node), &child), "scrolled, y=5 hits the child");
    let past = HitTester::hit_test(&root, Point { x: 10.0, y: 35.0 });
    assert!(
        !past.as_ref().is_some_and(|result| result.node.id() == child.id()),
        "scrolled, y=35 is past the child"
    );
}
