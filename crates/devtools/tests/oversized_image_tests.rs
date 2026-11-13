//! Oversized-image highlighting from real decoded-versus-display pixel sizes.
//!
//! The overlay may only flag an image when the image primitive recorded a
//! decoded source extent and the display rect it actually painted. These tests
//! pin the threshold boundary, vector/unknown/invalid/zero-display skipping,
//! root and ancestor clipping, the cap, the toggle, and independence from the
//! guidelines, baselines and selection layers.

use cove_core::scene::{Node, NodeTree, record_image_pixel_metrics};
use cove_core::{Color, DrawList, Rect};
use cove_devtools::inspection::MAX_OVERSIZED_IMAGES;
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const OVERSIZED: Color = Color {
    r: 1.0,
    g: 45.0 / 255.0,
    b: 120.0 / 255.0,
    a: 230.0 / 255.0,
};
const SELECTION: Color = Color {
    r: 80.0 / 255.0,
    g: 160.0 / 255.0,
    b: 1.0,
    a: 1.0,
};

fn probe_from(root: Node) -> RuntimeProbe {
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(root));
    RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None)
}

fn node_id(node: &Node) -> String {
    node.id().0.to_string()
}

fn overlay(probe: &RuntimeProbe) -> DrawList {
    let mut list = DrawList::default();
    probe.scene.editor.draw_overlay(&mut list);
    list
}

/// Every `add_rect` appends one quad, so each group of four vertices is a rect.
fn rects(list: &DrawList) -> Vec<Rect> {
    list.vertices()
        .chunks(4)
        .map(|quad| {
            let min = |value: fn(&cove_core::Vertex) -> f32| {
                quad.iter().map(value).fold(f32::INFINITY, f32::min)
            };
            let max = |value: fn(&cove_core::Vertex) -> f32| {
                quad.iter().map(value).fold(f32::NEG_INFINITY, f32::max)
            };
            Rect::new(
                min(|v| v.x),
                min(|v| v.y),
                max(|v| v.x) - min(|v| v.x),
                max(|v| v.y) - min(|v| v.y),
            )
        })
        .collect()
}

fn tree_root() -> Node {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    root
}

/// A node carrying the same metrics an image paint records.
fn image_node(frame: Rect, source: (f32, f32), display: (f32, f32), vector: bool) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    record_image_pixel_metrics(&node, source, display, vector);
    node
}

fn oversized_node(frame: Rect) -> Node {
    image_node(frame, (800.0, 600.0), (100.0, 75.0), false)
}

fn enable(probe: &mut RuntimeProbe) {
    let applied = probe
        .execute(ProbeCommand::OversizedImages { enabled: true })
        .expect("the probe accepts the toggle");
    assert_eq!(applied, serde_json::json!({"enabled": true}));
}

#[test]
fn enabling_outlines_real_oversized_images_and_disabling_removes_them() {
    let root = tree_root();
    let image = oversized_node(Rect::new(10.0, 20.0, 100.0, 50.0));
    root.add_child(&image);
    let mut probe = probe_from(root);

    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabled draws nothing"
    );

    enable(&mut probe);
    assert!(probe.scene.editor.oversized_images());

    let list = overlay(&probe);
    assert_eq!(
        rects(&list),
        vec![
            Rect::new(10.0, 20.0, 100.0, 2.0),
            Rect::new(10.0, 68.0, 100.0, 2.0),
            Rect::new(10.0, 20.0, 2.0, 50.0),
            Rect::new(108.0, 20.0, 2.0, 50.0),
        ],
        "a two-pixel border is four quads inside the image frame"
    );
    assert!(
        list.vertices()
            .iter()
            .all(|vertex| vertex.color == OVERSIZED.rgba8()),
        "the oversized outline has its own color"
    );
    assert_eq!(list.current_clip(), None, "clip stack is balanced");

    let applied = probe
        .execute(ProbeCommand::OversizedImages { enabled: false })
        .expect("the probe accepts disabling");
    assert_eq!(applied, serde_json::json!({"enabled": false}));
    assert!(!probe.scene.editor.oversized_images());
    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabling removes every outline"
    );
}

#[test]
fn only_images_above_the_area_threshold_are_outlined() {
    let root = tree_root();
    // Exactly four times is the boundary and must not be flagged.
    let boundary = image_node(
        Rect::new(0.0, 0.0, 100.0, 50.0),
        (400.0, 200.0),
        (200.0, 100.0),
        false,
    );
    // Just above four times is flagged.
    let above = image_node(
        Rect::new(150.0, 0.0, 100.0, 50.0),
        (400.5, 200.0),
        (200.0, 100.0),
        false,
    );
    root.add_child(&boundary);
    root.add_child(&above);
    let mut probe = probe_from(root);
    enable(&mut probe);

    let drawn = rects(&overlay(&probe));
    assert_eq!(drawn.len(), 4, "only one image is above the threshold");
    assert_eq!(
        drawn[0],
        Rect::new(150.0, 0.0, 100.0, 2.0),
        "the outline belongs to the above-threshold image"
    );
}

#[test]
fn vector_unknown_invalid_and_zero_display_nodes_are_skipped() {
    let root = tree_root();
    let control = oversized_node(Rect::new(0.0, 0.0, 50.0, 50.0));

    let vector = image_node(
        Rect::new(60.0, 0.0, 50.0, 50.0),
        (4000.0, 4000.0),
        (50.0, 50.0),
        true,
    );
    let unknown = Node::new();
    unknown.set_frame(Rect::new(120.0, 0.0, 50.0, 50.0));
    let zero_display = image_node(
        Rect::new(180.0, 0.0, 50.0, 50.0),
        (4000.0, 4000.0),
        (0.0, 50.0),
        false,
    );
    let negative_display = image_node(
        Rect::new(240.0, 0.0, 50.0, 50.0),
        (4000.0, 4000.0),
        (-50.0, 50.0),
        false,
    );
    let non_finite = image_node(
        Rect::new(0.0, 60.0, 50.0, 50.0),
        (4000.0, 4000.0),
        (f32::NAN, 50.0),
        false,
    );
    let zero_source = image_node(
        Rect::new(60.0, 60.0, 50.0, 50.0),
        (0.0, 0.0),
        (50.0, 50.0),
        false,
    );
    let zero_frame = oversized_node(Rect::new(120.0, 60.0, 0.0, 50.0));

    let hidden = oversized_node(Rect::new(180.0, 60.0, 50.0, 50.0));
    hidden.set_layout_visibility(false);
    let transparent = oversized_node(Rect::new(240.0, 60.0, 50.0, 50.0));
    transparent.set_opacity(0.0);
    let detached = oversized_node(Rect::new(0.0, 120.0, 50.0, 50.0));

    for node in [
        &control,
        &vector,
        &unknown,
        &zero_display,
        &negative_display,
        &non_finite,
        &zero_source,
        &zero_frame,
        &hidden,
        &transparent,
    ] {
        root.add_child(node);
    }
    root.add_child(&detached);
    root.remove_child(&detached);

    let mut probe = probe_from(root);
    enable(&mut probe);
    assert_eq!(
        rects(&overlay(&probe)).len(),
        4,
        "only the control image is flagged; vector, unknown, invalid and zero-display nodes are not"
    );
}

#[test]
fn outlines_are_clipped_by_the_root_and_by_clipping_ancestors() {
    let root = tree_root();
    let scroller = Node::new();
    scroller.set_frame(Rect::new(10.0, 20.0, 200.0, 100.0));
    scroller.set_clips_to_bounds(true);
    let inside = oversized_node(Rect::new(5.0, 5.0, 50.0, 20.0));
    scroller.add_child(&inside);
    // Extends past the root's right edge and must stop at the viewport.
    let outside = oversized_node(Rect::new(350.0, 0.0, 100.0, 50.0));
    root.add_child(&scroller);
    root.add_child(&outside);

    let mut probe = probe_from(root);
    enable(&mut probe);
    let list = overlay(&probe);
    assert_eq!(rects(&list).len(), 8, "both images are outlined");
    let scroller_frame = Rect::new(10.0, 20.0, 200.0, 100.0);
    assert!(
        list.batches()
            .iter()
            .any(|batch| batch.scissor == Some(scroller_frame)),
        "the image inside a clipping scroller is clipped to that scroller"
    );
    let root_frame = Rect::new(0.0, 0.0, 400.0, 300.0);
    assert!(
        list.batches()
            .iter()
            .any(|batch| batch.scissor == Some(root_frame)),
        "an unclipped outline is still clipped to the root viewport"
    );
    assert_eq!(list.current_clip(), None, "clip stack is balanced");
}

#[test]
fn outlines_are_capped_across_the_whole_tree() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 10_000.0, 10_000.0));
    for index in 0..MAX_OVERSIZED_IMAGES + 50 {
        let x = (index % 100) as f32 * 100.0;
        let y = (index / 100) as f32 * 100.0;
        root.add_child(&oversized_node(Rect::new(x, y, 90.0, 90.0)));
    }
    let mut probe = probe_from(root);
    enable(&mut probe);
    assert_eq!(
        rects(&overlay(&probe)).len(),
        MAX_OVERSIZED_IMAGES * 4,
        "the overlay emits at most {MAX_OVERSIZED_IMAGES} image outlines"
    );
}

#[test]
fn toggling_redraws_only_on_a_real_change_and_disconnect_clears_it() {
    let root = tree_root();
    root.add_child(&oversized_node(Rect::new(10.0, 20.0, 100.0, 50.0)));
    let mut probe = probe_from(root);
    let redraws = Rc::new(Cell::new(0));
    {
        let redraws = redraws.clone();
        *probe.scene.editor.on_change.borrow_mut() =
            Some(Rc::new(move || redraws.set(redraws.get() + 1)));
    }
    enable(&mut probe);
    assert_eq!(redraws.get(), 1);
    probe
        .execute(ProbeCommand::OversizedImages { enabled: true })
        .expect("the probe accepts a repeated toggle");
    assert_eq!(redraws.get(), 1, "an unchanged state schedules no redraw");
    probe.disconnect();
    assert_eq!(redraws.get(), 2, "disconnect clears oversized images");
    assert!(!probe.scene.editor.oversized_images());
    assert!(overlay(&probe).vertices().is_empty());
}

#[test]
fn the_overlay_is_independent_of_guidelines_baselines_and_selection() {
    let root = tree_root();
    let left = Node::new();
    left.set_frame(Rect::new(0.0, 0.0, 200.0, 300.0));
    let right = Node::new();
    right.set_frame(Rect::new(200.0, 0.0, 200.0, 300.0));
    let image = oversized_node(Rect::new(210.0, 20.0, 100.0, 50.0));
    right.add_child(&image);
    root.add_child(&left);
    root.add_child(&right);

    let mut probe = probe_from(root);
    assert!(
        probe
            .execute(ProbeCommand::Select {
                node_id: Some(node_id(&right)),
            })
            .is_ok()
    );
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .expect("guidelines toggle");
    enable(&mut probe);

    let list = overlay(&probe);
    assert_eq!(
        rects(&list).len(),
        4 * 4 + 4 + 4,
        "four guideline quads per visible node, one image outline and one selection border"
    );
    assert_eq!(
        list.vertices()
            .iter()
            .filter(|vertex| vertex.color == OVERSIZED.rgba8())
            .count(),
        16,
        "the image outline is exactly four quads"
    );
    let selection: Vec<_> = list
        .vertices()
        .iter()
        .filter(|vertex| vertex.color == SELECTION.rgba8())
        .collect();
    assert_eq!(
        selection.len(),
        16,
        "the selection border is still four quads"
    );
    assert!(probe.scene.editor.debug_paint().guidelines);
    assert!(!probe.scene.editor.baselines());

    probe
        .execute(ProbeCommand::OversizedImages { enabled: false })
        .expect("oversized images off");
    let without = overlay(&probe);
    assert_eq!(
        rects(&without).len(),
        4 * 4 + 4,
        "turning images off leaves guidelines and selection untouched"
    );
    assert!(probe.scene.editor.debug_paint().guidelines);
    assert!(
        probe.scene.editor.selected_node().is_some(),
        "selection is untouched"
    );

    probe
        .execute(ProbeCommand::DebugPaint { guidelines: false })
        .expect("guidelines off");
    assert!(
        !probe.scene.editor.oversized_images(),
        "guidelines cannot change the oversized-image state"
    );
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .expect("baselines on");
    assert!(
        !probe.scene.editor.oversized_images(),
        "baselines cannot change the oversized-image state"
    );
}

#[test]
fn the_capability_is_advertised() {
    let probe = probe_from(tree_root());
    assert!(
        probe
            .capabilities()
            .contains(&"oversized_images".to_owned())
    );
}
