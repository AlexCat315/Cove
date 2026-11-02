//! Translation of `Tests/CoveUIComposeTests/SliderAxisTests.swift`.
//!
//! Behaviour checklist:
//! - an intentional slider drag keeps pointer capture inside a touch scroll
//!   container, and the scroll view does not move
//! - vertical sliders use the Y axis, keep a touch-sized track and preserve
//!   value semantics for pointer, keyboard and assistive actions
//! - a horizontal RTL slider maps the leading edge to zero and reverses the
//!   horizontal keys

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, ViewGraph};
use cove_core::scene::Direction;
use cove_core::scene::{
    AccessibilityRole, EventDispatcher, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, NodeTree, PlatformInputContext, PointerCapture,
    PointerCaptureHolder, Recomposer,
};
use cove_core::{Binding, Environment, InputCapabilities, Point, Rect};
use cove_platform::{
    InputEvent, KeyEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, WindowId,
};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::slider::geometry::SliderAxis;
use cove_ui_compose::primitives::controls::fields::slider::slider::Slider;
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView};
use cove_ui_compose::{BoxView, Scancode};

use support::GlobalTestLock;

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn nodes(graph: &Rc<ViewGraph>) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        all_nodes(&root, &mut collected);
    }
    collected
}

fn role_node(graph: &Rc<ViewGraph>, role: AccessibilityRole) -> Node {
    nodes(graph)
        .into_iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == role)
        })
        .unwrap_or_else(|| panic!("a node with role {role:?} is mounted"))
}

fn environment(width: f32, input: InputCapabilities) -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = width;
    environment.viewport.height = 300.0;
    environment.input = input;
    environment
}

fn empty_box() -> AnyView {
    AnyView::new(BoxView::new(
        cove_core::scene::FlexDirection::Column,
        cove_core::scene::Align::Stretch,
        cove_core::scene::Justify::FlexStart,
        cove_core::scene::Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    ))
}

fn with_scene(body: impl FnOnce(&PlatformInputContext)) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| body(&context));
        });
    });
}

fn mouse_down(x: f32, y: f32) -> InputEvent {
    InputEvent::MouseButtonDown(MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    })
}

fn mouse_up(x: f32, y: f32) -> InputEvent {
    InputEvent::MouseButtonUp(MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    })
}

fn mouse_move(x: f32, y: f32, dx: f32, dy: f32) -> InputEvent {
    InputEvent::MouseMotion(MouseMotionEvent {
        x,
        y,
        delta_x: dx,
        delta_y: dy,
        kind: Default::default(),
    })
}

#[test]
fn an_exclusive_drag_keeps_capture_inside_a_touch_scroll_container() {
    with_scene(|context| {
        let value = cove_core::State::new(0.0_f64);
        let read = value.clone();
        let write = value.clone();
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            AnyView::new(
                AnyView::new(ScrollView::new(
                    ScrollAxis::Horizontal,
                    AnyView::new(BoxView::new(
                        cove_core::scene::FlexDirection::Row,
                        cove_core::scene::Align::Stretch,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::TupleView::new(vec![
                            AnyView::new(
                                AnyView::new(Slider::new(Binding::new(
                                    move || read.get(),
                                    move |next| write.set(next),
                                )))
                                .frame_width(160.0),
                            ),
                            AnyView::new(empty_box().frame_size(400.0, 44.0)),
                        ])),
                    )),
                ))
                .frame_size(200.0, 60.0)
                .surface_environment(environment(200.0, InputCapabilities::TOUCH)),
            ),
            Default::default(),
        );
        for _ in 0..4 {
            graph.recomposer.commit_all();
            graph.compute_layout(200.0, 60.0);
        }

        let slider = role_node(&graph, AccessibilityRole::Slider);
        let scroll = role_node(&graph, AccessibilityRole::ScrollArea);
        let frame = slider.absolute_frame();
        let x = frame.x;
        let y = frame.y + frame.height / 2.0;

        let dispatcher = EventDispatcher::new(
            graph.tree.clone(),
            context.interactions.clone(),
            context.pointer_capture.clone(),
            context.focus_chain.clone(),
            WindowId::MAIN,
        );
        dispatcher.dispatch(&mouse_down(x + 40.0, y));
        dispatcher.dispatch(&mouse_move(x + 120.0, y, 80.0, 0.0));

        assert_eq!(
            value.get(),
            0.75,
            "dragging a quarter across the track moves a quarter of the range"
        );
        assert_eq!(
            scroll.content_offset().x,
            0.0,
            "the scroll container does not move while the slider is captured"
        );
        assert_eq!(
            context.pointer_capture.target().map(|target| target.id()),
            Some(slider.id()),
            "the slider holds pointer capture"
        );

        dispatcher.dispatch(&mouse_up(x + 120.0, y));
        graph.install(cove_composition::EmptyView, Default::default());
    });
}

#[test]
fn vertical_sliders_use_the_y_axis_and_stay_touch_sized() {
    with_scene(|context| {
        for width in [390.0_f32, 720.0, 1280.0] {
            for input in [
                InputCapabilities::TOUCH,
                InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            ] {
                let value = cove_core::State::new(50.0_f64);
                let read = value.clone();
                let write = value.clone();
                let graph = Rc::new(ViewGraph::new(
                    Rc::new(RefCell::new(NodeTree::new())),
                    Rc::new(Recomposer::new()),
                ));
                graph.install(
                    AnyView::new(
                        AnyView::new(
                            Slider::new(Binding::new(
                                move || read.get(),
                                move |next| write.set(next),
                            ))
                            .range((0.0, 100.0))
                            .step(5.0)
                            .shows_ticks(true)
                            .axis(SliderAxis::Vertical),
                        )
                        .frame_height(180.0)
                        .surface_environment(environment(width, input)),
                    ),
                    Default::default(),
                );
                graph.compute_layout(width, 300.0);

                let host = role_node(&graph, AccessibilityRole::Slider);
                assert_eq!(
                    host.frame().height,
                    180.0,
                    "the vertical track keeps its explicit height"
                );
                let floor = if input.contains(InputCapabilities::TOUCH) {
                    44.0
                } else {
                    24.0
                };
                assert!(
                    host.frame().width >= floor,
                    "the track is at least {floor}pt wide (got {})",
                    host.frame().width
                );

                let dispatcher = EventDispatcher::new(
                    graph.tree.clone(),
                    context.interactions.clone(),
                    context.pointer_capture.clone(),
                    context.focus_chain.clone(),
                    WindowId::MAIN,
                );
                let frame = host.absolute_frame();
                let x = frame.x + frame.width / 2.0;
                let top = frame.y;

                dispatcher.dispatch(&mouse_down(x, top + 45.0));
                assert_eq!(
                    value.get(),
                    75.0,
                    "a press a quarter down the track sets three quarters"
                );

                dispatcher.dispatch(&mouse_move(x, top - 40.0, 0.0, -85.0));
                assert_eq!(value.get(), 100.0, "dragging above the track clamps high");

                dispatcher.dispatch(&mouse_move(x, top + 220.0, 0.0, 260.0));
                assert_eq!(value.get(), 0.0, "dragging below the track clamps low");

                dispatcher.dispatch(&mouse_up(x, top + 220.0));
                assert_eq!(
                    context.pointer_capture.target().map(|target| target.id()),
                    None,
                    "releasing the pointer drops capture"
                );

                context.focus_chain.focus(Some(&host), false);
                dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
                    scancode: Scancode::ARROW_UP,
                    keycode: 0,
                    modifiers: Default::default(),
                    is_repeat: false,
                }));
                assert_eq!(value.get(), 5.0, "ArrowUp steps up by one increment");

                let set_value = host
                    .accessibility_actions()
                    .set_value
                    .expect("the slider accepts a set-value action");
                set_value("40");
                assert_eq!(value.get(), 40.0, "assistive set-value is honoured");

                let increment = host
                    .accessibility_actions()
                    .increment
                    .expect("the slider accepts an increment action");
                increment();
                assert_eq!(value.get(), 45.0, "assistive increment adds one step");

                let mut list = cove_core::DrawList::default();
                if let Some(draw) = host.draw() {
                    draw(&mut list, Point::default());
                }
                assert!(!list.vertices().is_empty(), "the slider paints itself");

                graph.install(cove_composition::EmptyView, Default::default());
            }
        }
    });
}

#[test]
fn horizontal_rtl_maps_the_leading_edge_to_zero_and_reverses_keys() {
    with_scene(|context| {
        let value = cove_core::State::new(0.0_f64);
        let read = value.clone();
        let write = value.clone();
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            AnyView::new(
                AnyView::new(
                    Slider::new(Binding::new(
                        move || read.get(),
                        move |next| write.set(next),
                    ))
                    .range((0.0, 100.0))
                    .step(5.0),
                )
                .frame_width(200.0)
                .layout_direction(Direction::Rtl),
            ),
            Default::default(),
        );
        graph.compute_layout(300.0, 200.0);

        let host = role_node(&graph, AccessibilityRole::Slider);
        let frame = host.absolute_frame();
        let pointer = context
            .interactions
            .handlers(&host)
            .pointer
            .expect("the slider registers a pointer handler");
        let _ = pointer(
            &MouseButtonEvent {
                button: MouseButton::Left,
                x: frame.x + 50.0,
                y: frame.y + frame.height / 2.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            },
            cove_core::scene::PointerPhase::Down,
            cove_core::scene::EventPhase::Target,
        );
        assert_eq!(
            value.get(),
            75.0,
            "in RTL the leading edge is the right edge, so a left press reads high"
        );

        let key = context
            .interactions
            .handlers(&host)
            .key
            .expect("the slider registers a key handler");
        let _ = key(
            &KeyEvent {
                scancode: Scancode::ARROW_RIGHT,
                keycode: 0,
                modifiers: Default::default(),
                is_repeat: false,
            },
            cove_core::scene::EventPhase::Target,
        );
        assert_eq!(value.get(), 70.0, "RTL reverses the horizontal arrows");

        graph.install(cove_composition::EmptyView, Default::default());
        let _ = (
            Rect::new(0.0, 0.0, 0.0, 0.0),
            FocusChain::new(),
            InteractionRegistry::new(),
            PointerCapture::new(),
        );
        let _ = FocusChainHolder::current();
        let _ = InteractionRegistryHolder::current();
        let _ = PointerCaptureHolder::current();
    });
}
