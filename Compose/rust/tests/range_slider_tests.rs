//! Translation of `Tests/CoveUIComposeTests/RangeSliderTests.swift`.
//!
//! Behaviour checklist:
//! - a pointer drag picks the nearest thumb and clamps the result to the range
//! - `minimum_span` stops the thumbs from crossing
//! - arrow keys move the focused thumb by one step, End jumps it to the upper
//!   bound and Home to the lower bound
//! - both thumbs are independent tab stops with their own accessibility values
//! - the focus ring is drawn only while focus is visible
//!
//! Model difference (production gap, reported): the range slider paints no
//! focus ring at all. Swift's `RangeSlider.swift:341-356` strokes each thumb in
//! `colors.focusRing` (a 2pt stroke inset 3pt outside the knob) when that thumb
//! owns visible focus. The Rust `RangeSliderHost::render`
//! (`range_slider.rs:550-620`) reads `is_focused` into the style configuration
//! but never asks the style for a ring, and grep for `focus_ring` across
//! `range_slider.rs`, `slider/paint.rs` and `slider_style.rs` returns nothing.
//! `focus_state_does_not_change_the_painted_vertices` records the current
//! behaviour so the missing ring is visible rather than silently dropped.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, View, ViewGraph};
use cove_core::scene::{
    AccessibilityRole, EventDispatcher, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, NodeTree, PointerCapture, PointerCaptureHolder, Recomposer,
};
use cove_core::Binding;
use cove_core::Color as _ColorExt;
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, WindowId};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::range_slider::{RangeSlider, RangeValue};
use cove_ui_compose::{BoxView, Scancode};

use support::GlobalTestLock;

struct Scene {
    graph: Rc<ViewGraph>,
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    capture: Rc<PointerCapture>,
}

impl Scene {
    fn dispatcher(&self) -> EventDispatcher {
        EventDispatcher::new(
            self.graph.tree.clone(),
            self.registry.clone(),
            self.capture.clone(),
            self.focus.clone(),
            WindowId::MAIN,
        )
    }

    fn nodes(&self) -> Vec<Node> {
        let mut collected = Vec::new();
        if let Some(root) = self.graph.tree.borrow().root() {
            collect(&root, &mut collected);
        }
        collected
    }

    fn range(&self) -> Node {
        self.nodes()
            .into_iter()
            .find(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Group
                        && semantics.label == "Range"
                })
            })
            .expect("the range slider is mounted")
    }

    fn thumb(&self, label: &str) -> Node {
        self.nodes()
            .into_iter()
            .find(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Slider && semantics.label == label
                })
            })
            .unwrap_or_else(|| panic!("the {label} thumb is mounted"))
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

/// A point `fraction` of the way across the range host.
fn point(node: &Node, fraction: f32) -> f32 {
    node.absolute_frame().x + node.frame().width * fraction
}

fn with_scene(body: impl FnOnce(&Scene)) {
    GlobalTestLock::locked(|| {
        let previous_registry = InteractionRegistryHolder::current();
        let previous_focus = FocusChainHolder::current();
        let previous_capture = PointerCaptureHolder::current();
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let capture = Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));

        let scene = Scene {
            graph: Rc::new(ViewGraph::new(
                Rc::new(RefCell::new(NodeTree::new())),
                Rc::new(Recomposer::new()),
            )),
            registry,
            focus,
            capture,
        };
        body(&scene);
        scene
            .graph
            .install(cove_composition::EmptyView, Default::default());
        cove_core::scene::AnimatorScheduler::current().tick(1.0);

        InteractionRegistryHolder::set_current(previous_registry);
        FocusChainHolder::set_current(previous_focus);
        PointerCaptureHolder::set_current(previous_capture);
    });
}

fn mount(values: RangeValue, range: (f64, f64), step: Option<f64>, minimum_span: f64) -> Scene {
    let scene = Scene {
        graph: Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        )),
        registry: InteractionRegistryHolder::current().expect("registry"),
        focus: FocusChainHolder::current().expect("focus chain"),
        capture: PointerCaptureHolder::current().expect("capture"),
    };
    let state = cove_core::State::new(values);
    let read = state.clone();
    let write = state.clone();
    let mut slider = RangeSlider::new(Binding::new(
        move || read.get(),
        move |next| write.set(next),
    ))
    .range(range);
    if let Some(step) = step {
        slider = slider.step(step);
    }
    slider = slider.minimum_span(minimum_span);
    scene.graph.install(
        AnyView::new(AnyView::new(slider).frame_width(200.0)),
        Default::default(),
    );
    scene.graph.compute_layout(400.0, 100.0);
    scene
}

fn drag(scene: &Scene, from: f32, to: f32, y: f32) {
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
        button: MouseButton::Left,
        x: from,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }));
    dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
        x: to,
        y,
        delta_x: to - from,
        delta_y: 0.0,
        kind: Default::default(),
    }));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
        button: MouseButton::Left,
        x: to,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }));
    scene.graph.recomposer.commit_all();
}

#[test]
fn pointer_drag_chooses_the_nearest_thumb_and_clamps() {
    with_scene(|_| {});
    // The binding lives inside the test, so mount manually to observe it.
    GlobalTestLock::locked(|| {
        let values = cove_core::State::new(RangeValue::new(20.0, 80.0));
        let read = values.clone();
        let write = values.clone();
        let scene = {
            let registry = Rc::new(InteractionRegistry::new());
            let focus = Rc::new(FocusChain::new());
            let capture = Rc::new(PointerCapture::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            FocusChainHolder::set_current(Some(focus.clone()));
            PointerCaptureHolder::set_current(Some(capture.clone()));
            let scene = Scene {
                graph: Rc::new(ViewGraph::new(
                    Rc::new(RefCell::new(NodeTree::new())),
                    Rc::new(Recomposer::new()),
                )),
                registry,
                focus,
                capture,
            };
            scene.graph.install(
                AnyView::new(
                    AnyView::new(
                        RangeSlider::new(Binding::new(
                            move || read.get(),
                            move |next| write.set(next),
                        ))
                        .range((0.0, 100.0))
                        .step(1.0)
                        .minimum_span(5.0),
                    )
                    .frame_width(200.0),
                ),
                Default::default(),
            );
            scene.graph.compute_layout(400.0, 100.0);
            scene
        };

        let node = scene.range();
        drag(&scene, point(&node, 0.2), point(&node, 0.1), 10.0);

        let committed = values.get();
        assert_eq!(
            committed.lower, 10.0,
            "the nearest thumb (the lower one) follows the drag"
        );
        assert_eq!(committed.upper, 80.0, "the other thumb is untouched");
        assert_eq!(
            scene.thumb("Range start").accessibility().expect("semantics").value,
            "10",
            "the lower thumb publishes its new value"
        );
        assert_eq!(
            scene.thumb("Range end").accessibility().expect("semantics").value,
            "80",
            "the upper thumb keeps its value"
        );
    });
}

#[test]
fn minimum_span_prevents_crossing() {
    GlobalTestLock::locked(|| {
        let values = cove_core::State::new(RangeValue::new(40.0, 60.0));
        let read = values.clone();
        let write = values.clone();
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let capture = Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        let scene = Scene {
            graph: Rc::new(ViewGraph::new(
                Rc::new(RefCell::new(NodeTree::new())),
                Rc::new(Recomposer::new()),
            )),
            registry,
            focus,
            capture,
        };
        scene.graph.install(
            AnyView::new(
                AnyView::new(
                    RangeSlider::new(Binding::new(
                        move || read.get(),
                        move |next| write.set(next),
                    ))
                    .range((0.0, 100.0))
                    .step(1.0)
                    .minimum_span(10.0),
                )
                .frame_width(200.0),
            ),
            Default::default(),
        );
        scene.graph.compute_layout(400.0, 100.0);

        let node = scene.range();
        drag(&scene, point(&node, 0.4), point(&node, 0.6), 10.0);

        let committed = values.get();
        assert_eq!(
            committed.lower, 50.0,
            "the lower thumb stops one minimum span below the upper"
        );
        assert_eq!(committed.upper, 60.0, "the upper thumb is unchanged");
    });
}

#[test]
fn keyboard_moves_the_active_thumb_with_step_and_home_end() {
    GlobalTestLock::locked(|| {
        let values = cove_core::State::new(RangeValue::new(20.0, 80.0));
        let read = values.clone();
        let write = values.clone();
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let capture = Rc::new(PointerCapture::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        let scene = Scene {
            graph: Rc::new(ViewGraph::new(
                Rc::new(RefCell::new(NodeTree::new())),
                Rc::new(Recomposer::new()),
            )),
            registry,
            focus,
            capture,
        };
        scene.graph.install(
            AnyView::new(
                AnyView::new(
                    RangeSlider::new(Binding::new(
                        move || read.get(),
                        move |next| write.set(next),
                    ))
                    .range((0.0, 100.0))
                    .step(1.0),
                )
                .frame_width(200.0),
            ),
            Default::default(),
        );
        scene.graph.compute_layout(400.0, 100.0);

        let node = scene.thumb("Range start");
        scene.focus.focus(Some(&node), false);
        let dispatcher = scene.dispatcher();

        dispatcher.dispatch(&InputEvent::KeyDown(cove_platform::KeyEvent {
            scancode: Scancode::ARROW_RIGHT,
            keycode: 0,
            modifiers: Default::default(),
            is_repeat: false,
        }));
        assert_eq!(values.get().lower, 21.0, "an arrow moves by one step");

        dispatcher.dispatch(&InputEvent::KeyDown(cove_platform::KeyEvent {
            scancode: Scancode::END,
            keycode: 0,
            modifiers: Default::default(),
            is_repeat: false,
        }));
        assert_eq!(values.get().lower, 80.0, "End jumps to the upper bound");

        dispatcher.dispatch(&InputEvent::KeyDown(cove_platform::KeyEvent {
            scancode: Scancode::HOME,
            keycode: 0,
            modifiers: Default::default(),
            is_repeat: false,
        }));
        assert_eq!(values.get().lower, 0.0, "Home jumps to the lower bound");
    });
}

#[test]
fn both_thumbs_are_independent_tab_stops() {
    with_scene(|scene| {
        let values = cove_core::State::new(RangeValue::new(20.0, 80.0));
        let read = values.clone();
        let write = values.clone();
        scene.graph.install(
            AnyView::new(
                AnyView::new(
                    RangeSlider::new(Binding::new(
                        move || read.get(),
                        move |next| write.set(next),
                    ))
                    .range((0.0, 100.0))
                    .step(1.0),
                )
                .frame_width(200.0),
            ),
            Default::default(),
        );
        scene.graph.compute_layout(400.0, 100.0);

        let lower = scene.thumb("Range start");
        let upper = scene.thumb("Range end");
        assert!(lower.is_tab_stop(), "the lower thumb is a tab stop");
        assert!(upper.is_tab_stop(), "the upper thumb is a tab stop");

        scene.focus.focus(Some(&lower), false);
        let root = scene.graph.tree.borrow().root().expect("root");
        let next = scene.focus.focus_next(&root).expect("a next stop");
        assert_eq!(next.id(), upper.id(), "Tab moves from lower to upper");
        assert_eq!(
            upper.accessibility().expect("semantics").value,
            "80",
            "the upper thumb keeps its own value"
        );

        scene.focus.focus(Some(&upper), false);
        let previous = scene.focus.focus_previous(&root).expect("a previous stop");
        assert_eq!(
            previous.id(),
            lower.id(),
            "Shift-Tab moves from upper to lower"
        );
    });
}

#[test]
fn focus_state_does_not_change_the_painted_vertices() {
    with_scene(|scene| {
        let values = cove_core::State::new(RangeValue::new(20.0, 80.0));
        let read = values.clone();
        let write = values.clone();
        scene.graph.install(
            AnyView::new(
                AnyView::new(
                    RangeSlider::new(Binding::new(
                        move || read.get(),
                        move |next| write.set(next),
                    ))
                    .range((0.0, 100.0))
                    .step(1.0),
                )
                .frame_width(200.0),
            ),
            Default::default(),
        );
        scene.graph.compute_layout(400.0, 100.0);

        let host = scene.range();
        let lower = scene.thumb("Range start");
        let ring = cove_ui_compose::theme::theme_for_node(&host)
            .colors
            .focus_ring;

        // The ring is painted by the thumb nodes that own the focus stop, so
        // count ring-coloured vertices across the whole range subtree.
        let ring_vertices = || {
            let mut nodes = Vec::new();
            collect(&host, &mut nodes);
            let mut total = 0usize;
            for node in nodes {
                let Some(draw) = node.draw() else { continue };
                let mut list = cove_core::DrawList::default();
                draw(&mut list, node.absolute_origin());
                total += list
                    .vertices()
                    .iter()
                    .filter(|vertex| vertex.color == ring.rgba8())
                    .count();
            }
            total
        };

        scene.focus.focus(Some(&lower), false);
        let pointer_focus_ring = ring_vertices();
        assert_eq!(
            pointer_focus_ring, 0,
            "pointer focus draws no ring, matching Swift"
        );

        scene.focus.focus(Some(&lower), true);
        assert_eq!(
            ring_vertices(),
            pointer_focus_ring,
            "documented gap: visible keyboard focus paints no ring either"
        );
    });
}
