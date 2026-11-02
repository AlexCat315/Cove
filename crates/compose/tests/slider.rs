//! Translation of `Tests/CoveUIComposeTests/SliderTests.swift`.
//!
//! Behaviour checklist:
//! - a width-only slider in a centered row keeps its 24pt intrinsic height and paints
//! - the host is hit-testable, focusable and registers pointer/motion/hover handlers
//! - a pointer down writes the position under the pointer, clamped to the range
//! - `step` quantises writes onto the step grid
//! - motion drags the value while pressed, and capture pairs press with release
//! - interaction updates render state without scheduling a recomposition
//! - a disabled slider registers no pointer handler
//! - PageUp/PageDown move by ten steps and clamp
//! - discrete ticks add one mark per step

use crate::support;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cove_composition::AnyView;
use cove_core::Binding;
use cove_core::scene::{
    AccessibilityRole, EventPhase, EventResult, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder, PointerPhase,
};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::slider::{Slider, SliderAxis, SliderGeometry};
use cove_ui_compose::{Row, VerticalAlignment};

use support::GlobalTestLock;

fn make_binding(initial: f64) -> (Binding<f64>, Arc<AtomicU64>) {
    let storage = Arc::new(AtomicU64::new(initial.to_bits()));
    let get = storage.clone();
    let set = storage.clone();
    (
        Binding::new(
            move || f64::from_bits(get.load(Ordering::SeqCst)),
            move |value: f64| set.store(value.to_bits(), Ordering::SeqCst),
        ),
        storage,
    )
}

fn value_of(storage: &Arc<AtomicU64>) -> f64 {
    f64::from_bits(storage.load(Ordering::SeqCst))
}

fn find_slider(node: &Node) -> Option<Node> {
    if node
        .accessibility()
        .is_some_and(|semantics| semantics.role == AccessibilityRole::Slider)
    {
        return Some(node.clone());
    }
    node.children().iter().find_map(find_slider)
}

fn slider_node(graph: &Rc<cove_composition::ViewGraph>) -> Node {
    let root = graph.tree.borrow().root().expect("root node");
    find_slider(&root).expect("a slider host node")
}

fn pointer_event(x: f32, y: f32) -> cove_platform::MouseButtonEvent {
    cove_platform::MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

#[test]
fn width_only_slider_keeps_intrinsic_height_and_paints() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        let slider = AnyView::new(Slider::new({
            let (binding, _) = make_binding(0.3);
            binding
        }))
        .frame_width(180.0);
        let row = AnyView::new(Row::new(
            VerticalAlignment::Center,
            0.0,
            AnyView::new(slider),
        ));
        graph.install_view(row, Default::default());
        graph.compute_layout(500.0, 100.0);

        let slider = slider_node(&graph);
        assert_eq!(slider.frame().width, 180.0, "the authored width wins");
        assert_eq!(slider.frame().height, 24.0, "a width-only frame keeps 24pt");

        let mut draw = cove_core::DrawList::default();
        let hook = slider.draw().expect("the slider paints");
        hook(&mut draw, cove_core::Point::default());
        assert!(!draw.vertices().is_empty(), "the slider paints geometry");
    });
}

#[test]
fn slider_materialises_interactive_host() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, _) = make_binding(0.5);
        let graph = support::graph();
        graph.install_view(AnyView::new(Slider::new(binding)), Default::default());

        let host = slider_node(&graph);
        assert!(host.is_hit_testable());
        assert!(host.is_focusable());
        let handlers = registry.handlers(&host);
        assert!(handlers.pointer.is_some(), "the host takes pointer input");
        assert!(handlers.motion.is_some(), "the host takes drag motion");
        assert!(handlers.hover.is_some(), "the host reacts to hover");

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn pointer_down_writes_the_value_under_the_pointer() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, read) = make_binding(0.0);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 1.0))),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let width = host.frame().width;
        assert!(width > 0.0, "the host has a measurable width");

        let pointer = registry.handlers(&host).pointer.expect("pointer handler");
        pointer(
            &pointer_event(width, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert_eq!(
            value_of(&read),
            1.0,
            "the right edge writes the upper bound"
        );

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn pointer_outside_the_host_clamps_to_the_range() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, read) = make_binding(0.5);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 10.0))),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let width = host.frame().width;
        let pointer = registry.handlers(&host).pointer.expect("pointer handler");

        pointer(
            &pointer_event(-50.0, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert_eq!(
            value_of(&read),
            0.0,
            "a left overflow clamps to the lower bound"
        );

        pointer(
            &pointer_event(width + 50.0, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert_eq!(
            value_of(&read),
            10.0,
            "a right overflow clamps to the upper bound"
        );

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn step_quantises_writes_onto_the_grid() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, read) = make_binding(0.0);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 1.0)).step(0.25)),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let width = host.frame().width;
        let pointer = registry.handlers(&host).pointer.expect("pointer handler");

        pointer(
            &pointer_event(width * 0.30, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert_eq!(value_of(&read), 0.25, "30% snaps down to 0.25");

        pointer(
            &pointer_event(width * 0.60, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert_eq!(value_of(&read), 0.50, "60% snaps to 0.50");

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn drag_motion_updates_the_value_while_pressed() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, read) = make_binding(0.0);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 100.0))),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let width = host.frame().width;
        let pointer = registry.handlers(&host).pointer.expect("pointer handler");
        pointer(
            &pointer_event(0.0, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        graph.recomposer.commit_all();

        // The recompose re-registers the motion handler with a fresh
        // `isPressed = true` snapshot, so re-read it.
        let motion = registry.handlers(&host).motion.expect("motion handler");
        motion(
            &cove_platform::MouseMotionEvent {
                x: width * 0.5,
                y: 12.0,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            },
            EventPhase::Target,
        );
        assert_eq!(value_of(&read), 50.0, "half the track is half the range");

        motion(
            &cove_platform::MouseMotionEvent {
                x: width,
                y: 12.0,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            },
            EventPhase::Target,
        );
        assert_eq!(
            value_of(&read),
            100.0,
            "the far edge reaches the upper bound"
        );

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn interaction_updates_render_state_without_recomposing() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, read) = make_binding(0.0);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 100.0))),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let width = host.frame().width;
        let handlers = registry.handlers(&host);
        let hover = handlers.hover.expect("hover handler");
        let pointer = handlers.pointer.expect("pointer handler");
        let motion = handlers.motion.expect("motion handler");

        graph.tree.borrow_mut().flush();
        assert!(!graph.recomposer.has_pending(), "settled after layout");

        hover(cove_core::scene::HoverPhase::Enter);
        assert!(
            !graph.recomposer.has_pending(),
            "hover repaints without recomposing"
        );
        assert!(
            graph.tree.borrow().has_render_updates(),
            "hover dirties render"
        );
        assert_eq!(
            host.attachment::<bool>("__slider_hovered"),
            Some(true),
            "the host records the hover state"
        );

        graph.tree.borrow_mut().flush();
        pointer(
            &pointer_event(width * 0.25, 12.0),
            PointerPhase::Down,
            EventPhase::Target,
        );
        assert!(!graph.recomposer.has_pending(), "press repaints only");
        assert!(graph.tree.borrow().has_render_updates());
        assert_eq!(
            host.attachment::<bool>("__slider_pressed"),
            Some(true),
            "the host records the press state"
        );
        assert_eq!(value_of(&read), 25.0);

        graph.tree.borrow_mut().flush();
        motion(
            &cove_platform::MouseMotionEvent {
                x: width * 0.75,
                y: 12.0,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            },
            EventPhase::Target,
        );
        assert!(!graph.recomposer.has_pending(), "drag repaints only");
        assert!(graph.tree.borrow().has_render_updates());
        assert_eq!(value_of(&read), 75.0);

        graph.tree.borrow_mut().flush();
        pointer(
            &pointer_event(width * 0.75, 12.0),
            PointerPhase::Up,
            EventPhase::Target,
        );
        assert!(!graph.recomposer.has_pending(), "release repaints only");
        assert!(graph.tree.borrow().has_render_updates());
        assert_eq!(
            host.attachment::<bool>("__slider_pressed"),
            Some(false),
            "release clears the press state"
        );

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn disabled_slider_registers_no_pointer_handler() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, read) = make_binding(0.25);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 1.0)).enabled(false)),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        assert!(
            registry.handlers(&host).pointer.is_none(),
            "a disabled slider takes no pointer input"
        );
        assert_eq!(value_of(&read), 0.25, "the value is untouched");

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn pointer_capture_is_acquired_on_press_and_released_on_release() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let capture = Rc::new(PointerCapture::new());
        PointerCaptureHolder::set_current(Some(capture.clone()));

        let (binding, _) = make_binding(0.0);
        let graph = support::graph();
        graph.install_view(AnyView::new(Slider::new(binding)), Default::default());
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let pointer = registry.handlers(&host).pointer.expect("pointer handler");
        let event = pointer_event(0.0, 12.0);

        pointer(&event, PointerPhase::Down, EventPhase::Target);
        assert_eq!(
            capture.target(),
            Some(host.clone()),
            "press captures the pointer"
        );

        pointer(&event, PointerPhase::Up, EventPhase::Target);
        assert_eq!(capture.target(), None, "release gives the pointer back");

        PointerCaptureHolder::set_current(None);
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn page_keys_move_by_ten_steps_and_clamp() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let focus = Rc::new(FocusChain::new());
        FocusChainHolder::set_current(Some(focus.clone()));

        let (binding, read) = make_binding(50.0);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 100.0)).step(1.0)),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        focus.focus(Some(&host), true);
        let dispatcher = cove_core::scene::EventDispatcher::new(
            graph.tree.clone(),
            registry.clone(),
            Rc::new(PointerCapture::new()),
            focus.clone(),
            cove_platform::WindowId::MAIN,
        );
        let key = |scancode: u32| {
            cove_platform::InputEvent::KeyDown(cove_platform::KeyEvent {
                scancode,
                keycode: 0,
                modifiers: Default::default(),
                is_repeat: false,
            })
        };

        dispatcher.dispatch(&key(cove_composition::Scancode::PAGE_UP));
        assert_eq!(value_of(&read), 60.0, "PageUp adds ten steps");
        dispatcher.dispatch(&key(cove_composition::Scancode::PAGE_DOWN));
        assert_eq!(value_of(&read), 50.0, "PageDown removes ten steps");
        for _ in 0..5 {
            dispatcher.dispatch(&key(cove_composition::Scancode::PAGE_UP));
        }
        assert_eq!(
            value_of(&read),
            100.0,
            "repeated PageUp clamps at the maximum"
        );

        FocusChainHolder::set_current(None);
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn discrete_ticks_add_one_mark_per_step() {
    GlobalTestLock::locked(|| {
        // The tick policy is the implemented half: one hairline per step, minus
        // those hidden under the knob, and none when the step is absent.
        let node = Node::new();
        node.set_frame(cove_core::Rect::new(0.0, 0.0, 200.0, 24.0));
        let geometry = SliderGeometry::new(&node, SliderAxis::Horizontal);
        assert_eq!(geometry.length, 200.0);

        let marks = |step: f64| {
            cove_ui_compose::primitives::controls::fields::slider::ticks(
                &geometry,
                cove_core::Point::default(),
                4.0,
                step,
                1.0,
                0.5,
                14.0,
            )
            .len()
        };
        // Five tick positions 0, .25, .5, .75, 1 — the one at the knob is hidden.
        assert_eq!(marks(0.25), 4, "one mark per step, minus the knob position");
        assert_eq!(marks(0.0), 0, "no step means no ticks");
        assert_eq!(
            marks(0.04),
            0,
            "ticks closer than 10pt are omitted instead of crowding"
        );
        assert_eq!(marks(0.02), 0, "more than 40 ticks are dropped entirely");
    });
}

#[test]
fn shows_ticks_paints_extra_geometry() {
    GlobalTestLock::locked(|| {
        // The draw path, not just the policy: the host's render must add one
        // hairline per step. Without the tick pass the two draws would match.
        let paint = |shows_ticks: bool| {
            let (binding, _) = make_binding(0.5);
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    Slider::new(binding)
                        .range((0.0, 1.0))
                        .step(0.1)
                        .shows_ticks(shows_ticks),
                ),
                Default::default(),
            );
            graph.compute_layout(200.0, 24.0);
            let slider = slider_node(&graph);
            let mut draw = cove_core::DrawList::default();
            let hook = slider.draw().expect("the slider paints");
            hook(&mut draw, cove_core::Point::default());
            draw.vertices().len()
        };
        let plain = paint(false);
        let ticked = paint(true);
        assert!(plain > 0, "the slider paints its track and knob");
        assert!(
            ticked > plain,
            "ticks add geometry: {ticked} vertices with ticks vs {plain} without"
        );
    });
}

#[test]
fn key_handler_reports_handled_only_for_slider_keys() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let (binding, _) = make_binding(0.5);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Slider::new(binding).range((0.0, 100.0)).step(1.0)),
            Default::default(),
        );
        graph.compute_layout(200.0, 24.0);

        let host = slider_node(&graph);
        let key = registry.handlers(&host).key.expect("key handler");
        let press = |scancode: u32| {
            key(
                &cove_platform::KeyEvent {
                    scancode,
                    keycode: 0,
                    modifiers: Default::default(),
                    is_repeat: false,
                },
                EventPhase::Target,
            )
        };
        assert_eq!(
            press(cove_composition::Scancode::ARROW_UP),
            EventResult::Handled
        );
        assert_eq!(
            press(cove_composition::Scancode::HOME),
            EventResult::Handled
        );
        assert_eq!(
            press(cove_composition::Scancode::A),
            EventResult::Ignored,
            "an unrelated key is ignored"
        );

        InteractionRegistryHolder::set_current(None);
    });
}
