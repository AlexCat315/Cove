//! Translation of `Tests/CoveUIRuntimeTests/RoundedClipTests.swift`.
//!
//! Rounded clipping containers must keep child geometry inside their corners on
//! every backend, since GPU scissors are rectangular.
//!
//! Behaviour checklist:
//! - a mask cuts the corners of emitted geometry and keeps its interior intact
//! - clipping interpolates texture coordinates along cut edges (circular mask)
//! - appended cached geometry is clipped by the destination's masks
//! - glyphs crossing a straight edge keep exact texture interpolation
//! - invisible geometry drops indices while retaining cache bounds
//! - geometry already clipped to the same mask is reused without retriangulation
//! - nested rounded caches preserve coverage through recording and reuse
//! - a rounded clipping container keeps a full-size child fill inside its border
//!   in both renderers
//! - a cached leaf recorded outside a rounded clip paints once its parent moves
//!   it into view

use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_core::scene::{LayerAwareNodeRenderer, Node, NodeRenderer, NodeTree, RenderTree};
use cove_core::{Clip, Color, DrawList, Point, Rect, RoundedRect};

struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn locked<T>(body: impl FnOnce() -> T) -> T {
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        body()
    }
}

/// Whether any triangle with visible alpha covers `(x, y)`, optionally of one
/// colour family — the Swift `covers(_:_:in:color:)` helper.
fn covers(x: f32, y: f32, list: &DrawList, color: Option<Color>) -> bool {
    for triangle in list.indices().chunks_exact(3) {
        let verts: Vec<&cove_core::Vertex> = triangle
            .iter()
            .map(|index| &list.vertices()[*index as usize])
            .collect();
        if let Some(color) = color
            && verts[0].color & 0x00FF_FFFF != color.rgba8() & 0x00FF_FFFF
        {
            continue;
        }
        let d = (verts[1].y - verts[2].y) * (verts[0].x - verts[2].x)
            + (verts[2].x - verts[1].x) * (verts[0].y - verts[2].y);
        if d.abs() <= 0.0001 {
            continue;
        }
        let a = ((verts[1].y - verts[2].y) * (x - verts[2].x)
            + (verts[2].x - verts[1].x) * (y - verts[2].y))
            / d;
        let b = ((verts[2].y - verts[0].y) * (x - verts[2].x)
            + (verts[0].x - verts[2].x) * (y - verts[2].y))
            / d;
        if a.min(b).min(1.0 - a - b) >= -0.0001 && verts[0].color >> 24 > 20 {
            return true;
        }
    }
    false
}

fn red() -> Color {
    Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, width, height)
}

#[test]
fn mask_cuts_corners() {
    let mut list = DrawList::default();
    list.push_mask(RoundedRect::new(rect(0.0, 0.0, 100.0, 50.0), 12.0));
    list.add_rect(rect(0.0, 0.0, 100.0, 50.0), red()).expect("the background rect");
    list.add_rect(rect(20.0, 20.0, 10.0, 10.0), red()).expect("the interior rect");
    list.pop_mask();

    for (x, y) in [(1.0, 1.0), (99.0, 49.0), (99.0, 1.0)] {
        assert!(!covers(x, y, &list, None), "({x},{y}) is outside the rounded corner");
    }
    for (x, y) in [(50.0, 25.0), (50.0, 1.0), (1.0, 25.0)] {
        assert!(covers(x, y, &list, None), "({x},{y}) survives the mask");
    }
    assert!(covers(6.0, 6.0, &list, None), "a point inside the arc of a 12pt corner is covered");

    // The small interior rect passes through as its original two triangles.
    list.add_rect(rect(0.0, 0.0, 4.0, 4.0), red()).expect("the corner rect");
    assert!(covers(1.0, 1.0, &list, None), "the unmasked corner rect paints over the cut");
}

#[test]
fn interpolation() {
    let mut list = DrawList::default();
    list.push_mask(RoundedRect::new(rect(0.0, 0.0, 100.0, 100.0), 50.0));
    list.add_rect(rect(0.0, 0.0, 100.0, 100.0), red()).expect("the circular fill");

    for vertex in &list.vertices()[4..] {
        let dx = vertex.x - 50.0;
        let dy = vertex.y - 50.0;
        assert!(
            (dx * dx + dy * dy).sqrt() <= 50.01,
            "a cut vertex stays inside the circular mask",
        );
    }
}

#[test]
fn appended_geometry() {
    let mut cached = DrawList::default();
    cached.add_rect(rect(0.0, 0.0, 80.0, 40.0), red()).expect("the cached rect");

    let mut frame = DrawList::default();
    frame.push_mask(RoundedRect::new(rect(0.0, 0.0, 80.0, 40.0), 10.0));
    frame.append(&cached, Point::default()).expect("the append");

    assert!(!covers(1.0, 1.0, &frame, None), "the destination mask cuts the cached corner");
    assert!(covers(40.0, 20.0, &frame, None), "the interior survives");
    let batched: u32 = frame.batches().iter().map(|batch| batch.index_count).sum();
    assert_eq!(batched, frame.indices().len() as u32, "batch index counts sum to the index buffer");
}

#[test]
fn straight_edge_glyph() {
    let mut list = DrawList::default();
    list.push_mask(RoundedRect::new(rect(0.0, 0.0, 100.0, 100.0), 10.0));
    list.add_glyph_quad(
        rect(95.0, 40.0, 10.0, 10.0),
        0,
        red(),
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 1.0 },
    )
    .expect("the glyph quad");

    let mut visible: Vec<cove_core::Vertex> = Vec::new();
    for index in list.indices() {
        let vertex = list.vertices()[*index as usize];
        if !visible.iter().any(|seen| seen.x == vertex.x && seen.y == vertex.y && seen.u == vertex.u) {
            visible.push(vertex);
        }
    }
    assert!(!visible.is_empty(), "the clipped glyph still emits geometry");
    assert!(
        visible.iter().all(|vertex| vertex.x <= 100.0 && vertex.x >= 95.0),
        "every visible vertex stays within the glyph's x span",
    );
    let edge: Vec<&cove_core::Vertex> =
        visible.iter().filter(|vertex| (vertex.x - 100.0).abs() < 0.001).collect();
    assert!(!edge.is_empty(), "the cut edge emits vertices");
    assert!(
        edge.iter().all(|vertex| (vertex.u - 0.5).abs() < 0.001),
        "the cut edge interpolates u exactly halfway",
    );
    assert!(covers(97.0, 45.0, &list, None));
    assert!(!covers(103.0, 45.0, &list, None), "geometry past the mask edge is not covered");
}

#[test]
fn invisible_geometry() {
    let clip = RoundedRect::new(rect(0.0, 0.0, 100.0, 100.0), 10.0);

    let mut outside = DrawList::default();
    outside.push_mask(clip);
    outside.add_rect(rect(150.0, 20.0, 10.0, 10.0), red()).expect("the far rect");
    assert!(outside.indices().is_empty(), "fully clipped geometry drops its indices");
    assert!(outside.batches().is_empty(), "and its batches");
    assert_eq!(outside.vertices().len(), 4, "the vertices are retained for cache bounds");
    assert_eq!(
        outside.vertices().iter().map(|vertex| vertex.x).fold(f32::NEG_INFINITY, f32::max),
        160.0,
    );

    let mut cached = DrawList::default();
    for x in [-30.0_f32, 40.0, 130.0] {
        cached.add_rect(rect(x, 40.0, 10.0, 10.0), red()).expect("a cached rect");
    }
    let mut frame = DrawList::default();
    frame.push_mask(clip);
    frame.append(&cached, Point::default()).expect("the append");

    assert_eq!(frame.indices().len(), 6, "only the visible cached rect keeps its indices");
    assert_eq!(frame.vertices().len(), cached.vertices().len());
    assert!(covers(45.0, 45.0, &frame, None));
    assert!(!covers(-25.0, 45.0, &frame, None));
    assert!(!covers(135.0, 45.0, &frame, None));
}

#[test]
fn preclipped_geometry() {
    let clip = Clip {
        rect: rect(0.0, 0.0, 100.0, 100.0),
        mask: Some(RoundedRect::new(rect(0.0, 0.0, 100.0, 100.0), 15.0)),
    };

    let mut cached = DrawList::default();
    cached.push_clip(clip);
    cached.add_rect(clip.rect, red()).expect("the clipped rect");
    cached.pop_clip(clip);

    let mut frame = DrawList::default();
    frame.push_clip(clip);
    frame.append_preclipped(&cached, &[clip], Point::default());

    assert_eq!(frame.vertices().len(), cached.vertices().len(), "no retriangulation");
    assert_eq!(
        frame.vertices().iter().map(|vertex| vertex.x).collect::<Vec<_>>(),
        cached.vertices().iter().map(|vertex| vertex.x).collect::<Vec<_>>(),
    );
    assert_eq!(
        frame.vertices().iter().map(|vertex| vertex.y).collect::<Vec<_>>(),
        cached.vertices().iter().map(|vertex| vertex.y).collect::<Vec<_>>(),
    );
    assert_eq!(frame.indices(), cached.indices());
    assert!(!covers(1.0, 1.0, &frame, None));
    assert!(covers(50.0, 3.0, &frame, None));

    let mut constrained = DrawList::default();
    constrained.push_clip(clip);
    let extra = Clip {
        rect: rect(40.0, 40.0, 20.0, 20.0),
        mask: Some(RoundedRect::new(rect(40.0, 40.0, 20.0, 20.0), 5.0)),
    };
    constrained.push_clip(extra);
    constrained.append_preclipped(&cached, &[clip], Point::default());

    assert!(covers(50.0, 50.0, &constrained, None), "the extra clip does not hide the shared interior");
    assert!(!covers(20.0, 20.0, &constrained, None));
    assert!(!covers(40.5, 40.5, &constrained, None));
}

fn rc(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(rect(x, y, width, height));
    node
}

fn fill_rect(width: f32, height: f32, color: Color) -> impl Fn(&mut DrawList, Point) + 'static {
    move |list: &mut DrawList, origin: Point| {
        let _ = list.add_rect(Rect::new(origin.x, origin.y, width, height), color);
    }
}

#[test]
fn nested_cache_coverage() {
    GlobalTestLock::locked(|| {
        let root = rc(0.0, 0.0, 120.0, 120.0);
        let pane = rc(0.0, 0.0, 120.0, 120.0);
        pane.set_clips_to_bounds(true);
        pane.set_corner_radius(20.0);
        let inner = rc(10.0, 10.0, 100.0, 100.0);
        inner.set_clips_to_bounds(true);
        inner.set_corner_radius(15.0);
        let leaf = rc(0.0, 0.0, 100.0, 100.0);
        leaf.update_draw("fill", fill_rect(100.0, 100.0, red()));
        root.add_child(&pane);
        pane.add_child(&inner);
        inner.add_child(&leaf);

        let render = RenderTree::new();
        render.install(&root);
        let mut renderer = LayerAwareNodeRenderer::new();
        let mut counts = Vec::new();
        for pass in 0..6 {
            let mut frame = DrawList::default();
            renderer.render(&render, &mut frame);
            assert!(covers(60.0, 12.0, &frame, None), "pass {pass}: the inner top edge is covered");
            assert!(covers(60.0, 60.0, &frame, None), "pass {pass}: the centre is covered");
            assert!(!covers(11.0, 11.0, &frame, None), "pass {pass}: the outer corner is cut");
            assert!(!covers(109.0, 109.0, &frame, None), "pass {pass}: the far corner is cut");
            counts.push(frame.indices().len());
        }
        assert_eq!(
            counts.iter().collect::<std::collections::BTreeSet<_>>().len(),
            1,
            "the cached composite is stable across passes",
        );
    });
}

#[test]
fn container_parity() {
    GlobalTestLock::locked(|| {
        let root = rc(0.0, 0.0, 200.0, 100.0);
        let card = rc(10.0, 10.0, 120.0, 40.0);
        card.set_corner_radius(8.0);
        card.set_clips_to_bounds(true);
        card.set_border_color(Some(Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }));
        card.set_border_width(1.0);
        let chip = rc(0.0, 0.0, 30.0, 40.0);
        chip.set_background_color(Some(red()));
        root.add_child(&card);
        card.add_child(&chip);

        let mut scene_tree = NodeTree::new();
        scene_tree.set_root(Some(root.clone()));
        let render = RenderTree::new();
        render.install(&root);

        let mut direct = DrawList::default();
        NodeRenderer.render(&root, &mut direct);
        let mut layered = DrawList::default();
        LayerAwareNodeRenderer::new().render(&render, &mut layered);

        for (label, list) in [("direct", &direct), ("layered", &layered)] {
            assert!(
                !covers(11.0, 11.0, list, Some(red())),
                "{label}: the corner outside the rounded clip is not painted",
            );
            assert!(
                !covers(10.5, 30.0, list, Some(red())),
                "{label}: the border itself is not painted by the child fill",
            );
            assert!(covers(25.0, 30.0, list, Some(red())), "{label}: ordinary content paints");
            assert!(covers(16.0, 16.0, list, Some(red())), "{label}: content inside the arc paints");
        }
    });
}

#[test]
fn leaf_moved_into_rounded_clip() {
    GlobalTestLock::locked(|| {
        // An inspector section header first laid out below its scrolling pane,
        // then pulled up when the sections above it settle. The mask dropped
        // every triangle while the label sat outside the pane, so reusing that
        // cache at the new position must not paint nothing.
        let root = rc(0.0, 0.0, 200.0, 300.0);
        let pane = rc(0.0, 0.0, 200.0, 100.0);
        pane.set_corner_radius(8.0);
        pane.set_clips_to_bounds(true);
        let header = rc(0.0, 150.0, 200.0, 20.0);
        let label = rc(20.0, 0.0, 60.0, 20.0);
        label.update_draw("label", fill_rect(60.0, 20.0, red()));
        root.add_child(&pane);
        pane.add_child(&header);
        header.add_child(&label);

        let mut scene_tree = NodeTree::new();
        scene_tree.set_root(Some(root.clone()));
        let render = RenderTree::new();
        render.install(&root);
        let mut renderer = LayerAwareNodeRenderer::new();

        let mut offscreen = DrawList::default();
        renderer.render(&render, &mut offscreen);
        assert!(
            !covers(50.0, 160.0, &offscreen, Some(red())),
            "the label below the pane is fully clipped",
        );

        header.set_frame(rect(0.0, 40.0, 200.0, 20.0));
        let mut layered = DrawList::default();
        renderer.render(&render, &mut layered);
        let mut direct = DrawList::default();
        NodeRenderer.render(&root, &mut direct);

        for (label_name, list) in [("direct", &direct), ("layered", &layered)] {
            assert!(
                covers(50.0, 50.0, list, Some(red())),
                "{label_name}: the moved leaf paints once it enters the clip",
            );
        }
    });
}
