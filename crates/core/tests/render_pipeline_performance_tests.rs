//! Translation of `Tests/Headless/Tests/CoveUISceneTests/RenderPipelinePerformanceTests.swift`.
//!
//! A scroll of a 1200-row painter-dense tree must keep producing geometry. The
//! Swift suite runs one case per corner radius (0 and 16) and only under
//! `COVE_RENDER_BENCHMARK=1`; the assertion it makes is that the frame is not
//! empty, which is exactly what a regression in retained-layer culling or the
//! rounded-clip path would break.
//!
//! The Rust port always runs the work and asserts the same invariant instead of
//! gating on an environment variable: `#[test]` cannot be conditionally
//! disabled without losing the assertion, and the cost is 90 frames over 1200
//! rows, which is milliseconds. The percentile print is kept for parity.
//!
//! Behaviour checklist:
//! - a 1200-row scrolled frame draws geometry for a square root
//! - the same tree draws geometry for a rounded root

use std::time::Instant;

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeTree, RenderTree};
use cove_core::{Color, DrawList, Point, Rect};

fn build(corner_radius: f32) -> (Node, Node) {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 1200.0, 800.0));
    root.set_clips_to_bounds(true);
    root.set_corner_radius(corner_radius);

    let mut container = Node::new();
    for row in 0..1200_usize {
        container = Node::new();
        container.set_frame(Rect::new(0.0, row as f32 * 24.0, 1200.0, 24.0));
        root.add_child(&container);

        let leaf = Node::new();
        leaf.set_frame(Rect::new(4.0, 2.0, 1160.0, 20.0));
        leaf.set_draw(Some(std::rc::Rc::new(
            |list: &mut DrawList, origin: Point| {
                for column in 0..8 {
                    let _ = list.add_rect(
                        Rect::new(origin.x + column as f32 * 100.0, origin.y, 90.0, 18.0),
                        Color::WHITE,
                    );
                }
            },
        )));
        container.add_child(&leaf);
    }
    (root, container)
}

fn scroll_frames(corner_radius: f32) -> (usize, Vec<f64>) {
    let (root, _last_container) = build(corner_radius);
    let mut node_tree = NodeTree::new();
    node_tree.set_root(Some(root.clone()));

    let tree = RenderTree::new();
    tree.install(&root);
    let mut renderer = LayerAwareNodeRenderer::new();
    let mut list = DrawList::default();

    let mut samples = Vec::new();
    for frame in 0..90 {
        root.set_content_offset(Point {
            x: 0.0,
            y: frame as f32 * 7.0,
        });
        list.reset();
        let start = Instant::now();
        renderer.render(&tree, &mut list);
        let elapsed = start.elapsed().as_secs_f64() * 1_000.0;
        if frame >= 30 {
            samples.push(elapsed);
        }
    }
    (list.indices().len(), samples)
}

#[test]
fn scrolling() {
    for corner_radius in [0.0_f32, 16.0] {
        let (indices, mut samples) = scroll_frames(corner_radius);
        assert!(
            indices > 0,
            "a scrolled 1200-row frame draws geometry at corner radius {corner_radius}",
        );
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = samples[samples.len() / 2];
        let p95 = samples[(samples.len() as f64 * 0.95) as usize];
        println!(
            "COVE_RENDER_BENCHMARK scrolling radius={corner_radius} p50={p50} ms p95={p95} ms"
        );
    }
}
