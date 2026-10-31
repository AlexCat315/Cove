//! Translation of `Tests/CoveUIRuntimeTests/Rendering/RevealRequestTests.swift`.
//!
//! The renderer has to carry a node's frame-level reveal all the way to the
//! frame's `DrawList`, including when the node sits inside a cached layer that
//! gets replayed instead of re-recorded.
//!
//! Behaviour checklist:
//! - the legacy walker raises the request onto the frame list
//! - an inline node's request reaches the frame list through the layer renderer
//! - a request survives a cached layer being replayed, and follows radius changes
//! - a cleared request does not inherit the previous frame's value
//! - a reveal is keyed by sequence, so two runs are distinguishable
//! - a zero-radius reveal is not visible
//! - `DrawList::reset` drops the pending request

use std::rc::Rc;

use cove_core::Reveal;
use cove_core::scene::{DrawHook, LayerAwareNodeRenderer, Node, NodeRenderer, RenderTree};
use cove_core::{Color, DrawList, Rect};

fn reveal(sequence: u64, radius: f32) -> Reveal {
    Reveal::new(sequence, 12.0, 34.0, radius, 0.0)
}

fn framed(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

/// `RenderObject` holds nodes weakly, so the test keeps every node alive.
fn scene() -> (Node, Node) {
    let root = framed(0.0, 0.0, 200.0, 120.0);
    let surface = framed(0.0, 0.0, 200.0, 120.0);
    root.add_child(&surface);
    (root, surface)
}

#[test]
fn direct_renderer() {
    let (root, surface) = scene();
    surface.request_reveal(Some(reveal(1, 40.0)));

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    assert_eq!(
        list.reveal,
        Some(reveal(1, 40.0)),
        "the legacy walker must put the node's request on the frame list"
    );
    drop((root, surface));
}

#[test]
fn inline_node() {
    let (root, surface) = scene();
    surface.request_reveal(Some(reveal(1, 40.0)));

    let tree = RenderTree::new();
    tree.install(&root);
    let mut list = DrawList::default();
    LayerAwareNodeRenderer::new().render(&tree, &mut list);

    assert_eq!(
        list.reveal,
        Some(reveal(1, 40.0)),
        "an inline node's request must reach the frame list"
    );
    drop((root, surface, tree));
}

#[test]
fn cached_layer() {
    let (root, surface) = scene();
    // A painter makes the node a layer root: that is the case which records into
    // its own cache and appends the cache instead of re-recording.
    let draw: DrawHook = Rc::new(|list: &mut DrawList, _origin: cove_core::Point| {
        list.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::WHITE);
    });
    surface.set_draw(Some(draw));

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();

    for radius in [40.0_f32, 90.0] {
        surface.request_reveal(Some(reveal(1, radius)));
        let mut list = DrawList::default();
        renderer.render(&tree, &mut list);
        assert_eq!(
            list.reveal,
            Some(reveal(1, radius)),
            "a replayed cached layer must still carry its reveal (radius={radius})"
        );
    }

    // A stable frame with no request must not inherit the previous one: a
    // replayed layer still carries whatever it last recorded.
    surface.request_reveal(None);
    let mut list = DrawList::default();
    renderer.render(&tree, &mut list);
    assert_eq!(
        list.reveal, None,
        "clearing the request must clear the frame, not replay the stale one"
    );
    drop((root, surface, tree));
}

#[test]
fn sequence_keys_the_run() {
    let first = reveal(1, 10.0);
    let second = reveal(2, 10.0);
    assert_ne!(
        first, second,
        "a different sequence is a different run, even at the same radius"
    );
    assert_eq!(first.sequence, 1);
    assert!(reveal(1, 40.0).is_visible(), "a positive radius is visible");
    assert!(
        !reveal(1, 0.0).is_visible(),
        "a zero radius has nothing to reveal"
    );
}

#[test]
fn reset_drops_the_request() {
    let mut list = DrawList::default();
    list.begin_reveal(reveal(1, 40.0));
    assert_eq!(
        list.reveal,
        Some(reveal(1, 40.0)),
        "begin_reveal sets the frame's request"
    );
    list.reset();
    assert_eq!(
        list.reveal, None,
        "a reset frame must carry no request"
    );
}
