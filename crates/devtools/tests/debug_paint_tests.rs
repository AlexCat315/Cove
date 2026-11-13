//! Debug paint guidelines: live node outlines, bounded and clipped.
//!
//! Flutter's debug paint also draws baselines, a repaint rainbow and slow
//! animations. This runtime has no data for those, so the probe implements
//! guidelines only and these tests pin what the overlay actually emits: real
//! `absolute_frame` geometry from the live tree, four one-pixel quads per
//! visible node, nothing when disabled, and a cap for large trees.

use cove_core::scene::{Node, NodeTree};
use cove_core::{Color, DrawList, Rect};
use cove_devtools::inspection::MAX_GUIDELINE_NODES;
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

/// Every outline is four quads, and `DrawList` emits four vertices per quad.
fn rects(list: &DrawList) -> usize {
    list.vertices().len() / 4
}

fn open_tree() -> (Node, Node, Node, Node) {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    let left = Node::new();
    left.set_frame(Rect::new(0.0, 0.0, 200.0, 300.0));
    let right = Node::new();
    right.set_frame(Rect::new(200.0, 0.0, 200.0, 300.0));
    let inner = Node::new();
    inner.set_frame(Rect::new(10.0, 20.0, 40.0, 30.0));
    right.add_child(&inner);
    root.add_child(&left);
    root.add_child(&right);
    (root, left, right, inner)
}

#[test]
fn enabling_guidelines_adds_live_node_outlines_and_disabling_removes_them() {
    let (root, _, _, inner) = open_tree();
    let mut probe = probe_from(root);
    let selection = Color::from_rgba8(80, 160, 255, 255).rgba8();
    assert!(
        probe
            .execute(ProbeCommand::Select {
                node_id: Some(node_id(&inner)),
            })
            .is_ok()
    );

    let disabled = overlay(&probe);
    assert_eq!(rects(&disabled), 4, "only the selection border is drawn");

    let applied = probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    assert_eq!(
        applied,
        serde_json::json!({"guidelines": true}),
        "the probe reports the state it applied"
    );
    assert!(probe.scene.editor.debug_paint().guidelines);

    let enabled = overlay(&probe);
    // Root, left, right and the selected inner node are all visible.
    assert_eq!(
        rects(&enabled),
        4 * 4 + 4,
        "four guideline quads per visible node plus the selection border"
    );
    assert_eq!(enabled.current_clip(), None, "clip stack is balanced");
    // Guidelines are clipped to the root viewport, so their batch carries the
    // root frame as scissor. The selection border uses no ancestors that clip,
    // so it stays in its own unclipped batch after the guidelines.
    let root_frame = Rect::new(0.0, 0.0, 400.0, 300.0);
    assert!(
        enabled
            .batches()
            .iter()
            .any(|batch| batch.scissor == Some(root_frame)),
        "guidelines are clipped to the root"
    );
    let selection_batch = enabled
        .batches()
        .iter()
        .find(|batch| batch.scissor.is_none())
        .expect("selection border is not clipped by the guideline root clip");
    let first = selection_batch.index_offset as usize;
    let last = first + selection_batch.index_count as usize;
    assert!(first > 0, "selection is drawn after the guidelines");
    let vertices = enabled.vertices();
    let selection_vertices: Vec<_> = enabled.indices()[first..last]
        .iter()
        .map(|&index| vertices[index as usize])
        .collect();
    assert!(
        selection_vertices
            .iter()
            .all(|vertex| vertex.color == selection),
        "the selection border is painted over the guidelines"
    );
    let min_x = selection_vertices
        .iter()
        .map(|v| v.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = selection_vertices
        .iter()
        .map(|v| v.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = selection_vertices
        .iter()
        .map(|v| v.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = selection_vertices
        .iter()
        .map(|v| v.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(
        (min_x, min_y, max_x, max_y),
        (210.0, 20.0, 250.0, 50.0),
        "the selection border sits on the live inner node frame"
    );

    let applied = probe
        .execute(ProbeCommand::DebugPaint { guidelines: false })
        .unwrap();
    assert_eq!(applied, serde_json::json!({"guidelines": false}));
    assert_eq!(
        rects(&overlay(&probe)),
        4,
        "disabling removes every guideline and keeps the selection"
    );
}

#[test]
fn disabling_guidelines_without_a_selection_draws_nothing() {
    let (root, _, _, _) = open_tree();
    let mut probe = probe_from(root);
    assert_eq!(rects(&overlay(&probe)), 0);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    assert_eq!(rects(&overlay(&probe)), 4 * 4);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: false })
        .unwrap();
    let empty = overlay(&probe);
    assert_eq!(rects(&empty), 0);
    assert!(empty.vertices().is_empty());
}

#[test]
fn toggling_redraws_through_the_change_callback_only_on_a_real_change() {
    let (root, _, _, _) = open_tree();
    let mut probe = probe_from(root);
    let redraws = Rc::new(Cell::new(0));
    {
        let redraws = redraws.clone();
        *probe.scene.editor.on_change.borrow_mut() =
            Some(Rc::new(move || redraws.set(redraws.get() + 1)));
    }
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    assert_eq!(redraws.get(), 1);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    assert_eq!(redraws.get(), 1, "an unchanged state schedules no redraw");
    probe.disconnect();
    assert_eq!(redraws.get(), 2, "disconnect clears debug paint");
    assert!(!probe.scene.editor.debug_paint().guidelines);
}

#[test]
fn invisible_and_zero_area_nodes_get_no_guideline() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));

    let hidden = Node::new();
    hidden.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    hidden.set_layout_visibility(false);
    let hidden_child = Node::new();
    hidden_child.set_frame(Rect::new(10.0, 10.0, 20.0, 20.0));
    hidden.add_child(&hidden_child);

    let zero_area = Node::new();

    let transparent = Node::new();
    transparent.set_frame(Rect::new(200.0, 0.0, 50.0, 50.0));
    transparent.set_opacity(0.0);

    root.add_child(&hidden);
    root.add_child(&zero_area);
    root.add_child(&transparent);

    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    assert_eq!(
        rects(&overlay(&probe)),
        4,
        "only the visible root with positive area is outlined"
    );
}

#[test]
fn a_descendant_outline_is_clipped_by_its_ancestors_and_the_root() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    let scroller = Node::new();
    scroller.set_frame(Rect::new(10.0, 20.0, 200.0, 100.0));
    scroller.set_clips_to_bounds(true);
    let child = Node::new();
    child.set_frame(Rect::new(5.0, 5.0, 50.0, 20.0));
    scroller.add_child(&child);
    root.add_child(&scroller);

    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    let list = overlay(&probe);
    assert_eq!(rects(&list), 3 * 4);
    let scroller_frame = Rect::new(10.0, 20.0, 200.0, 100.0);
    assert!(
        list.batches()
            .iter()
            .any(|batch| batch.scissor == Some(scroller_frame)),
        "the child's outline is clipped to its clipping ancestor"
    );
    assert_eq!(list.current_clip(), None);
}

#[test]
fn a_large_tree_is_capped() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 10_000.0, 10_000.0));
    for index in 0..MAX_GUIDELINE_NODES + 50 {
        let child = Node::new();
        let x = (index % 100) as f32 * 10.0;
        let y = (index / 100) as f32 * 10.0;
        child.set_frame(Rect::new(x, y, 8.0, 8.0));
        root.add_child(&child);
    }
    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    let list = overlay(&probe);
    assert_eq!(
        rects(&list),
        MAX_GUIDELINE_NODES * 4,
        "the overlay emits at most {MAX_GUIDELINE_NODES} outlines"
    );
    assert_eq!(list.vertices().len(), MAX_GUIDELINE_NODES * 16);
    assert_eq!(list.current_clip(), None);
}

#[test]
fn guideline_geometry_follows_a_live_frame_change() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    let child = Node::new();
    child.set_frame(Rect::new(10.0, 20.0, 100.0, 50.0));
    root.add_child(&child);
    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();

    let has_corner = |list: &DrawList, x: f32, y: f32| {
        list.vertices()
            .iter()
            .any(|vertex| vertex.x == x && vertex.y == y)
    };
    let before = overlay(&probe);
    assert!(has_corner(&before, 10.0, 20.0));
    assert!(has_corner(&before, 110.0, 70.0));

    child.set_frame(Rect::new(30.0, 40.0, 100.0, 50.0));
    let after = overlay(&probe);
    assert!(
        has_corner(&after, 30.0, 40.0) && has_corner(&after, 130.0, 90.0),
        "the outline reads the node's current absolute frame"
    );
    assert!(
        !has_corner(&after, 10.0, 20.0),
        "the previous frame is no longer outlined"
    );
}

#[test]
fn the_capability_is_advertised() {
    let (root, _, _, _) = open_tree();
    let probe = probe_from(root);
    assert!(probe.capabilities().contains(&"debug_paint".to_owned()));
}
