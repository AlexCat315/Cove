//! Translation of `Tests/CoveUIComposeTests/Scrolling/ScrollPhysicsTests.swift`.
//!
//! Behaviour checklist:
//! - line wheel input clamps at both ends with no elastic motion, even under
//!   repeated input and after silence
//! - line wheel bursts never infer motion after silence
//! - precise points and OS momentum are followed exactly with no second fling
//! - page units scroll by the logical viewport
//! - a notch, or a press anywhere inside, stops coasting content for good
//! - system momentum at the edge never starts a return spring
//! - slow notches, `.precise` and `.clamped` physics land each notch exactly
//! - scrollbar thumb drags clamp to the absolute range and stay where released
//! - `.scroll_physics(.clamped)` clamps hard; `.bouncy` gestures overshoot and
//!   spring back
//! - a press cancels a pending application-gesture throw
//! - unmounting the scrolling node releases its scheduler entry
//! - a nested scroll edge consumes outward input while its axis overflows
//! - reduced motion disables momentum and rubber band

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, HorizontalAlignment, LayoutDebugExt, TupleView, ViewGraph};
use cove_core::Point;
use cove_core::scene::{
    Align, AnimatorScheduler, EventPhase, EventResult, FlexDirection, InputHandlerRole,
    InteractionRegistry, InteractionRegistryHolder, Justify, Node, PointerCapture,
    PointerCaptureHolder, PointerPhase, ScrollAnimator, ScrollPhysics, Wrap,
};
use cove_platform::{
    MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, ScrollDevice, ScrollInput,
    ScrollPhase, ScrollUnit,
};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::scrolling::ScrollPhysicsExt;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;
use cove_ui_compose::{BoxView, Column};

use support::GlobalTestLock;

const MOTION: &str = "__scrollview_motion";

struct Fixture {
    view: Node,
    animator: Rc<ScrollAnimator>,
    clock: Rc<Cell<f64>>,
    registry: Rc<InteractionRegistry>,
    _graph: Rc<ViewGraph>,
}

impl Fixture {
    fn advance(&self, seconds: f64) {
        self.clock.set(self.clock.get() + seconds);
    }

    fn wheel(&self, y: f32) {
        self.wheel_with(y, ScrollInput::default());
    }

    fn wheel_with(&self, y: f32, input: ScrollInput) {
        self.advance(1.0 / 60.0);
        let handler = self
            .registry
            .handlers(&self.view)
            .wheel
            .expect("wheel handler");
        handler(
            &MouseWheelEvent {
                x: 0.0,
                y,
                mouse_x: None,
                mouse_y: None,
                input,
            },
            EventPhase::Target,
        );
    }

    fn pointer(&self, x: f32, y: f32, phase: PointerPhase) -> EventResult {
        let handler = self
            .registry
            .handlers(&self.view)
            .pointer
            .expect("pointer handler");
        handler(&button(x, y), phase, EventPhase::Target)
    }

    fn motion(&self, x: f32, y: f32, delta_y: f32) {
        self.advance(1.0 / 60.0);
        let handler = self
            .registry
            .handlers(&self.view)
            .motion
            .expect("motion handler");
        handler(
            &MouseMotionEvent {
                x,
                y,
                delta_x: 0.0,
                delta_y,
                kind: Default::default(),
            },
            EventPhase::Target,
        );
    }

    fn tick(&self, frames: usize) {
        let interval = 1.0 / 60.0;
        for _ in 0..frames {
            self.advance(interval);
            AnimatorScheduler::current().tick(interval);
        }
    }

    fn content_offset(&self) -> f32 {
        self.view.content_offset().y
    }

    fn is_finished(&self) -> bool {
        cove_core::scene::AnyAnimationController::is_finished(&*self.animator)
    }
}

fn button(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn input(
    unit: ScrollUnit,
    device: ScrollDevice,
    phase: ScrollPhase,
    momentum: ScrollPhase,
) -> ScrollInput {
    ScrollInput {
        unit,
        device,
        phase,
        momentum_phase: momentum,
        timestamp: 0.0,
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn nodes(graph: &ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

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

/// Installs `rows` rows of 100pt in a 150pt viewport.
fn installed_scroll_view(physics: Option<ScrollPhysics>, rows: usize) -> Fixture {
    let registry = Rc::new(InteractionRegistry::new());
    InteractionRegistryHolder::set_current(Some(registry.clone()));
    PointerCaptureHolder::set_current(Some(Rc::new(PointerCapture::new())));

    let graph = support::graph();
    let scroll = ScrollView::new(
        ScrollAxis::Vertical,
        Column::new(
            HorizontalAlignment::Center,
            0.0,
            AnyView::new(Text::new("content").frame_size(200.0, rows as f32 * 100.0)),
        ),
    )
    .frame_size(200.0, 150.0);
    match physics {
        Some(physics) => graph.install(scroll.scroll_physics(physics), Default::default()),
        None => graph.install(scroll, Default::default()),
    }
    graph.recomposer.commit_all();
    graph.compute_layout(200.0, 400.0);

    let view = nodes(&graph)
        .into_iter()
        .find(|node| {
            registry
                .handlers(node)
                .wheel_route
                .is_some_and(|route| route.role == InputHandlerRole::Scroll)
        })
        .expect("ScrollView did not register a wheel handler");
    let animator = view
        .attachment::<Rc<ScrollAnimator>>(MOTION)
        .expect("ScrollView did not attach a motion animator");
    let clock = Rc::new(Cell::new(0.0));
    let read = clock.clone();
    animator.set_clock(Rc::new(move || read.get()));
    Fixture {
        view,
        animator,
        clock,
        registry,
        _graph: graph,
    }
}

fn teardown() {
    InteractionRegistryHolder::set_current(None);
    PointerCaptureHolder::set_current(None);
}

fn locked(body: impl FnOnce()) {
    GlobalTestLock::with_scheduler(|| {
        body();
        teardown();
    });
}

#[test]
fn wheel_clamps_at_end() {
    locked(|| {
        let fixture = installed_scroll_view(None, 3);
        for _ in 0..6 {
            fixture.wheel(-1.0);
        }
        assert_eq!(fixture.content_offset(), 150.0);
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), 150.0);
    });
}

#[test]
fn wheel_clamps_at_start() {
    locked(|| {
        let fixture = installed_scroll_view(None, 3);
        for _ in 0..2 {
            fixture.wheel(1.0);
        }
        assert_eq!(fixture.content_offset(), 0.0);
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), 0.0);
    });
}

#[test]
fn repeated_input_stays_bounded() {
    locked(|| {
        let fixture = installed_scroll_view(None, 3);
        for _ in 0..5 {
            fixture.wheel(-1.0);
        }
        assert_eq!(fixture.content_offset(), 150.0);
        for _ in 0..40 {
            fixture.wheel(-1.0);
            fixture.tick(2);
            assert_eq!(fixture.content_offset(), 150.0);
            assert!(fixture.content_offset() < 300.0);
        }
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), 150.0);
    });
}

#[test]
fn wheel_stops_with_input() {
    locked(|| {
        let fixture = installed_scroll_view(None, 40);
        let mut expected = 0.0;
        for gap in [0.016_f64, 0.018, 0.016, 0.12, 0.016, 0.15] {
            fixture.wheel(-1.0);
            expected += 30.0;
            fixture.tick(((gap * 60.0).ceil() as usize).max(1));
            assert_eq!(fixture.content_offset(), expected, "after gap {gap}");
            assert!(fixture.is_finished(), "no motion after gap {gap}");
        }
        let delivered = fixture.content_offset();
        assert_eq!(delivered, 180.0);
        fixture.tick(600);
        assert_eq!(fixture.content_offset(), delivered);
    });
}

#[test]
fn platform_momentum_is_not_synthesized_again() {
    locked(|| {
        let fixture = installed_scroll_view(None, 40);
        fixture.wheel_with(
            -12.5,
            input(
                ScrollUnit::Points,
                ScrollDevice::Continuous,
                ScrollPhase::Began,
                ScrollPhase::None,
            ),
        );
        assert_eq!(fixture.content_offset(), 12.5);
        for delta in [-8.0, -4.0, -2.0, -0.5] {
            fixture.wheel_with(
                delta,
                input(
                    ScrollUnit::Points,
                    ScrollDevice::Continuous,
                    ScrollPhase::None,
                    ScrollPhase::Changed,
                ),
            );
        }
        fixture.wheel_with(
            0.0,
            input(
                ScrollUnit::Points,
                ScrollDevice::Unknown,
                ScrollPhase::None,
                ScrollPhase::Ended,
            ),
        );
        assert_eq!(fixture.content_offset(), 27.0);
        fixture.tick(600);
        assert_eq!(fixture.content_offset(), 27.0);
        assert!(fixture.is_finished());
        fixture.wheel_with(
            2.25,
            input(
                ScrollUnit::Points,
                ScrollDevice::Unknown,
                ScrollPhase::Began,
                ScrollPhase::None,
            ),
        );
        assert_eq!(fixture.content_offset(), 24.75);
    });
}

#[test]
fn page_units() {
    locked(|| {
        let fixture = installed_scroll_view(None, 40);
        fixture.wheel_with(
            -1.0,
            input(
                ScrollUnit::Pages,
                ScrollDevice::Unknown,
                ScrollPhase::None,
                ScrollPhase::None,
            ),
        );
        assert_eq!(fixture.content_offset(), 150.0);
    });
}

fn start_coast(fixture: &Fixture, step: f32) {
    for _ in 0..5 {
        fixture.advance(1.0 / 60.0);
        fixture.animator.scroll_by(Point::new(0.0, step));
    }
}

#[test]
fn notch_during_coast_stops_it() {
    locked(|| {
        let fixture = installed_scroll_view(None, 40);
        start_coast(&fixture, 30.0);
        fixture.tick(30);
        assert!(
            fixture.animator.is_gliding(),
            "the gesture launched a coast"
        );

        fixture.wheel(-1.0);
        let braked = fixture.content_offset();
        assert!(!fixture.animator.is_gliding(), "a notch stops the coast");

        fixture.tick(600);
        assert_eq!(fixture.content_offset(), braked);
    });
}

#[test]
fn press_stops_the_coast() {
    locked(|| {
        let fixture = installed_scroll_view(None, 40);
        start_coast(&fixture, 30.0);
        fixture.tick(30);
        assert!(fixture.animator.is_gliding());
        let origin = fixture.view.absolute_origin();

        let result = fixture.pointer(origin.x + 40.0, origin.y + 40.0, PointerPhase::Down);
        assert_ne!(
            result,
            EventResult::Handled,
            "the press still reaches the content"
        );
        assert!(!fixture.animator.is_gliding());
        let caught = fixture.content_offset();

        fixture.tick(200);
        assert_eq!(fixture.content_offset(), caught);
    });
}

#[test]
fn platform_momentum_stops_at_edge() {
    locked(|| {
        let fixture = installed_scroll_view(None, 4);
        for _ in 0..20 {
            fixture.wheel_with(
                -30.0,
                input(
                    ScrollUnit::Points,
                    ScrollDevice::Unknown,
                    ScrollPhase::None,
                    ScrollPhase::Changed,
                ),
            );
            fixture.tick(1);
            assert!(
                fixture.content_offset() <= 250.0,
                "offset {}",
                fixture.content_offset()
            );
            assert!(!fixture.animator.is_settling());
        }
        assert_eq!(fixture.content_offset(), 250.0);
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), 250.0);
    });
}

#[test]
fn slow_notches_do_not_coast() {
    locked(|| {
        let fixture = installed_scroll_view(None, 40);
        fixture.advance(0.5);
        fixture.wheel(-1.0);
        fixture.advance(0.5);
        fixture.wheel(-1.0);
        fixture.tick(600);
        assert_eq!(fixture.content_offset(), 60.0);
    });
}

#[test]
fn precise_does_not_coast() {
    locked(|| {
        let fixture = installed_scroll_view(Some(ScrollPhysics::precise()), 40);
        for _ in 0..3 {
            fixture.wheel(-1.0);
        }
        fixture.tick(600);
        assert_eq!(fixture.content_offset(), 90.0);
    });
}

#[test]
fn clamped_stops_at_the_edge() {
    locked(|| {
        let fixture = installed_scroll_view(Some(ScrollPhysics::clamped()), 4);
        for _ in 0..3 {
            fixture.wheel(-1.0);
        }
        let maximum_offset = 4.0 * 100.0 - 150.0;
        for _ in 0..600 {
            fixture.tick(1);
            assert!(fixture.content_offset() <= maximum_offset);
        }
        assert_eq!(fixture.content_offset(), 90.0);
    });
}

#[test]
fn scrollbar_drag_clamps() {
    locked(|| {
        let fixture = installed_scroll_view(None, 3);
        let origin = fixture.view.absolute_origin();
        let scrollbar_x = origin.x + fixture.view.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;

        assert_eq!(
            fixture.pointer(scrollbar_x, thumb_y, PointerPhase::Down),
            EventResult::Handled,
            "the thumb press is handled"
        );
        fixture.motion(scrollbar_x, thumb_y + 500.0, 500.0);

        let stretched = fixture.content_offset();
        assert_eq!(stretched, 150.0);
        assert!(stretched < 300.0);

        fixture.tick(30);
        assert_eq!(fixture.content_offset(), stretched);

        fixture.pointer(scrollbar_x, thumb_y + 500.0, PointerPhase::Up);
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), 150.0);
    });
}

#[test]
fn scrollbar_release_stays_placed() {
    locked(|| {
        let fixture = installed_scroll_view(None, 10);
        let origin = fixture.view.absolute_origin();
        let scrollbar_x = origin.x + fixture.view.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;

        assert_eq!(
            fixture.pointer(scrollbar_x, thumb_y, PointerPhase::Down),
            EventResult::Handled
        );
        for step in 1..=8 {
            fixture.motion(scrollbar_x, thumb_y + step as f32 * 2.0, 2.0);
        }
        let released = fixture.content_offset();
        assert!(released > 0.0, "the drag moved the content ({released})");
        assert!(released < 10.0 * 100.0 - 150.0);

        fixture.pointer(scrollbar_x, thumb_y + 16.0, PointerPhase::Up);
        assert!(!fixture.animator.is_gliding());
        fixture.tick(900);
        assert_eq!(fixture.content_offset(), released);
        assert!(fixture.is_finished());
    });
}

#[test]
fn clamped_physics() {
    locked(|| {
        let fixture = installed_scroll_view(Some(ScrollPhysics::clamped()), 3);
        for _ in 0..100 {
            fixture.wheel(-1.0);
        }
        assert_eq!(fixture.content_offset(), 150.0);
        for _ in 0..100 {
            fixture.wheel(1.0);
        }
        assert_eq!(fixture.content_offset(), 0.0);
    });
}

#[test]
fn bouncy_physics() {
    locked(|| {
        let fixture = installed_scroll_view(Some(ScrollPhysics::bouncy()), 3);
        for _ in 0..6 {
            fixture.animator.scroll_by(Point::new(0.0, 30.0));
        }
        assert!(
            fixture.content_offset() > 150.0,
            "offset {}",
            fixture.content_offset()
        );

        let mut dipped_below_bound = false;
        for _ in 0..180 {
            fixture.tick(1);
            if fixture.content_offset() < 149.5 {
                dipped_below_bound = true;
            }
        }
        assert!(dipped_below_bound, "the bouncy spring overshoots the bound");
        assert_eq!(fixture.content_offset(), 150.0);
    });
}

#[test]
fn pointer_cancels_pending_throw() {
    locked(|| {
        let fixture = installed_scroll_view(None, 20);
        for _ in 0..6 {
            fixture.advance(1.0 / 60.0);
            fixture.animator.scroll_by(Point::new(0.0, 6.0));
        }
        let held = fixture.content_offset();
        fixture.pointer(60.0, 60.0, PointerPhase::Down);
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), held);
        assert!(fixture.is_finished());
    });
}

#[test]
fn unmount_cancels_motion() {
    locked(|| {
        let scheduler = AnimatorScheduler::current();
        let fixture = installed_scroll_view(None, 20);
        for _ in 0..6 {
            fixture.advance(1.0 / 60.0);
            fixture.animator.scroll_by(Point::new(0.0, 6.0));
        }
        assert_eq!(scheduler.active_count(), 1);
        let parent = fixture.view.parent().unwrap_or_else(|| {
            let parent = Node::new();
            parent.add_child(&fixture.view);
            parent
        });
        parent.remove_child(&fixture.view);
        assert_eq!(scheduler.active_count(), 0);
        let held = fixture.content_offset();
        fixture.tick(240);
        assert_eq!(fixture.content_offset(), held);
    });
}

#[test]
fn nested_boundary_contains_input() {
    locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            ScrollView::new(ScrollAxis::Vertical, empty_box().frame_height(500.0))
                                .frame_height(150.0)
                                .debug_name("inner-scroll"),
                        ),
                        AnyView::new(empty_box().frame_height(600.0)),
                    ])),
                )
                .flex_with(0.0, 0.0, None),
            )
            .frame_size(200.0, 300.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);
        let inner = nodes(&graph)
            .into_iter()
            .find(|node| support::debug_name(node).as_deref() == Some("inner-scroll"))
            .expect("inner scroll");
        let wheel = |y: f32| {
            let handler = registry
                .handlers(&inner)
                .wheel
                .expect("inner wheel handler");
            handler(
                &MouseWheelEvent {
                    x: 0.0,
                    y,
                    ..Default::default()
                },
                EventPhase::Target,
            )
        };
        assert_eq!(wheel(1.0), EventResult::Handled);
        assert_eq!(inner.content_offset(), Point::default());
        assert_eq!(wheel(-1.0), EventResult::Handled);
        assert!(inner.content_offset().y > 0.0);
    });
}

#[test]
fn reduced_motion_scrolls_precisely() {
    locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let graph = support::graph();
        graph.install(
            ScrollView::new(ScrollAxis::Vertical, empty_box().frame_height(800.0))
                .frame_size(200.0, 150.0)
                .reduced_motion(true),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 150.0);
        let node = nodes(&graph)
            .into_iter()
            .find(|node| registry.handlers(node).wheel.is_some())
            .expect("wheel node");
        let animator = node
            .attachment::<Rc<ScrollAnimator>>(MOTION)
            .expect("motion animator");
        assert!(!animator.physics().momentum.is_enabled);
        assert!(!animator.physics().rubber_band.is_enabled);
        for _ in 0..6 {
            let handler = registry.handlers(&node).wheel.expect("wheel");
            handler(
                &MouseWheelEvent {
                    x: 0.0,
                    y: -1.0,
                    ..Default::default()
                },
                EventPhase::Target,
            );
        }
        assert!(cove_core::scene::AnyAnimationController::is_finished(
            &*animator
        ));
        assert_eq!(node.content_offset().y, 180.0);
    });
}
