//! Translation of `Tests/CoveUIRuntimeTests/ContainerOpacityTests.swift`.
//!
//! Behaviour checklist:
//! - nested container opacity multiplies through to descendants
//! - the direct renderer and the cached layer-aware renderer agree, repeatedly
//! - raising an ancestor's opacity back to 1 reflects on the next cached frame

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, RenderTree};
use cove_core::{Color, DrawList, Rect};

/// Premultiplied alpha for `0.5 * 0.5` at 8 bits per channel.
const QUARTER: u32 = 64;
/// Premultiplied alpha for a single `0.5` container at 8 bits per channel.
const HALF: u32 = 128;

fn alpha(vertex: &cove_core::Vertex) -> u32 {
    vertex.color >> 24
}

#[test]
fn descendant_opacity() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_opacity(0.5);
    let child = Node::new();
    child.set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));
    child.set_background_color(Some(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }));
    child.set_opacity(0.5);
    root.add_child(&child);

    let mut direct = DrawList::default();
    NodeRenderer.render(&root, &mut direct);
    assert!(!direct.vertices().is_empty(), "the red child produced geometry");
    assert!(
        direct.vertices().iter().all(|vertex| alpha(vertex) == QUARTER),
        "nested 0.5 * 0.5 composes to a quarter alpha, saw {:?}",
        direct.vertices().iter().map(alpha).collect::<Vec<_>>()
    );

    let render_tree = RenderTree::new();
    render_tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    // The cached path must agree with the direct one on every frame, including
    // the first: caching must not change what is drawn.
    for frame in 0..3 {
        let mut cached = DrawList::default();
        renderer.render(&render_tree, &mut cached);
        assert_eq!(
            cached.vertices().iter().map(alpha).collect::<Vec<_>>(),
            direct.vertices().iter().map(alpha).collect::<Vec<_>>(),
            "cached frame {frame} matches the direct render"
        );
    }

    root.set_opacity(1.0);
    let mut changed = DrawList::default();
    renderer.render(&render_tree, &mut changed);
    assert!(
        changed.vertices().iter().all(|vertex| alpha(vertex) == HALF),
        "removing the ancestor's 0.5 raises descendants to a half alpha"
    );
}
