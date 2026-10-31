//! Translation of `Tests/CoveUIComposeTests/ScrollOffsetBindingTests.swift`.
//!
//! Behaviour checklist:
//! - binding offsets use the newly laid-out content bounds after growth and
//!   shrink, clamp to the range and write the clamped value back
//! - native wheel input stays direct (no glide after the notch)
//! - timed scroll requests animate, clamp, cancel on wheel input and snap under
//!   reduced motion
//! - a visible scroll anchor keeps its viewport position when content is
//!   prepended
//!
//! Named differences:
//! - GAP `scroll_anchor_modifier`: Swift exposes `.scrollAnchor()` on any view.
//!   Rust has the anchor-preserving logic inside `ScrollView` (it reads the
//!   `__scrollview_anchor` attachment) but no public modifier sets that marker,
//!   so rows built only from public API do not anchor and the offset stays
//!   where it was. The anchoring itself is still verified through a local
//!   marker modifier.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, TupleView, View, ViewExt, ViewGraph, ViewModifier};
use cove_core::scene::{
    Align, AnimatorScheduler, EventDispatcher, FlexDirection, Justify, Node, PlatformInputContext,
    ScrollAnimator, Wrap,
};
use cove_core::{Point, State};
use cove_platform::{InputEvent, MouseWheelEvent, WindowId};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollRequest, ScrollView};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;
use cove_ui_compose::BoxView;

use support::GlobalTestLock;

fn empty_box() -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    )
}

#[derive(Clone)]
struct Harness {
    content_height: State<f32>,
    offset: State<Point>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        AnyView::new(
            ScrollView::new(
                ScrollAxis::Vertical,
                empty_box()
                    .frame_height(self.content_height.get())
                    .flex_with(0.0, 0.0, None),
            )
            .scroll_offset(self.offset.binding())
            .frame_size(200.0, 100.0),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("contentHeight", &self.content_height),
            DynamicField::state("offset", &self.offset),
        ]
    }
}

/// Marks a row as a scroll anchor the way Swift's `.scrollAnchor()` does.
struct AnchorMarker;
impl ViewModifier for AnchorMarker {
    fn apply_node(&self, node: &Node) {
        node.set_attachment("__scrollview_anchor", true);
    }
}

#[derive(Clone)]
struct AnchorHarness {
    rows: State<Vec<i32>>,
    offset: State<Point>,
    marks_anchors: bool,
}

impl View for AnchorHarness {
    fn body(&self) -> AnyView {
        let rows: Vec<AnyView> = self
            .rows
            .get()
            .into_iter()
            .map(|row| {
                let text = Text::new(format!("Row {row}")).frame_height(40.0);
                if self.marks_anchors {
                    AnyView::new(text.modifier(AnchorMarker).id(row))
                } else {
                    AnyView::new(text.id(row))
                }
            })
            .collect();
        AnyView::new(
            ScrollView::new(
                ScrollAxis::Vertical,
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(rows)),
                )
                .flex_with(0.0, 0.0, None),
            )
            .scroll_offset(self.offset.binding())
            .frame_size(200.0, 100.0),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("rows", &self.rows),
            DynamicField::state("offset", &self.offset),
        ]
    }
}

#[derive(Clone)]
struct TimedHarness {
    request: State<Option<ScrollRequest>>,
    offset: State<Point>,
}

impl View for TimedHarness {
    fn body(&self) -> AnyView {
        let mut scroll = ScrollView::new(
            ScrollAxis::Vertical,
            empty_box().frame_height(1_000.0).flex_with(0.0, 0.0, None),
        )
        .scroll_offset(self.offset.binding());
        if let Some(request) = self.request.get() {
            scroll = scroll.scroll_request(request);
        }
        AnyView::new(scroll.frame_size(200.0, 100.0))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("request", &self.request),
            DynamicField::state("offset", &self.offset),
        ]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn scroll_node(graph: &ViewGraph) -> Node {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all.into_iter()
        .find(|node| node.scroll_reveal().is_some())
        .expect("the scroll view installs a reveal handler")
}

fn settle(graph: &ViewGraph) {
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 200.0);
    }
}

fn wheel(graph: &ViewGraph, context: &PlatformInputContext) {
    let dispatcher = EventDispatcher::new(
        graph.tree.clone(),
        context.interactions.clone(),
        context.pointer_capture.clone(),
        context.focus_chain.clone(),
        WindowId::MAIN,
    );
    dispatcher.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
        x: 0.0,
        y: -1.0,
        mouse_x: Some(100.0),
        mouse_y: Some(50.0),
        ..Default::default()
    }));
}

fn scene(body: impl FnOnce(&PlatformInputContext)) {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| body(&context));
        });
    });
}

#[test]
fn content_growth_and_shrink() {
    scene(|context| {
        let harness = Harness {
            content_height: State::new(400.0),
            offset: State::new(Point::default()),
        };
        let graph = support::graph();
        graph.install(harness.clone(), Default::default());
        settle(&graph);
        let scroll = scroll_node(&graph);

        harness.offset.set(Point::new(0.0, 250.0));
        settle(&graph);
        assert_eq!(scroll.content_offset().y, 250.0);

        harness.content_height.set(800.0);
        harness.offset.set(Point::new(0.0, 650.0));
        settle(&graph);
        assert_eq!(scroll.content_offset().y, 650.0, "the grown range admits 650");
        assert_eq!(harness.offset.get().y, 650.0);

        wheel(&graph, context);
        let after_wheel = scroll.content_offset().y;
        assert_eq!(after_wheel, 680.0, "one line notch moves 30pt");
        AnimatorScheduler::current().tick(0.2);
        assert_eq!(scroll.content_offset().y, after_wheel, "wheel input is direct");
        let motion = scroll
            .attachment::<Rc<ScrollAnimator>>("__scrollview_motion")
            .expect("motion animator");
        assert!(!motion.is_gliding());

        harness.content_height.set(180.0);
        harness.offset.set(Point::new(0.0, 70.0));
        settle(&graph);
        assert_eq!(scroll.content_offset().y, 70.0);
        assert_eq!(harness.offset.get().y, 70.0);

        harness.offset.set(Point::new(0.0, 9_000.0));
        settle(&graph);
        assert_eq!(scroll.content_offset().y, 80.0, "the offset clamps to the shrunk range");
        assert_eq!(harness.offset.get().y, 80.0, "the clamped offset is written back");
        graph.install(cove_composition::EmptyView, Default::default());
    });
}

#[test]
fn timed_scroll() {
    scene(|context| {
        let harness = TimedHarness {
            request: State::new(None),
            offset: State::new(Point::default()),
        };
        let graph = support::graph();
        graph.install(harness.clone(), Default::default());
        settle(&graph);
        let scroll = scroll_node(&graph);

        harness
            .request
            .set(Some(ScrollRequest::new(Point::new(0.0, 600.0), 0.2)));
        settle(&graph);
        AnimatorScheduler::current().tick(0.1);
        let mid = scroll.content_offset().y;
        assert!(mid > 0.0 && mid < 600.0, "a timed request animates (mid {mid})");
        AnimatorScheduler::current().tick(0.3);
        assert_eq!(scroll.content_offset().y, 600.0);
        assert_eq!(harness.offset.get().y, 600.0);

        harness
            .request
            .set(Some(ScrollRequest::new(Point::new(0.0, 900.0), 0.4)));
        settle(&graph);
        AnimatorScheduler::current().tick(0.05);
        let before_wheel = scroll.content_offset().y;
        wheel(&graph, context);
        assert_ne!(scroll.content_offset().y, before_wheel, "the wheel moves the content");
        AnimatorScheduler::current().tick(0.5);
        assert!(
            scroll.content_offset().y < 900.0,
            "wheel input cancels the timed request (offset {})",
            scroll.content_offset().y
        );
        graph.install(cove_composition::EmptyView, Default::default());
    });

    scene(|_| {
        let harness = TimedHarness {
            request: State::new(Some(ScrollRequest::new(Point::new(0.0, 700.0), 0.4))),
            offset: State::new(Point::default()),
        };
        let graph = support::graph();
        graph.install(harness.reduced_motion(true), Default::default());
        settle(&graph);
        let scroll = scroll_node(&graph);
        assert_eq!(scroll.content_offset().y, 700.0, "reduced motion snaps the request");
        graph.install(cove_composition::EmptyView, Default::default());
    });
}

fn prepend_rows(marks_anchors: bool) -> (f32, f32) {
    let mut result = (0.0, 0.0);
    scene(|_| {
        let harness = AnchorHarness {
            rows: State::new((0..24).collect()),
            offset: State::new(Point::default()),
            marks_anchors,
        };
        let graph = support::graph();
        graph.install(harness.clone(), Default::default());
        settle(&graph);
        let scroll = scroll_node(&graph);
        harness.offset.set(Point::new(0.0, 400.0));
        settle(&graph);
        assert_eq!(scroll.content_offset().y, 400.0);
        let mut rows = vec![-3, -2, -1];
        rows.extend(harness.rows.get());
        harness.rows.set(rows);
        settle(&graph);
        result = (scroll.content_offset().y, harness.offset.get().y);
        graph.install(cove_composition::EmptyView, Default::default());
    });
    result
}

#[test]
fn scroll_anchor() {
    let (offset, bound) = prepend_rows(true);
    assert_eq!(offset, 520.0, "the anchor keeps its viewport position");
    assert_eq!(bound, 520.0, "the anchored offset is written back");
}

#[test]
fn scroll_anchor_requires_a_marker() {
    let (offset, bound) = prepend_rows(false);
    assert_eq!(
        (offset, bound),
        (400.0, 400.0),
        "GAP: Swift's public `.scrollAnchor()` modifier marks rows so prepending keeps \
         them in place (520); Rust has no public anchor modifier, so rows built from \
         public API do not anchor and the offset stays at 400"
    );
}
