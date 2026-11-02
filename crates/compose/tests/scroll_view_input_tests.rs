//! Translation of `Tests/CoveUIComposeTests/ScrollViewInputTests.swift`.
//!
//! Behaviour checklist:
//! - scroll content accepts conditional branches and repeated children, and every
//!   measured frame stays finite
//! - a global scrollbar hit lane cannot capture an adjacent control, and a
//!   clipped pane cannot reach across its ancestor's clip
//! - a touch drag moves the content and releases into scroll physics
//! - Page/Home/End scrolls a focused viewport
//! - a desktop pointer drag does not scroll content

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, TupleView, ViewGraph};
use cove_core::scene::{
    AccessibilityRole, EventDispatcher, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, NodeTree, PlatformInputContext, PointerCapture,
    PointerCaptureHolder, Recomposer,
};
use cove_core::{Environment, InputCapabilities};
use cove_platform::{
    InputEvent, KeyEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, WindowId,
};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;
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

fn environment(input: InputCapabilities) -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = 200.0;
    environment.viewport.height = 400.0;
    environment.input = input;
    environment
}

/// The scroll viewport node: the one that registered a wheel handler.
fn scroll_node(graph: &Rc<ViewGraph>) -> Node {
    nodes(graph)
        .into_iter()
        .find(|node| {
            InteractionRegistryHolder::current()
                .is_some_and(|registry| registry.handlers(node).wheel.is_some())
        })
        .expect("the scroll viewport registers a wheel handler")
}

fn install(graph: &Rc<ViewGraph>, input: InputCapabilities) -> Scene {
    let scene = Scene::new(graph.clone());
    graph.install(
        AnyView::new(
            ScrollView::new(
                ScrollAxis::Vertical,
                AnyView::new(BoxView::new(
                    cove_core::scene::FlexDirection::Column,
                    cove_core::scene::Align::Stretch,
                    cove_core::scene::Justify::FlexStart,
                    cove_core::scene::Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(AnyView::new(Text::new("a")).frame_height(100.0)),
                        AnyView::new(AnyView::new(Text::new("b")).frame_height(100.0)),
                        AnyView::new(AnyView::new(Text::new("c")).frame_height(100.0)),
                    ])),
                )),
            )
            .surface_environment(environment(input))
            .frame_height(150.0),
        ),
        Default::default(),
    );
    graph.compute_layout(200.0, 400.0);
    scene
}

struct Scene {
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    capture: Rc<PointerCapture>,
}

impl Scene {
    fn new(_graph: Rc<ViewGraph>) -> Self {
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let capture = Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        Self {
            registry,
            focus,
            capture,
        }
    }

    fn pointer(&self, node: &Node) -> cove_core::scene::PointerHandler {
        self.registry
            .handlers(node)
            .pointer
            .expect("the scroll viewport registers a pointer handler")
    }

    fn motion(&self, node: &Node) -> cove_core::scene::MotionHandler {
        self.registry
            .handlers(node)
            .motion
            .expect("the scroll viewport registers a motion handler")
    }

    fn key(&self, node: &Node) -> cove_core::scene::KeyHandler {
        self.registry
            .handlers(node)
            .key
            .expect("the scroll viewport registers a key handler")
    }
}

fn pointer_event(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn motion_event(x: f32, y: f32, dy: f32) -> MouseMotionEvent {
    MouseMotionEvent {
        x,
        y,
        delta_x: 0.0,
        delta_y: dy,
        kind: Default::default(),
    }
}

#[test]
fn scroll_content_accepts_conditional_branches_and_repeated_children() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for empty in [false, true] {
                let graph = Rc::new(ViewGraph::new(
                    Rc::new(RefCell::new(NodeTree::new())),
                    Rc::new(Recomposer::new()),
                ));
                let content = if empty {
                    AnyView::new(Text::new("Empty document"))
                } else {
                    AnyView::new(cove_composition::TupleView::new(
                        (0..3)
                            .map(|index| {
                                AnyView::new(
                                    AnyView::new(Text::new(format!("Section {index}")))
                                        .frame_height(100.0),
                                )
                            })
                            .collect(),
                    ))
                };
                graph.install(
                    AnyView::new(
                        AnyView::new(ScrollView::new(ScrollAxis::Vertical, content))
                            .frame_size(200.0, 120.0),
                    ),
                    Default::default(),
                );
                graph.compute_layout(200.0, 120.0);

                let labels: Vec<String> = nodes(&graph)
                    .into_iter()
                    .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
                    .filter(|label| !label.is_empty())
                    .collect();
                assert_eq!(
                    labels.iter().any(|label| label == "Empty document"),
                    empty,
                    "the conditional branch renders exactly when selected"
                );
                let sections = labels
                    .iter()
                    .filter(|label| label.starts_with("Section "))
                    .count();
                assert_eq!(
                    sections,
                    if empty { 0 } else { 3 },
                    "the repeated children render exactly once each"
                );
                assert!(
                    nodes(&graph)
                        .iter()
                        .all(|node| node.frame().width.is_finite()
                            && node.frame().height.is_finite()),
                    "every measured frame stays finite"
                );

                graph.install(cove_composition::EmptyView, Default::default());
            }
        });
    });
}

#[test]
fn a_scrollbar_lane_cannot_capture_an_adjacent_control() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for clipped in [false, true] {
                let context = PlatformInputContext::new();
                context.with_current(|| {
                    let calls = Rc::new(std::cell::Cell::new(0u32));
                    let pane = if clipped { 80.0_f32 } else { 100.0 };
                    let graph = Rc::new(ViewGraph::new(
                        Rc::new(RefCell::new(NodeTree::new())),
                        Rc::new(Recomposer::new()),
                    ));

                    let sized = AnyView::new(
                        AnyView::new(ScrollView::new(
                            ScrollAxis::Vertical,
                            AnyView::new(empty_box().frame_height(600.0)),
                        ))
                        .frame_size(100.0, 100.0),
                    );
                    let scroll = if clipped {
                        AnyView::new(sized.clipped())
                    } else {
                        sized
                    };

                    let counter = calls.clone();
                    let button = AnyView::new(
                        AnyView::new(Button::new(Text::new("Adjacent"), move || {
                            counter.set(counter.get() + 1)
                        }))
                        .frame_size(44.0, 100.0),
                    );

                    graph.install(
                        AnyView::new(
                            AnyView::new(BoxView::new(
                                cove_core::scene::FlexDirection::Row,
                                cove_core::scene::Align::Stretch,
                                cove_core::scene::Justify::FlexStart,
                                cove_core::scene::Wrap::NoWrap,
                                0.0,
                                AnyView::new(TupleView::new(vec![
                                    AnyView::new(
                                        AnyView::new(BoxView::new(
                                            cove_core::scene::FlexDirection::Column,
                                            cove_core::scene::Align::FlexStart,
                                            cove_core::scene::Justify::FlexStart,
                                            cove_core::scene::Wrap::NoWrap,
                                            0.0,
                                            scroll,
                                        ))
                                        .frame_width(pane),
                                    ),
                                    button,
                                ])),
                            ))
                            .frame_size(pane + 44.0, 100.0),
                        ),
                        Default::default(),
                    );
                    for _ in 0..4 {
                        graph.recomposer.commit_all();
                        graph.compute_layout(pane + 44.0, 100.0);
                    }

                    let dispatcher = EventDispatcher::new(
                        graph.tree.clone(),
                        context.interactions.clone(),
                        context.pointer_capture.clone(),
                        context.focus_chain.clone(),
                        WindowId::MAIN,
                    );
                    let event = pointer_event(pane + 2.0, 50.0);
                    dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
                    assert_eq!(
                        context
                            .pointer_capture
                            .target()
                            .and_then(|node| node.accessibility().map(|s| s.role)),
                        Some(AccessibilityRole::Button),
                        "the pressed lane lands on the adjacent button"
                    );
                    dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
                    assert_eq!(calls.get(), 1, "the button fires exactly once");
                    assert_eq!(
                        context.pointer_capture.target().map(|node| node.id()),
                        None,
                        "the release drops capture"
                    );

                    graph.install(cove_composition::EmptyView, Default::default());
                });
            }
        });
    });
}

#[test]
fn a_touch_drag_moves_the_content_and_releases_into_physics() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let graph = Rc::new(ViewGraph::new(
                    Rc::new(RefCell::new(NodeTree::new())),
                    Rc::new(Recomposer::new()),
                ));
                let scene = install(&graph, InputCapabilities::TOUCH);
                let scroll = scroll_node(&graph);
                let pointer = scene.pointer(&scroll);
                let motion = scene.motion(&scroll);
                let origin = scroll.absolute_origin();
                let x = origin.x + 80.0;
                let y = origin.y + 80.0;

                let _ = pointer(
                    &pointer_event(x, y),
                    cove_core::scene::PointerPhase::Down,
                    cove_core::scene::EventPhase::Target,
                );
                let _ = motion(
                    &motion_event(x, y - 80.0, -80.0),
                    cove_core::scene::EventPhase::Target,
                );
                let _ = pointer(
                    &pointer_event(x, y - 80.0),
                    cove_core::scene::PointerPhase::Up,
                    cove_core::scene::EventPhase::Target,
                );

                assert!(
                    scroll.content_offset().y > 0.0,
                    "a touch drag moves the content (offset {})",
                    scroll.content_offset().y
                );
                assert_eq!(
                    scene.capture.target().map(|node| node.id()),
                    None,
                    "releasing the drag drops capture"
                );

                graph.install(cove_composition::EmptyView, Default::default());
            });
        });
    });
}

#[test]
fn page_home_and_end_scroll_a_focused_viewport() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let graph = Rc::new(ViewGraph::new(
                    Rc::new(RefCell::new(NodeTree::new())),
                    Rc::new(Recomposer::new()),
                ));
                let scene = install(
                    &graph,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                );
                let scroll = scroll_node(&graph);
                scene.focus.focus(Some(&scroll), false);
                let key = scene.key(&scroll);

                let press = |scancode: u32| {
                    let _ = key(
                        &KeyEvent {
                            scancode,
                            keycode: 0,
                            modifiers: Default::default(),
                            is_repeat: false,
                        },
                        cove_core::scene::EventPhase::Target,
                    );
                };

                press(Scancode::PAGE_DOWN);
                assert!(
                    scroll.content_offset().y > 0.0,
                    "Page Down scrolls the viewport"
                );
                let page_offset = scroll.content_offset().y;

                press(Scancode::END);
                assert!(
                    scroll.content_offset().y > page_offset,
                    "End scrolls further than Page Down"
                );

                press(Scancode::HOME);
                assert_eq!(scroll.content_offset().y, 0.0, "Home returns to the top");

                graph.install(cove_composition::EmptyView, Default::default());
            });
        });
    });
}

#[test]
fn a_desktop_pointer_drag_does_not_scroll_content() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let graph = Rc::new(ViewGraph::new(
                    Rc::new(RefCell::new(NodeTree::new())),
                    Rc::new(Recomposer::new()),
                ));
                let scene = install(
                    &graph,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                );
                let scroll = scroll_node(&graph);
                let pointer = scene.pointer(&scroll);
                let motion = scene.motion(&scroll);
                let origin = scroll.absolute_origin();
                let x = origin.x + 80.0;
                let y = origin.y + 80.0;

                let _ = pointer(
                    &pointer_event(x, y),
                    cove_core::scene::PointerPhase::Down,
                    cove_core::scene::EventPhase::Target,
                );
                let _ = motion(
                    &motion_event(x, y - 80.0, -80.0),
                    cove_core::scene::EventPhase::Target,
                );
                let _ = pointer(
                    &pointer_event(x, y - 80.0),
                    cove_core::scene::PointerPhase::Up,
                    cove_core::scene::EventPhase::Target,
                );

                assert_eq!(
                    scroll.content_offset().y,
                    0.0,
                    "a desktop pointer drag never scrolls content"
                );

                graph.install(cove_composition::EmptyView, Default::default());
            });
        });
    });
}
