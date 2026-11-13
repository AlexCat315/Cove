//! Repaint highlight overlay: real paint generations, no cached-layer color.
use cove_core::Color;
use cove_core::DrawList;
use cove_core::Rect;
use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, NodeTree, RenderTree};
use cove_devtools::inspection::{MAX_REPAINT_NODES, REPAINT_PALETTE, repaint_color};
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn probe() -> (RuntimeProbe, Node, Node) {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let child = Node::new();
    child.set_frame(Rect::new(10.0, 20.0, 40.0, 30.0));
    root.add_child(&child);
    let mut tree = NodeTree::new();
    tree.set_root(Some(root.clone()));
    (
        RuntimeProbe::new(Rc::new(RefCell::new(tree)), None, None, None),
        root,
        child,
    )
}

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

const GUIDELINE: Color = Color {
    r: 1.0,
    g: 170.0 / 255.0,
    b: 0.0,
    a: 200.0 / 255.0,
};
const SELECTION: Color = Color {
    r: 80.0 / 255.0,
    g: 160.0 / 255.0,
    b: 1.0,
    a: 1.0,
};

/// Every outline quad as its bounds and the color of its first vertex.
fn quads(list: &DrawList) -> Vec<(Rect, u32)> {
    list.vertices()
        .chunks(4)
        .map(|quad| {
            let min = |value: fn(&cove_core::Vertex) -> f32| {
                quad.iter().map(value).fold(f32::INFINITY, f32::min)
            };
            let max = |value: fn(&cove_core::Vertex) -> f32| {
                quad.iter().map(value).fold(f32::NEG_INFINITY, f32::max)
            };
            (
                Rect::new(
                    min(|v| v.x),
                    min(|v| v.y),
                    max(|v| v.x) - min(|v| v.x),
                    max(|v| v.y) - min(|v| v.y),
                ),
                quad[0].color,
            )
        })
        .collect()
}

/// The colors of every outline quad drawn inside a node's frame.
fn colors_inside(list: &DrawList, frame: Rect) -> Vec<u32> {
    quads(list)
        .into_iter()
        .filter(|(rect, _)| {
            rect.x >= frame.x - 0.001
                && rect.y >= frame.y - 0.001
                && rect.x + rect.width <= frame.x + frame.width + 0.001
                && rect.y + rect.height <= frame.y + frame.height + 0.001
        })
        .map(|(_, color)| color)
        .collect()
}

fn tree_root() -> Node {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    root
}

fn painted_node(frame: Rect) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    node.set_background_color(Some(Color::BLACK));
    node
}

/// Paint a whole tree through the uncached renderer, which really paints every
/// visible node that owns a box or content.
fn paint(root: &Node) {
    NodeRenderer::new().render(root, &mut DrawList::default());
}

fn enable(probe: &mut RuntimeProbe) {
    let applied = probe
        .execute(ProbeCommand::RepaintHighlight { enabled: true })
        .expect("the probe accepts the toggle");
    assert_eq!(applied, serde_json::json!({"enabled": true}));
}

#[test]
fn repaint_highlight_follows_real_paint_generations() {
    let (mut probe, root, child) = probe();
    probe
        .execute(ProbeCommand::RepaintHighlight { enabled: true })
        .unwrap();
    assert!(
        overlay(&probe).vertices().is_empty(),
        "nodes that have never painted must not get a made-up color"
    );

    root.bump_repaint();
    child.bump_repaint();
    let first = overlay(&probe);
    assert_eq!(
        first.vertices().len(),
        8 * 4,
        "two painted nodes draw two 2px outlines (4 rects each)"
    );
    let first_color = first.vertices()[0].color;

    root.bump_repaint();
    let second = overlay(&probe);
    assert_ne!(
        second.vertices()[0].color,
        first_color,
        "a node that painted again changes its highlight color"
    );

    probe
        .execute(ProbeCommand::RepaintHighlight { enabled: false })
        .unwrap();
    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabling removes the repaint overlay"
    );
}

#[test]
fn repaint_highlight_is_capped_and_cleared_on_disconnect() {
    let (mut probe, root, _) = probe();
    for _ in 0..600 {
        let node = Node::new();
        node.set_frame(Rect::new(0.0, 0.0, 4.0, 4.0));
        node.bump_repaint();
        root.add_child(&node);
    }
    root.bump_repaint();
    probe
        .execute(ProbeCommand::RepaintHighlight { enabled: true })
        .unwrap();
    let list = overlay(&probe);
    assert_eq!(
        list.vertices().len(),
        MAX_REPAINT_NODES * 4 * 4,
        "the overlay is capped at {MAX_REPAINT_NODES} outlines"
    );
    probe.disconnect();
    assert!(!probe.scene.editor.repaint_highlight());
    assert!(overlay(&probe).vertices().is_empty());
}

#[test]
fn enabling_outlines_only_nodes_that_really_painted() {
    let root = tree_root();
    let painted = painted_node(Rect::new(10.0, 20.0, 100.0, 50.0));
    let wrapper = Node::new();
    wrapper.set_frame(Rect::new(0.0, 0.0, 50.0, 50.0));
    root.add_child(&painted);
    root.add_child(&wrapper);
    let mut probe = probe_from(root.clone());

    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabled draws nothing"
    );

    paint(&root);
    assert_eq!(root.repaint_epoch(), 0, "the root emits nothing of its own");
    assert_eq!(painted.repaint_epoch(), 1, "the child really painted");
    assert_eq!(
        wrapper.repaint_epoch(),
        0,
        "a structural wrapper never paints"
    );

    enable(&mut probe);
    assert!(probe.scene.editor.repaint_highlight());
    let list = overlay(&probe);
    assert_eq!(
        quads(&list),
        vec![
            (Rect::new(10.0, 20.0, 100.0, 2.0), repaint_color(1).rgba8()),
            (Rect::new(10.0, 68.0, 100.0, 2.0), repaint_color(1).rgba8()),
            (Rect::new(10.0, 20.0, 2.0, 50.0), repaint_color(1).rgba8()),
            (Rect::new(108.0, 20.0, 2.0, 50.0), repaint_color(1).rgba8()),
        ],
        "only the painted child is outlined, in its own generation's color"
    );
    assert_eq!(list.current_clip(), None, "clip stack is balanced");

    let applied = probe
        .execute(ProbeCommand::RepaintHighlight { enabled: false })
        .expect("the probe accepts disabling");
    assert_eq!(applied, serde_json::json!({"enabled": false}));
    assert!(!probe.scene.editor.repaint_highlight());
    assert!(
        overlay(&probe).vertices().is_empty(),
        "disabling removes every outline"
    );
}

#[test]
fn a_repainting_node_changes_color_while_a_stable_one_keeps_its_own() {
    let root = tree_root();
    let stable_frame = Rect::new(10.0, 20.0, 60.0, 40.0);
    let busy_frame = Rect::new(100.0, 20.0, 60.0, 40.0);
    let stable = painted_node(stable_frame);
    let busy = painted_node(busy_frame);
    root.add_child(&stable);
    root.add_child(&busy);

    paint(&root);
    assert_eq!((stable.repaint_epoch(), busy.repaint_epoch()), (1, 1));
    // A second paint of one node only: this is real renderer execution, not a
    // synthetic bump.
    paint(&busy);
    assert_eq!(
        (stable.repaint_epoch(), busy.repaint_epoch()),
        (1, 2),
        "only the painted node advances"
    );

    let mut probe = probe_from(root);
    enable(&mut probe);
    let list = overlay(&probe);
    assert_eq!(
        colors_inside(&list, stable_frame)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([repaint_color(1).rgba8()]),
        "the node that did not paint keeps its generation's color"
    );
    assert_eq!(
        colors_inside(&list, busy_frame)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([repaint_color(2).rgba8()]),
        "the repainted node moves to the next palette entry"
    );
    assert_ne!(
        repaint_color(1),
        repaint_color(2),
        "the palette really changes between generations"
    );
}

#[test]
fn a_layer_served_from_the_cache_keeps_its_color() {
    let root = tree_root();
    let card = painted_node(Rect::new(10.0, 20.0, 60.0, 40.0));
    card.set_clips_to_bounds(true);
    root.add_child(&card);
    let tree = RenderTree::new();
    tree.install(&root);
    let mut probe = probe_from(root);
    enable(&mut probe);
    let mut renderer = LayerAwareNodeRenderer::new();

    renderer.render(&tree, &mut DrawList::default());
    assert_eq!(card.repaint_epoch(), 1);
    let first = colors_inside(&overlay(&probe), Rect::new(10.0, 20.0, 60.0, 40.0));
    assert_eq!(first, vec![repaint_color(1).rgba8(); 4]);

    // The second frame appends the cached layer instead of painting it.
    renderer.render(&tree, &mut DrawList::default());
    assert_eq!(
        card.repaint_epoch(),
        1,
        "cache reuse does not count as a repaint"
    );
    let second = colors_inside(&overlay(&probe), Rect::new(10.0, 20.0, 60.0, 40.0));
    assert_eq!(second, first, "the cached layer keeps its color");
}

#[test]
fn detached_invisible_and_stale_nodes_are_skipped() {
    let root = tree_root();
    let control = painted_node(Rect::new(0.0, 0.0, 50.0, 50.0));

    let hidden = painted_node(Rect::new(60.0, 0.0, 50.0, 50.0));
    hidden.set_layout_visibility(false);
    let transparent = painted_node(Rect::new(120.0, 0.0, 50.0, 50.0));
    transparent.set_opacity(0.0);
    let detached = painted_node(Rect::new(180.0, 0.0, 50.0, 50.0));
    // Painted while it had area, then collapsed: its generation is real but its
    // frame is no longer positive, so it has nothing to outline.
    let stale = painted_node(Rect::new(240.0, 0.0, 50.0, 50.0));

    root.add_child(&control);
    root.add_child(&hidden);
    root.add_child(&transparent);
    root.add_child(&detached);
    root.add_child(&stale);
    root.remove_child(&detached);

    paint(&root);
    stale.set_frame(Rect::new(240.0, 0.0, 0.0, 0.0));
    assert!(stale.repaint_epoch() > 0, "the stale node did paint once");

    let mut probe = probe_from(root);
    enable(&mut probe);
    let list = overlay(&probe);
    assert_eq!(quads(&list).len(), 4, "only the control node is outlined");
    assert_eq!(
        colors_inside(&list, Rect::new(0.0, 0.0, 50.0, 50.0)),
        vec![repaint_color(1).rgba8(); 4]
    );
}

#[test]
fn outlines_are_clipped_by_the_root_and_by_clipping_ancestors() {
    let root = tree_root();
    let scroller = Node::new();
    scroller.set_frame(Rect::new(10.0, 20.0, 200.0, 100.0));
    scroller.set_clips_to_bounds(true);
    let inside = painted_node(Rect::new(5.0, 5.0, 50.0, 20.0));
    scroller.add_child(&inside);
    // Painted at rest, then moved past the root's right edge.
    let outside = painted_node(Rect::new(0.0, 0.0, 100.0, 50.0));
    root.add_child(&scroller);
    root.add_child(&outside);

    paint(&root);
    outside.set_frame(Rect::new(350.0, 0.0, 100.0, 50.0));

    let mut probe = probe_from(root);
    enable(&mut probe);
    let list = overlay(&probe);
    assert_eq!(quads(&list).len(), 8, "both painted nodes are outlined");
    let scroller_frame = Rect::new(10.0, 20.0, 200.0, 100.0);
    assert!(
        list.batches()
            .iter()
            .any(|batch| batch.scissor == Some(scroller_frame)),
        "the outline inside a clipping scroller is clipped to that scroller"
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
    for index in 0..MAX_REPAINT_NODES + 50 {
        let x = (index % 100) as f32 * 100.0;
        let y = (index / 100) as f32 * 100.0;
        root.add_child(&painted_node(Rect::new(x, y, 90.0, 90.0)));
    }
    paint(&root);
    let mut probe = probe_from(root);
    enable(&mut probe);
    assert_eq!(
        quads(&overlay(&probe)).len(),
        MAX_REPAINT_NODES * 4,
        "the overlay emits at most {MAX_REPAINT_NODES} outlines"
    );
}

#[test]
fn the_overlay_is_independent_of_guidelines_and_selection() {
    let root = tree_root();
    let left = Node::new();
    left.set_frame(Rect::new(0.0, 0.0, 200.0, 300.0));
    let right = Node::new();
    right.set_frame(Rect::new(200.0, 0.0, 200.0, 300.0));
    let inner_frame = Rect::new(210.0, 20.0, 40.0, 30.0);
    let inner = painted_node(inner_frame);
    right.add_child(&inner);
    root.add_child(&left);
    root.add_child(&right);
    paint(&root);

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
        quads(&list).len(),
        4 * 4 + 4 + 4,
        "four guideline quads per visible node, one repaint outline and one selection border"
    );
    assert_eq!(
        list.vertices()
            .iter()
            .filter(|vertex| vertex.color == repaint_color(1).rgba8())
            .count(),
        16,
        "the repaint outline is exactly four quads"
    );
    assert_eq!(
        list.vertices()
            .iter()
            .filter(|vertex| vertex.color == GUIDELINE.rgba8())
            .count(),
        4 * 4 * 4,
        "guidelines keep their own color and count"
    );
    assert_eq!(
        list.vertices()
            .iter()
            .filter(|vertex| vertex.color == SELECTION.rgba8())
            .count(),
        16,
        "the selection border is still four quads"
    );

    probe
        .execute(ProbeCommand::RepaintHighlight { enabled: false })
        .expect("repaint highlighting off");
    let without = overlay(&probe);
    assert_eq!(
        quads(&without).len(),
        4 * 4 + 4,
        "turning repaint highlighting off leaves guidelines and selection untouched"
    );
    assert!(probe.scene.editor.debug_paint().guidelines);
    assert!(probe.scene.editor.selected_node().is_some());
    assert!(!probe.scene.editor.repaint_highlight());

    probe
        .execute(ProbeCommand::DebugPaint { guidelines: false })
        .expect("guidelines off");
    probe
        .execute(ProbeCommand::RepaintHighlight { enabled: true })
        .expect("repaint highlighting on");
    assert!(
        !probe.scene.editor.debug_paint().guidelines,
        "repaint highlighting cannot change the guideline state"
    );
    probe
        .execute(ProbeCommand::OversizedImages { enabled: true })
        .expect("oversized images on");
    probe
        .execute(ProbeCommand::Baselines { enabled: true })
        .expect("baselines on");
    assert!(
        probe.scene.editor.repaint_highlight(),
        "neither baselines nor oversized images can change the repaint state"
    );
    assert!(
        !probe.scene.editor.debug_paint().guidelines,
        "the guideline state is unchanged"
    );
}

#[test]
fn colors_cycle_through_the_palette_by_generation() {
    let first: Vec<Color> = (0..REPAINT_PALETTE.len() as u64)
        .map(repaint_color)
        .collect();
    assert_eq!(first, REPAINT_PALETTE.to_vec());
    assert_eq!(
        repaint_color(REPAINT_PALETTE.len() as u64),
        REPAINT_PALETTE[0],
        "the palette wraps"
    );
    assert_eq!(repaint_color(0), REPAINT_PALETTE[0]);
}

#[test]
fn toggling_redraws_only_on_a_real_change_and_disconnect_clears_it() {
    let root = tree_root();
    root.add_child(&painted_node(Rect::new(10.0, 20.0, 60.0, 40.0)));
    paint(&root);
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
        .execute(ProbeCommand::RepaintHighlight { enabled: true })
        .expect("the probe accepts a repeated toggle");
    assert_eq!(redraws.get(), 1, "an unchanged state schedules no redraw");
    probe.disconnect();
    assert_eq!(redraws.get(), 2, "disconnect clears repaint highlighting");
    assert!(!probe.scene.editor.repaint_highlight());
    assert!(overlay(&probe).vertices().is_empty());
}

#[test]
fn the_capability_is_advertised() {
    let probe = probe_from(tree_root());
    assert!(
        probe
            .capabilities()
            .contains(&"repaint_highlight".to_owned())
    );
}
