//! Translation of `Tests/CoveUIRuntimeTests/NodeRendererTests.swift`.
//!
//! Behaviour checklist:
//! - background fills emit one quad per node and share a batch
//! - a frosted node attaches its backdrop to its own fill, covering the whole node
//! - a frosted panel's border is drawn in the backdrop batch, after the fill
//! - a backdrop with no fill leaves nothing behind
//! - child frames are translated by the parent origin
//! - `clips_to_bounds` pushes a scissor rect for the subtree
//! - the custom draw callback fires after the background
//! - `opacity` multiplies the background alpha
//! - `content_offset` translates children but not the parent's clip
//! - `corner_radius` switches the fill to a rounded-rect path; zero keeps it plain

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{DrawHook, LayerAwareNodeRenderer, Node, NodeRenderer, RenderTree};
use cove_core::{Backdrop, Color, DrawList, Point, Rect};

fn alpha(vertex: &cove_core::Vertex) -> u32 {
    (vertex.color >> 24) & 0xFF
}

fn framed(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

#[test]
fn background_fills_emit_quads() {
    let root = framed(0.0, 0.0, 200.0, 100.0);
    root.set_background_color(Some(Color::WHITE));
    let child = framed(10.0, 20.0, 50.0, 30.0);
    child.set_background_color(Some(Color::from_rgba8(255, 0, 0, 255)));
    root.add_child(&child);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    assert_eq!(list.vertices().len(), 8, "two filled quads");
    assert_eq!(list.indices().len(), 12);
    assert_eq!(
        list.batches().len(),
        1,
        "a shared texture id merges the draws"
    );
    assert_eq!(list.batches()[0].texture_id, 0);
}

#[test]
fn backdrop_rides_on_the_fill() {
    let root = framed(0.0, 0.0, 200.0, 100.0);
    root.set_background_color(Some(Color::WHITE));
    let panel = framed(20.0, 20.0, 80.0, 40.0);
    panel.set_corner_radius(6.0);
    panel.set_backdrop_blur(12.0);
    panel.set_background_color(Some(Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.84,
    }));
    root.add_child(&panel);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    // The panel cannot merge into the container behind it: the blur has to land
    // between the two, which only a batch boundary can express.
    assert_eq!(list.batches().len(), 2);
    assert!(
        list.batches()[0].backdrop.is_none(),
        "the container keeps a plain batch"
    );
    // The backdrop covers the whole node, not just the fill, so the border ring
    // drawn over it stays crisp.
    assert_eq!(
        list.batches()[1].backdrop,
        Some(Backdrop {
            rect: Rect::new(20.0, 20.0, 80.0, 40.0),
            corner_radius: 6.0,
            blur_radius: 12.0,
            opacity: 1.0,
        }),
    );
}

#[test]
fn frosted_border_stays_on_top() {
    for layered in [false, true] {
        let root = framed(0.0, 0.0, 120.0, 80.0);
        root.set_background_color(Some(Color {
            r: 0.2,
            g: 0.2,
            b: 0.2,
            a: 1.0,
        }));
        let panel = framed(20.0, 20.0, 80.0, 40.0);
        panel.set_corner_radius(6.0);
        panel.set_backdrop_blur(12.0);
        panel.set_background_color(Some(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.5,
        }));
        panel.set_border_color(Some(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.1,
        }));
        panel.set_border_width(1.0);
        root.add_child(&panel);

        let mut list = DrawList::default();
        if layered {
            let tree = RenderTree::new();
            tree.install(&root);
            LayerAwareNodeRenderer::new().render(&tree, &mut list);
        } else {
            NodeRenderer.render(&root, &mut list);
        }

        let frosted = list
            .batches()
            .iter()
            .position(|batch| batch.backdrop.is_some())
            .expect("a backdrop batch exists");
        // The ring's translucent black lands in the composited batch, so it is
        // painted over the frost rather than under it.
        let ring_color = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.1,
        }
        .rgba8();
        let ring_vertices: Vec<usize> = list
            .vertices()
            .iter()
            .enumerate()
            .filter(|(_, vertex)| vertex.color == ring_color)
            .map(|(index, _)| index)
            .collect();
        let start = list.batches()[frosted].index_offset as usize;
        let first_ring = list
            .indices()
            .iter()
            .position(|index| ring_vertices.contains(&(*index as usize)));
        assert!(
            first_ring.is_some_and(|position| position >= start),
            "layered {layered}: the border ring is painted in the frosted batch",
        );
    }
}

#[test]
fn backdrop_without_fill_is_dropped() {
    let root = framed(0.0, 0.0, 100.0, 100.0);
    root.set_backdrop_blur(12.0);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);
    assert!(
        list.batches().is_empty(),
        "a backdrop with nothing to composite emits nothing"
    );
}

#[test]
fn child_frames_accumulate_origin() {
    let root = framed(5.0, 7.0, 100.0, 100.0);
    let child = framed(10.0, 10.0, 20.0, 20.0);
    child.set_background_color(Some(Color::WHITE));
    root.add_child(&child);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    assert_eq!(
        list.vertices()[0].x,
        15.0,
        "the child origin accumulates onto the parent's"
    );
    assert_eq!(list.vertices()[0].y, 17.0);
}

#[test]
fn clips_to_bounds_pushes_scissor() {
    let root = framed(0.0, 0.0, 100.0, 100.0);
    root.set_clips_to_bounds(true);
    let child = framed(0.0, 0.0, 10.0, 10.0);
    child.set_background_color(Some(Color::WHITE));
    root.add_child(&child);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    assert_eq!(
        list.batches()[0].scissor,
        Some(Rect::new(0.0, 0.0, 100.0, 100.0)),
        "the subtree carries the root's scissor",
    );
}

#[test]
fn draw_callback_fires() {
    let root = framed(0.0, 0.0, 50.0, 50.0);
    root.set_background_color(Some(Color::from_rgba8(0, 0, 255, 255)));
    let observed = Rc::new(RefCell::new(None));
    let sink = observed.clone();
    root.set_draw(Some(Rc::new(move |_: &mut DrawList, origin: Point| {
        *sink.borrow_mut() = Some(origin);
    }) as DrawHook));

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    assert_eq!(
        *observed.borrow(),
        Some(Point { x: 0.0, y: 0.0 }),
        "the callback sees the node origin"
    );
    assert_eq!(list.vertices().len(), 4, "the background still emits");
}

#[test]
fn opacity_multiplies_alpha() {
    let root = framed(0.0, 0.0, 10.0, 10.0);
    root.set_background_color(Some(Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }));
    root.set_opacity(0.5);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    let packed_alpha = alpha(&list.vertices()[0]);
    assert!(
        (126..=129).contains(&packed_alpha),
        "a 0.5 opacity lands near 127, not {packed_alpha}",
    );
}

#[test]
fn content_offset_translates_children() {
    let root = framed(0.0, 0.0, 100.0, 100.0);
    root.set_clips_to_bounds(true);
    root.set_content_offset(Point { x: 0.0, y: 30.0 });
    let child = framed(0.0, 0.0, 50.0, 50.0);
    child.set_background_color(Some(Color::WHITE));
    root.add_child(&child);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    assert_eq!(
        list.batches()[0].scissor,
        Some(Rect::new(0.0, 0.0, 100.0, 100.0)),
        "the clip stays at the root frame",
    );
    assert_eq!(
        list.vertices()[0].y,
        -30.0,
        "the child is translated by the content offset"
    );
}

#[test]
fn corner_radius_emits_rounded_rect() {
    let root = framed(0.0, 0.0, 100.0, 100.0);
    root.set_background_color(Some(Color::WHITE));
    root.set_corner_radius(12.0);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);

    // A plain rect emits 4 vertices; a rounded rect decomposes into many more
    // (centre quad + edges + per-corner triangle fans).
    assert!(
        list.vertices().len() > 4,
        "the rounded path emits extra geometry"
    );
    assert_eq!(list.batches().len(), 1);
    assert_eq!(list.batches()[0].texture_id, 0);
}

#[test]
fn corner_radius_zero_is_plain() {
    let root = framed(0.0, 0.0, 100.0, 100.0);
    root.set_background_color(Some(Color::WHITE));
    root.set_corner_radius(0.0);

    let mut list = DrawList::default();
    NodeRenderer.render(&root, &mut list);
    assert_eq!(
        list.vertices().len(),
        4,
        "a zero radius keeps the plain quad"
    );
}
