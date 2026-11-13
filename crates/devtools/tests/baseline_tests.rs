//! Debug paint text baselines from real measured text layout.
//!
//! Baselines are only honest when they come from the layout the renderer
//! measured, so these tests build a real `TextLayoutResult` on a node's
//! `LayoutNode` and pin what the overlay emits: one 1px line per measured text
//! line at `frame.y + baseline_y`, nothing when disabled or on non-text nodes,
//! live frame movement, root/ancestor clipping, the cap, and independence from
//! guidelines and selection.

use cove_core::scene::{LayoutNode, Node, NodeTree};
use cove_core::{
    Color, DrawList, Font, FontWeight, Rect, TextAlignment, TextLayoutCacheEntry,
    TextLayoutCacheKey, TextLayoutResult, TextLine,
};
use cove_devtools::inspection::MAX_BASELINES;
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const BASELINE: Color = Color {
    r: 0.0,
    g: 200.0 / 255.0,
    b: 120.0 / 255.0,
    a: 220.0 / 255.0,
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

fn line(top_y: f32, baseline_y: f32, width: f32) -> TextLine {
    TextLine {
        baseline_y,
        top_y,
        width,
        start_cluster: 0,
        end_cluster: 1,
        ..Default::default()
    }
}

fn text_entry(lines: Vec<TextLine>) -> TextLayoutCacheEntry {
    let total_width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
    let total_height = lines.last().map(|line| line.top_y + 16.0).unwrap_or(0.0);
    TextLayoutCacheEntry {
        key: TextLayoutCacheKey {
            text: "Hello".into(),
            font: Font::system(13.0, FontWeight::Regular),
            line_height: 16.0,
            alignment: TextAlignment::Leading,
            max_width: 400.0,
            letter_spacing: 0.0,
            atlas_id: 1,
        },
        result: TextLayoutResult {
            lines,
            total_width,
            total_height,
        },
    }
}

/// A node whose `LayoutNode` carries a measured text layout, as a real text
/// primitive leaves it after measuring. The returned `LayoutNode` must stay
/// alive; the node only holds a weak reference.
fn text_node(frame: Rect, lines: Vec<TextLine>) -> (Node, LayoutNode) {
    let node = Node::new();
    node.set_frame(frame);
    let layout = LayoutNode::new();
    layout.set_text_measure(Some(text_entry(lines)));
    node.set_layout_node(Some(&layout));
    (node, layout)
}

fn tree_root() -> Node {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    root
}

#[test]
fn enabling_baselines_draws_measured_lines_and_disabling_removes_them() {
    let root = tree_root();
    let (text, _layout) = text_node(
        Rect::new(20.0, 30.0, 120.0, 40.0),
        vec![line(0.0, 12.0, 80.0), line(16.0, 28.0, 40.0)],
    );
    root.add_child(&text);
    let mut probe = probe_from(root);

    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabled draws nothing"
    );

    let applied = probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert_eq!(
        applied,
        serde_json::json!({"baselines": true}),
        "the probe reports the state it applied"
    );
    assert!(probe.scene.editor.baselines());

    let list = overlay(&probe);
    assert_eq!(
        rects(&list),
        vec![
            Rect::new(20.0, 42.0, 80.0, 1.0),
            Rect::new(20.0, 58.0, 40.0, 1.0),
        ],
        "each measured line draws at frame.y + baseline_y with its own width"
    );
    assert!(
        list.vertices()
            .iter()
            .all(|vertex| vertex.color == BASELINE.rgba8()),
        "baselines carry their own color"
    );
    assert_eq!(list.current_clip(), None, "clip stack is balanced");

    let applied = probe
        .execute(ProbeCommand::Baselines { enabled: false })
        .unwrap();
    assert_eq!(applied, serde_json::json!({"baselines": false}));
    assert!(!probe.scene.editor.baselines());
    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabling removes every baseline"
    );
}

#[test]
fn non_text_hidden_zero_area_and_transparent_nodes_get_no_baseline() {
    let root = tree_root();

    // A layout box without measured text is not a text node.
    let plain = Node::new();
    plain.set_frame(Rect::new(0.0, 0.0, 100.0, 40.0));
    let plain_layout = LayoutNode::new();
    plain.set_layout_node(Some(&plain_layout));

    let (hidden, _hidden_layout) = text_node(
        Rect::new(0.0, 50.0, 100.0, 40.0),
        vec![line(0.0, 12.0, 80.0)],
    );
    hidden.set_layout_visibility(false);

    let (transparent, _transparent_layout) = text_node(
        Rect::new(0.0, 100.0, 100.0, 40.0),
        vec![line(0.0, 12.0, 80.0)],
    );
    transparent.set_opacity(0.0);

    let (zero_area, _zero_layout) = text_node(
        Rect::new(0.0, 150.0, 0.0, 40.0),
        vec![line(0.0, 12.0, 80.0)],
    );

    let (detached, _detached_layout) = text_node(
        Rect::new(0.0, 200.0, 100.0, 40.0),
        vec![line(0.0, 12.0, 80.0)],
    );

    root.add_child(&plain);
    root.add_child(&hidden);
    root.add_child(&transparent);
    root.add_child(&zero_area);
    root.add_child(&detached);
    root.remove_child(&detached);

    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert!(
        overlay(&probe).vertices().is_empty(),
        "no node contributes an invented baseline"
    );
}

#[test]
fn baseline_geometry_follows_a_live_frame_move() {
    let root = tree_root();
    let (text, _layout) = text_node(
        Rect::new(10.0, 20.0, 100.0, 50.0),
        vec![line(0.0, 12.0, 60.0)],
    );
    root.add_child(&text);
    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();

    assert_eq!(
        rects(&overlay(&probe)),
        vec![Rect::new(10.0, 32.0, 60.0, 1.0)]
    );

    text.set_frame(Rect::new(30.0, 40.0, 100.0, 50.0));
    assert_eq!(
        rects(&overlay(&probe)),
        vec![Rect::new(30.0, 52.0, 60.0, 1.0)],
        "the baseline reads the node's current absolute frame"
    );
}

#[test]
fn line_width_is_clamped_to_the_node_frame() {
    let root = tree_root();
    let (text, _layout) = text_node(
        Rect::new(10.0, 20.0, 40.0, 50.0),
        vec![line(0.0, 12.0, 500.0)],
    );
    root.add_child(&text);
    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert_eq!(
        rects(&overlay(&probe)),
        vec![Rect::new(10.0, 32.0, 40.0, 1.0)],
        "a stale overlong line stops at the frame edge"
    );
}

#[test]
fn baselines_are_clipped_by_the_root_and_by_clipping_ancestors() {
    let root = tree_root();
    let scroller = Node::new();
    scroller.set_frame(Rect::new(10.0, 20.0, 200.0, 100.0));
    scroller.set_clips_to_bounds(true);
    let (scrolled, _scrolled_layout) =
        text_node(Rect::new(5.0, 5.0, 50.0, 20.0), vec![line(0.0, 10.0, 50.0)]);
    scroller.add_child(&scrolled);
    let (unclipped, _unclipped_layout) = text_node(
        Rect::new(250.0, 10.0, 100.0, 30.0),
        vec![line(0.0, 10.0, 80.0)],
    );
    root.add_child(&scroller);
    root.add_child(&unclipped);

    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    let list = overlay(&probe);
    let mut drawn = rects(&list);
    drawn.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert_eq!(
        drawn,
        vec![
            Rect::new(15.0, 35.0, 50.0, 1.0),
            Rect::new(250.0, 20.0, 80.0, 1.0),
        ]
    );
    let scroller_frame = Rect::new(10.0, 20.0, 200.0, 100.0);
    assert!(
        list.batches()
            .iter()
            .any(|batch| batch.scissor == Some(scroller_frame)),
        "a baseline inside a clipping ancestor is clipped to that ancestor"
    );
    let root_frame = Rect::new(0.0, 0.0, 400.0, 300.0);
    assert!(
        list.batches()
            .iter()
            .any(|batch| batch.scissor == Some(root_frame)),
        "an unclipped baseline is still clipped to the root viewport"
    );
    assert_eq!(list.current_clip(), None, "clip stack is balanced");
}

#[test]
fn baselines_are_capped_across_the_whole_tree() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 10_000.0, 10_000.0));
    let mut layouts = Vec::new();
    for index in 0..MAX_BASELINES + 50 {
        let x = (index % 100) as f32 * 10.0;
        let y = (index / 100) as f32 * 10.0;
        let (node, layout) = text_node(Rect::new(x, y, 8.0, 8.0), vec![line(0.0, 4.0, 8.0)]);
        root.add_child(&node);
        layouts.push(layout);
    }
    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert_eq!(
        rects(&overlay(&probe)).len(),
        MAX_BASELINES,
        "the overlay emits at most {MAX_BASELINES} baseline lines"
    );
    drop(layouts);
}

#[test]
fn the_cap_also_limits_a_single_tall_text_node() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 10_000.0, 10_000.0));
    let lines = (0..MAX_BASELINES + 20)
        .map(|index| {
            let top = index as f32 * 16.0;
            line(top, top + 12.0, 80.0)
        })
        .collect();
    let (text, _layout) = text_node(Rect::new(0.0, 0.0, 100.0, 10_000.0), lines);
    root.add_child(&text);
    let mut probe = probe_from(root);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert_eq!(rects(&overlay(&probe)).len(), MAX_BASELINES);
}

#[test]
fn guidelines_and_selection_render_independently_of_baselines() {
    let root = tree_root();
    let left = Node::new();
    left.set_frame(Rect::new(0.0, 0.0, 200.0, 300.0));
    let right = Node::new();
    right.set_frame(Rect::new(200.0, 0.0, 200.0, 300.0));
    let (text, _layout) = text_node(
        Rect::new(10.0, 20.0, 40.0, 30.0),
        vec![line(0.0, 12.0, 30.0)],
    );
    right.add_child(&text);
    root.add_child(&left);
    root.add_child(&right);

    let mut probe = probe_from(root);
    assert!(
        probe
            .execute(ProbeCommand::Select {
                node_id: Some(node_id(&text)),
            })
            .is_ok()
    );
    probe
        .execute(ProbeCommand::DebugPaint { guidelines: true })
        .unwrap();
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();

    let list = overlay(&probe);
    assert_eq!(
        rects(&list).len(),
        4 * 4 + 1 + 4,
        "four guideline quads per visible node, one baseline, one selection border"
    );
    assert!(probe.scene.editor.debug_paint().guidelines);
    assert!(
        rects(&list)
            .iter()
            .any(|rect| *rect == Rect::new(210.0, 32.0, 30.0, 1.0)),
        "the baseline is present next to the guides"
    );
    let selection: Vec<_> = list
        .vertices()
        .iter()
        .filter(|vertex| vertex.color == SELECTION.rgba8())
        .collect();
    assert_eq!(selection.len(), 16, "the selection border is four quads");
    let xs: Vec<f32> = selection.iter().map(|vertex| vertex.x).collect();
    let ys: Vec<f32> = selection.iter().map(|vertex| vertex.y).collect();
    assert_eq!(
        (
            xs.iter().copied().fold(f32::INFINITY, f32::min),
            ys.iter().copied().fold(f32::INFINITY, f32::min),
            xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        ),
        (210.0, 20.0, 250.0, 50.0),
        "the selection border still tracks the selected frame"
    );

    probe
        .execute(ProbeCommand::Baselines { enabled: false })
        .unwrap();
    assert_eq!(
        rects(&overlay(&probe)).len(),
        4 * 4 + 4,
        "turning baselines off leaves guidelines and selection untouched"
    );
    assert!(probe.scene.editor.debug_paint().guidelines);

    probe
        .execute(ProbeCommand::DebugPaint { guidelines: false })
        .unwrap();
    assert!(
        !probe.scene.editor.baselines(),
        "guidelines cannot change the baseline state"
    );
}

#[test]
fn toggling_baselines_redraws_only_on_a_real_change_and_disconnect_clears_them() {
    let root = tree_root();
    let (text, _layout) = text_node(
        Rect::new(10.0, 20.0, 100.0, 50.0),
        vec![line(0.0, 12.0, 60.0)],
    );
    root.add_child(&text);
    let mut probe = probe_from(root);
    let redraws = Rc::new(Cell::new(0));
    {
        let redraws = redraws.clone();
        *probe.scene.editor.on_change.borrow_mut() =
            Some(Rc::new(move || redraws.set(redraws.get() + 1)));
    }
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert_eq!(redraws.get(), 1);
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .unwrap();
    assert_eq!(redraws.get(), 1, "an unchanged state schedules no redraw");
    probe.disconnect();
    assert_eq!(redraws.get(), 2, "disconnect clears baselines");
    assert!(!probe.scene.editor.baselines());
    assert!(overlay(&probe).vertices().is_empty());
}

#[test]
fn the_capability_is_advertised() {
    let probe = probe_from(tree_root());
    assert!(probe.capabilities().contains(&"baselines".to_owned()));
}
