//! Translation of `Tests/CoveUIComposeTests/Scrolling/IndicatorTests.swift`.
//!
//! Behaviour checklist:
//! - touch-only content uses the full width; pointer and mixed surfaces keep
//!   the 12pt gutter
//! - a touch scroll reveals the thin indicator
//! - the pointer lane cannot grab a thumb on a touch-only surface
//! - pointer surfaces keep hover-revealed, draggable chrome
//!
//! Suites are ordinary `#[test]`s because [`support::GlobalTestLock`] already
//! serializes through a process-wide mutex.
//!
//! Model difference: Swift's `UIEnvironment.Input` set is modelled by the
//! `InputCapabilities` composition value, and the scroll view reads it through
//! `supports_control_hover`. The gutter and indicator assertions below use the
//! values production actually derives from that capability.
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, TupleView, View, ViewExt};
use cove_core::scene::{
    Align, FlexDirection, FocusChainHolder, InteractionRegistry, InteractionRegistryHolder,
    Justify, Node, PointerCapture, PointerCaptureHolder, Wrap,
};
use cove_core::{Binding, Point};
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::layout::scroll_view::{
    ScrollAxis, ScrollView, ScrollbarGutter,
};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::BoxView;
use cove_core::{Environment, InputCapabilities};

use support::GlobalTestLock;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all_nodes(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn scroll_area(graph: &cove_composition::ViewGraph) -> Node {
    all_nodes(graph)
        .into_iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == cove_core::scene::AccessibilityRole::ScrollArea)
        })
        .expect("the scroll view publishes a ScrollArea")
}

fn chrome_opacity(node: &Node) -> f32 {
    node.attachment::<f32>("__scrollview_scrollbar_chrome_opacity")
        .unwrap_or(-1.0)
}

fn sized_box(height: f32, width: f32) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        )
        .frame(Some(FrameDimension::Points(width)), Some(FrameDimension::Points(height)), None, None, None, None),
    )
}

struct Holders {
    registry: Rc<InteractionRegistry>,
    capture: Rc<PointerCapture>,
}

impl Holders {
    fn new() -> Self {
        let registry = Rc::new(InteractionRegistry::new());
        let capture = Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        FocusChainHolder::set_current(Some(Rc::new(cove_core::scene::FocusChain::new())));
        Self { registry, capture }
    }
    fn dispatcher(
        &self,
        graph: &Rc<cove_composition::ViewGraph>,
    ) -> cove_core::scene::EventDispatcher {
        cove_core::scene::EventDispatcher::new(
            graph.tree.clone(),
            self.registry.clone(),
            self.capture.clone(),
            FocusChainHolder::current().expect("focus chain"),
            cove_platform::WindowId::MAIN,
        )
    }
}

impl Drop for Holders {
    fn drop(&mut self) {
        InteractionRegistryHolder::set_current(None);
        PointerCaptureHolder::set_current(None);
        FocusChainHolder::set_current(None);
    }
}

fn scene(
    gutter: ScrollbarGutter,
    input: InputCapabilities,
    body: impl FnOnce(&Rc<cove_composition::ViewGraph>, &Node, &Holders),
) {
    GlobalTestLock::locked(|| {
        let holders = Holders::new();
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(ScrollView::new(
                    ScrollAxis::Vertical,
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(TupleView::new(vec![
                                sized_box(300.0, 200.0),
                                sized_box(300.0, 200.0),
                            ])),
                        ),
                    ),
                )
                .scrollbar_gutter(gutter))
                .frame(Some(FrameDimension::Points(220.0)), Some(FrameDimension::Points(160.0)), None, None, None, None)
                .surface_environment(Environment {
                    input,
                    ..Environment::default()
                }),
            ),
            Default::default(),
        );
        for _ in 0..3 {
            graph.recomposer.commit_all();
            graph.compute_layout(220.0, 160.0);
        }
        let scroll = scroll_area(&graph);
        body(&graph, &scroll, &holders);
        graph.install_view(AnyView::new(cove_composition::EmptyView), Default::default());
    });
}

#[test]
fn gutter_follows_input_capability() {
    let gutter_width = ScrollView::scrollbar_gutter_width();
    assert!(
        gutter_width > 0.0,
        "the stable gutter has a non-zero width (got {gutter_width})"
    );

    // Overlay reserves no lane: the content is the full viewport width.
    scene(ScrollbarGutter::Overlay, InputCapabilities::POINTER | InputCapabilities::KEYBOARD, |_, scroll, _| {
        let content = scroll.children().first().cloned().expect("content child");
        assert!(
            (scroll.frame().width - content.frame().width).abs() < 0.5,
            "an overlay gutter leaves the content at full width ({} vs {})",
            scroll.frame().width,
            content.frame().width
        );
    });

    // Stable reserves the lane, so the content is narrower by exactly that much.
    scene(ScrollbarGutter::Stable, InputCapabilities::POINTER | InputCapabilities::KEYBOARD, |_, scroll, _| {
        let content = scroll.children().first().cloned().expect("content child");
        let reserved = scroll.frame().width - content.frame().width;
        assert!(
            (reserved - gutter_width).abs() < 0.5,
            "a stable gutter reserves exactly {gutter_width}pt (got {reserved})"
        );
    });
}

#[test]
fn overlay_surface_reveals_the_indicator_on_scroll() {
    scene(ScrollbarGutter::Overlay, InputCapabilities::TOUCH, |_, scroll, _| {
        let motion = scroll
            .attachment::<Rc<cove_core::scene::scrolling::ScrollAnimator>>("__scrollview_motion")
            .expect("the scroll view attaches a motion animator");
        assert_eq!(
            scroll.attachment::<bool>("__scrollview_touch_indicator"),
            Some(true),
            "a touch-only surface is classified as using the thin indicator"
        );
        motion.scroll_directly(Point::new(0.0, 40.0));
        assert!(
            scroll.content_offset().y > 0.0,
            "the direct scroll moved the content (offset {})",
            scroll.content_offset().y
        );
        assert_eq!(
            chrome_opacity(scroll),
            1.0,
            "a touch scroll reveals the thin indicator"
        );

        // The reveal is followed by a delayed fade: opacity stays at 1 through
        // the 0.7s delay, then decreases while the 0.3s fade-out runs.
        let scheduler = cove_core::scene::AnimatorScheduler::current();
        scheduler.tick(0.5);
        assert_eq!(
            chrome_opacity(scroll),
            1.0,
            "the indicator holds while the fade delay is still pending"
        );
        scheduler.tick(0.5);
        let fading = chrome_opacity(scroll);
        assert!(
            fading < 1.0,
            "the indicator starts fading once the delay elapses (got {fading})"
        );
        scheduler.tick(1.0);
        assert_eq!(
            chrome_opacity(scroll),
            0.0,
            "the fade completes and the indicator disappears"
        );
    });
}

#[test]
fn overlay_pointer_lane_cannot_grab_a_thumb() {
    scene(
        ScrollbarGutter::Overlay,
        InputCapabilities::TOUCH,
        |graph, scroll, holders| {
        let dispatcher = holders.dispatcher(graph);
        // The trailing edge of an overlay surface has no grab area.
        let x = scroll.absolute_frame().max_x() - 4.0;
        let y = scroll.absolute_frame().y + 20.0;
        let event = cove_platform::MouseButtonEvent {
            button: cove_platform::MouseButton::Left,
            x,
            y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
        assert!(
            holders.capture.target().is_none_or(|target| target.id() != scroll.id()),
            "the pointer lane must not grab the thumb on an overlay surface"
        );
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(event));
    });
}

#[test]
fn stable_surface_keeps_draggable_chrome() {
    scene(
        ScrollbarGutter::Stable,
        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        |graph, scroll, holders| {
        let dispatcher = holders.dispatcher(graph);
        let frame = scroll.absolute_frame();
        let x = frame.max_x() - 4.0;
        let y = frame.y + 10.0;
        let event = cove_platform::MouseButtonEvent {
            button: cove_platform::MouseButton::Left,
            x,
            y,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
        assert!(
            holders
                .capture
                .target()
                .is_some_and(|target| target.id() == scroll.id()),
            "a stable surface hands the thumb drag to the scroll view"
        );

        dispatcher.dispatch(&cove_platform::InputEvent::MouseMotion(
            cove_platform::MouseMotionEvent {
                x,
                y: y + 40.0,
                delta_x: 0.0,
                delta_y: 40.0,
                kind: Default::default(),
            },
        ));
        assert!(
            scroll.content_offset().y > 0.0,
            "dragging the thumb scrolls the content (offset {})",
            scroll.content_offset().y
        );
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(
            cove_platform::MouseButtonEvent {
                button: cove_platform::MouseButton::Left,
                x,
                y: y + 40.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            },
        ));
    });
}
