//! Translation of `Tests/CoveUIComposeTests/Scrolling/NestedScrollRoutingTests.swift`.
//!
//! Behaviour checklist:
//! - fresh line wheels stay in an overflowing child at both bounds; moving to
//!   the parent scrolls it instead
//! - gesture continuation, OS momentum, and a fresh gesture at a child boundary
//!   do not chain
//! - a fresh stream over the parent releases the child's ownership
//! - a child with no overflow lets the parent scroll
//! - an explicit `Always` policy contains the input instead
//! - a horizontal child passes vertical input and contains only its own
//!   overflowing horizontal axis
//! - the `WhenOffsetChanged` policy explicitly enables same-axis chaining
//!
//! The Swift `Fixture` struct is a support shape for this one suite, so it is
//! folded in here rather than registered as its own target.

use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, TupleView};
use cove_core::Point;
use cove_core::scene::{
    Align, EventDispatcher, FlexDirection, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Justify, Node, PointerCapture, PointerCaptureHolder,
    ScrollConsumePolicy, Wrap,
};
use cove_platform::{InputEvent, MouseMotionEvent, MouseWheelEvent, ScrollInput, WindowId};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};

use support::GlobalTestLock;

fn sized(width: f32, height: f32) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        )
        .frame(
            Some(FrameDimension::Points(width)),
            Some(FrameDimension::Points(height)),
            None,
            None,
            None,
            None,
        ),
    )
}

struct Fixture {
    dispatcher: EventDispatcher,
    graph: Rc<cove_composition::ViewGraph>,
    registry: Rc<InteractionRegistry>,
    capture: Rc<PointerCapture>,
    focus: Rc<FocusChainHolder2>,
    outer: Node,
    inner: Node,
}

type FocusChainHolder2 = cove_core::scene::FocusChain;

impl Drop for Fixture {
    fn drop(&mut self) {
        InteractionRegistryHolder::set_current(None);
        PointerCaptureHolder::set_current(None);
        FocusChainHolder::set_current(None);
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn find_named(node: &Node, name: &str) -> Option<Node> {
    if support::debug_name(node).as_deref() == Some(name) {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_named(&child, name) {
            return Some(found);
        }
    }
    None
}

impl Fixture {
    fn new(content_height: f32, axis: ScrollAxis, policy: Option<ScrollConsumePolicy>) -> Self {
        let registry = Rc::new(InteractionRegistry::new());
        let capture = Rc::new(PointerCapture::new());
        let focus = Rc::new(cove_core::scene::FocusChain::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));

        let content = sized(
            if axis == ScrollAxis::Horizontal {
                500.0
            } else {
                180.0
            },
            content_height,
        );
        let mut inner_view = ScrollView::new(axis, content);
        if let Some(policy) = policy {
            inner_view = inner_view.consume_policy(policy);
        }

        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(ScrollView::new(
                    ScrollAxis::Vertical,
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(
                                    AnyView::new(inner_view.frame_height(150.0))
                                        .debug_name("nested-inner"),
                                ),
                                sized(180.0, 600.0),
                            ])),
                        ))
                        .flex_with(0.0, 0.0, None),
                    ),
                ))
                .frame(
                    Some(FrameDimension::Points(200.0)),
                    Some(FrameDimension::Points(300.0)),
                    None,
                    None,
                    None,
                    None,
                )
                .debug_name("nested-outer"),
            ),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);

        let root = graph.tree.borrow().root().expect("root");
        let outer = find_named(&root, "nested-outer").expect("the outer scroll view is mounted");
        let inner = find_named(&root, "nested-inner").expect("the inner scroll view is mounted");
        let dispatcher = EventDispatcher::new(
            graph.tree.clone(),
            registry.clone(),
            capture.clone(),
            focus.clone(),
            WindowId::MAIN,
        );
        Self {
            dispatcher,
            graph,
            registry,
            capture,
            focus,
            outer,
            inner,
        }
    }

    fn wheel(&self, x: f32, y: f32, pointer_y: f32, input: ScrollInput) {
        let dispatcher = &self.dispatcher;
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: 40.0,
            y: pointer_y,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        dispatcher.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
            x,
            y,
            mouse_x: Some(40.0),
            mouse_y: Some(pointer_y),
            input,
        }));
    }
}

#[test]
fn fresh_line_wheels_stay_in_an_overflowing_child_at_both_bounds() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new(500.0, ScrollAxis::Vertical, None);
        let input = ScrollInput::default();
        fixture.wheel(0.0, 1.0, 40.0, input);
        assert_eq!(
            fixture.inner.content_offset(),
            Point::default(),
            "a wheel up at the top stays put"
        );
        assert_eq!(fixture.outer.content_offset(), Point::default());

        fixture.wheel(0.0, -100.0, 40.0, input);
        assert_eq!(
            fixture.inner.content_offset().y,
            350.0,
            "the child scrolls its own extent"
        );
        for _ in 0..3 {
            fixture.wheel(0.0, -1.0, 40.0, input);
        }
        assert_eq!(
            fixture.inner.content_offset().y,
            350.0,
            "the child clamps at its own bottom"
        );
        assert_eq!(
            fixture.outer.content_offset(),
            Point::default(),
            "the parent does not receive the child's saturated wheel"
        );

        // Moving the pointer out of the child gives the parent the input.
        fixture.wheel(0.0, -1.0, 220.0, input);
        assert_eq!(fixture.outer.content_offset().y, 30.0, "the parent scrolls");

        fixture.wheel(0.0, 100.0, 40.0, input);
        fixture.wheel(0.0, 1.0, 40.0, input);
        assert_eq!(
            fixture.inner.content_offset(),
            Point::default(),
            "the child returns to the top"
        );
        assert_eq!(
            fixture.outer.content_offset().y,
            30.0,
            "and the parent keeps its own offset"
        );
    });
}

#[test]
fn phased_continuation_does_not_chain() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new(500.0, ScrollAxis::Vertical, None);
        let continuous = |phase, momentum_phase| ScrollInput {
            unit: cove_platform::ScrollUnit::Points,
            device: cove_platform::ScrollDevice::Continuous,
            phase,
            momentum_phase,
            ..ScrollInput::default()
        };

        fixture.wheel(
            0.0,
            -400.0,
            40.0,
            continuous(
                cove_platform::ScrollPhase::Began,
                cove_platform::ScrollPhase::None,
            ),
        );
        assert_eq!(fixture.inner.content_offset().y, 350.0);
        fixture.wheel(
            0.0,
            -12.0,
            220.0,
            continuous(
                cove_platform::ScrollPhase::Changed,
                cove_platform::ScrollPhase::None,
            ),
        );
        fixture.wheel(
            0.0,
            0.0,
            40.0,
            continuous(
                cove_platform::ScrollPhase::Ended,
                cove_platform::ScrollPhase::None,
            ),
        );
        fixture.wheel(
            0.0,
            -8.0,
            220.0,
            continuous(
                cove_platform::ScrollPhase::None,
                cove_platform::ScrollPhase::Began,
            ),
        );
        fixture.wheel(
            0.0,
            -4.0,
            220.0,
            continuous(
                cove_platform::ScrollPhase::None,
                cove_platform::ScrollPhase::Changed,
            ),
        );
        fixture.wheel(
            0.0,
            0.0,
            40.0,
            continuous(
                cove_platform::ScrollPhase::None,
                cove_platform::ScrollPhase::Ended,
            ),
        );
        fixture.wheel(
            0.0,
            -9.0,
            40.0,
            continuous(
                cove_platform::ScrollPhase::Began,
                cove_platform::ScrollPhase::None,
            ),
        );
        assert_eq!(
            fixture.inner.content_offset().y,
            350.0,
            "the child holds its bound"
        );
        assert_eq!(
            fixture.outer.content_offset(),
            Point::default(),
            "a continuation never chains into the parent"
        );

        // A fresh stream over the parent releases the child's ownership.
        fixture.wheel(
            0.0,
            -13.0,
            220.0,
            continuous(
                cove_platform::ScrollPhase::Began,
                cove_platform::ScrollPhase::None,
            ),
        );
        assert_eq!(
            fixture.outer.content_offset().y,
            13.0,
            "the parent takes a fresh stream"
        );
        let held = fixture.outer.content_offset();
        for _ in 0..120 {
            cove_core::scene::AnimatorScheduler::current().tick(1.0 / 60.0);
        }
        assert_eq!(
            fixture.outer.content_offset(),
            held,
            "no fling is synthesized"
        );
        assert_eq!(
            fixture.inner.content_offset().y,
            350.0,
            "the child stays where it was"
        );
    });
}

#[test]
fn a_child_without_overflow_lets_the_parent_scroll() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new(100.0, ScrollAxis::Vertical, None);
        fixture.wheel(0.0, -1.0, 40.0, ScrollInput::default());
        assert_eq!(
            fixture.inner.content_offset(),
            Point::default(),
            "a child that cannot scroll does not move"
        );
        assert_eq!(
            fixture.outer.content_offset().y,
            30.0,
            "the parent takes the input instead"
        );

        let isolated = Fixture::new(
            100.0,
            ScrollAxis::Vertical,
            Some(ScrollConsumePolicy::Always),
        );
        isolated.wheel(0.0, -1.0, 40.0, ScrollInput::default());
        assert_eq!(
            isolated.inner.content_offset(),
            Point::default(),
            "an `Always` child keeps its own offset"
        );
        assert_eq!(
            isolated.outer.content_offset(),
            Point::default(),
            "and contains the input rather than passing it up"
        );
    });
}

#[test]
fn a_horizontal_child_only_contains_its_overflowing_axis() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new(100.0, ScrollAxis::Horizontal, None);
        fixture.wheel(0.0, -1.0, 40.0, ScrollInput::default());
        assert_eq!(
            fixture.inner.content_offset(),
            Point::default(),
            "vertical input never moves a horizontal child"
        );
        assert_eq!(
            fixture.outer.content_offset().y,
            30.0,
            "so the parent takes the vertical input"
        );

        fixture.wheel(-100.0, 0.0, 40.0, ScrollInput::default());
        let bound = fixture.inner.content_offset().x;
        assert_eq!(bound, 300.0, "the child scrolls its own horizontal extent");
        fixture.wheel(-1.0, 0.0, 40.0, ScrollInput::default());
        assert_eq!(
            fixture.inner.content_offset().x,
            bound,
            "and clamps at its horizontal bound"
        );
        assert_eq!(
            fixture.outer.content_offset().y,
            30.0,
            "the parent keeps its vertical offset throughout"
        );
    });
}

#[test]
fn offset_change_policy_enables_same_axis_chaining() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new(
            500.0,
            ScrollAxis::Vertical,
            Some(ScrollConsumePolicy::WhenOffsetChanged),
        );
        fixture.wheel(0.0, -100.0, 40.0, ScrollInput::default());
        assert_eq!(
            fixture.inner.content_offset().y,
            350.0,
            "the child scrolls to its bound"
        );
        assert_eq!(
            fixture.outer.content_offset(),
            Point::default(),
            "the wheel that changed the offset is consumed by the child"
        );

        fixture.wheel(0.0, -1.0, 40.0, ScrollInput::default());
        assert_eq!(
            fixture.inner.content_offset().y,
            350.0,
            "the child stays clamped"
        );
        assert_eq!(
            fixture.outer.content_offset().y,
            30.0,
            "an unchanged offset chains the next wheel to the parent"
        );
    });
}
