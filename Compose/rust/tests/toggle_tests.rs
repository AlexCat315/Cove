//! Translation of `Tests/CoveUIComposeTests/ToggleTests.swift`.
//!
//! Behaviour checklist:
//! - a `Toggle` materialises a hit-testable, focusable host that registers
//!   pointer and hover handlers
//! - pointer down does not toggle; pointer up does, and a second press/release
//!   toggles back
//! - hover and press update render state without scheduling a recomposition
//! - a right click is ignored and leaves the binding alone
//! - a disabled toggle registers no pointer handler, refuses the pointer cursor
//!   and keeps its value
//! - Space and Return toggle the focused control
//! - `Checkbox` shares the same boolean activation semantics

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cove_composition::{AnyView, View};
use cove_core::Binding;
use cove_core::scene::{
    EventPhase, EventResult, InteractionRegistry, InteractionRegistryHolder, Node, PointerPhase,
};
use cove_platform::{KeyEvent, MouseButton, MouseButtonEvent};
use cove_ui_compose::primitives::controls::toggle::{Checkbox, Toggle};

use support::GlobalTestLock;

const KEY_SPACE: u32 = 44;
const KEY_RETURN: u32 = 40;

fn binding(initial: bool) -> (Binding<bool>, Arc<AtomicBool>) {
    let storage = Arc::new(AtomicBool::new(initial));
    let get = storage.clone();
    let set = storage.clone();
    (
        Binding::new(
            move || get.load(Ordering::SeqCst),
            move |value| set.store(value, Ordering::SeqCst),
        ),
        storage,
    )
}

fn value_of(storage: &Arc<AtomicBool>) -> bool {
    storage.load(Ordering::SeqCst)
}

/// Materialises `view` and returns the boolean control host it mounted.
fn install(view: impl View + 'static) -> (std::rc::Rc<cove_composition::ViewGraph>, Node) {
    let graph = support::graph();
    graph.install_view(AnyView::new(view), Default::default());
    graph.compute_layout(100.0, 24.0);
    let root = graph.tree.borrow().root().expect("root");
    let host = find_bool_host(&root).expect("the boolean control host is mounted");
    (graph, host)
}

fn find_bool_host(node: &Node) -> Option<Node> {
    if node
        .accessibility()
        .is_some_and(|semantics| {
            matches!(
                semantics.role,
                cove_core::scene::AccessibilityRole::Toggle
                    | cove_core::scene::AccessibilityRole::Checkbox
            )
        })
    {
        return Some(node.clone());
    }
    node.children().iter().find_map(find_bool_host)
}

fn event(button: MouseButton) -> MouseButtonEvent {
    MouseButtonEvent {
        button,
        x: 12.0,
        y: 12.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn key(scancode: u32) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers: Default::default(),
        is_repeat: false,
    }
}

#[test]
fn materialises_a_hit_testable_focusable_host() {
    GlobalTestLock::locked(|| {
        let registry = std::rc::Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let (binding, _) = binding(false);
        let (_graph, host) = install(Toggle::new(binding));

        assert!(host.is_hit_testable(), "the host accepts pointer input");
        assert!(host.is_focusable(), "the host joins focus traversal");
        let handlers = registry.handlers(&host);
        assert!(handlers.pointer.is_some(), "the host registers a pointer handler");
        assert!(handlers.hover.is_some(), "the host registers a hover handler");
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn pointer_up_toggles_the_binding() {
    GlobalTestLock::locked(|| {
        let registry = std::rc::Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let (binding, storage) = binding(false);
        let (graph, host) = install(Toggle::new(binding));

        let pointer = registry
            .handlers(&host)
            .pointer
            .expect("pointer handler");
        let press = event(MouseButton::Left);

        pointer(&press, PointerPhase::Down, EventPhase::Target);
        graph.recomposer.commit_all();
        assert!(!value_of(&storage), "a press alone does not toggle");

        pointer(&press, PointerPhase::Up, EventPhase::Target);
        graph.recomposer.commit_all();
        assert!(value_of(&storage), "the release toggles the binding");

        pointer(&press, PointerPhase::Down, EventPhase::Target);
        graph.recomposer.commit_all();
        pointer(&press, PointerPhase::Up, EventPhase::Target);
        graph.recomposer.commit_all();
        assert!(!value_of(&storage), "a second press/release toggles back");
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn interaction_updates_render_state_without_recomposing() {
    GlobalTestLock::locked(|| {
        let registry = std::rc::Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let (binding, storage) = binding(false);
        let (graph, host) = install(Toggle::new(binding));

        let handlers = registry.handlers(&host);
        let hover = handlers.hover.expect("hover handler");
        let pointer = handlers.pointer.expect("pointer handler");
        let press = event(MouseButton::Left);

        assert!(!graph.recomposer.has_pending(), "a settled graph is idle");

        hover(cove_core::scene::HoverPhase::Enter);
        assert!(
            !graph.recomposer.has_pending(),
            "hover does not schedule a recomposition"
        );
        assert!(
            host.attachment::<bool>("__bool_control_hovered") == Some(true),
            "hover writes the render-state attachment"
        );

        pointer(&press, PointerPhase::Down, EventPhase::Target);
        assert!(
            !graph.recomposer.has_pending(),
            "press does not schedule a recomposition"
        );
        assert!(
            host.attachment::<bool>("__bool_control_pressed") == Some(true),
            "press writes the render-state attachment"
        );
        assert!(!value_of(&storage), "press alone leaves the value");

        pointer(&press, PointerPhase::Up, EventPhase::Target);
        assert!(
            !graph.recomposer.has_pending(),
            "release does not schedule a recomposition either"
        );
        assert!(
            host.attachment::<bool>("__bool_control_pressed") == Some(false),
            "release clears the press attachment"
        );
        assert!(value_of(&storage), "release commits the value");
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn right_click_is_ignored() {
    GlobalTestLock::locked(|| {
        let registry = std::rc::Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let (binding, storage) = binding(false);
        let (graph, host) = install(Toggle::new(binding));

        let pointer = registry
            .handlers(&host)
            .pointer
            .expect("pointer handler");
        let press = event(MouseButton::Right);

        assert_eq!(
            pointer(&press, PointerPhase::Down, EventPhase::Target),
            EventResult::Ignored,
            "a right press is ignored"
        );
        graph.recomposer.commit_all();
        assert_eq!(
            pointer(&press, PointerPhase::Up, EventPhase::Target),
            EventResult::Ignored,
            "a right release is ignored"
        );
        assert!(!value_of(&storage), "a right click leaves the value alone");
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn disabled_toggle_ignores_input() {
    GlobalTestLock::locked(|| {
        let registry = std::rc::Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let (binding, storage) = binding(true);
        let (_graph, host) = install(Toggle::new(binding).enabled(false));

        assert!(
            registry.handlers(&host).pointer.is_none(),
            "a disabled toggle registers no pointer handler"
        );
        assert_eq!(
            host.cursor(),
            Some(cove_platform::SystemCursor::NotAllowed),
            "a disabled toggle refuses the pointer cursor"
        );
        assert!(value_of(&storage), "a disabled toggle keeps its value");
        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn space_and_return_toggle_the_focused_control() {
    GlobalTestLock::locked(|| {
        for scancode in [KEY_SPACE, KEY_RETURN] {
            let registry = std::rc::Rc::new(InteractionRegistry::new());
            InteractionRegistryHolder::set_current(Some(registry.clone()));
            let (binding, storage) = binding(false);
            let (_graph, host) = install(Toggle::new(binding));

            let handler = registry
                .handlers(&host)
                .key
                .expect("key handler");
            assert_eq!(
                handler(&key(scancode), EventPhase::Target),
                EventResult::Handled,
                "scancode {scancode} toggles the control"
            );
            assert!(
                value_of(&storage),
                "scancode {scancode} committed the new value"
            );
            InteractionRegistryHolder::set_current(None);
        }
    });
}

#[test]
fn checkbox_shares_bool_activation_semantics() {
    GlobalTestLock::locked(|| {
        let registry = std::rc::Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        let (binding, storage) = binding(false);
        let (graph, host) = install(Checkbox::from_boolean(binding));

        let pointer = registry
            .handlers(&host)
            .pointer
            .expect("pointer handler");
        let press = event(MouseButton::Left);
        pointer(&press, PointerPhase::Down, EventPhase::Target);
        graph.recomposer.commit_all();
        pointer(&press, PointerPhase::Up, EventPhase::Target);
        graph.recomposer.commit_all();
        assert!(value_of(&storage), "the check box toggles on pointer");

        let handler = registry
            .handlers(&host)
            .key
            .expect("key handler");
        assert_eq!(
            handler(&key(KEY_SPACE), EventPhase::Target),
            EventResult::Handled,
            "Space toggles the check box"
        );
        assert!(!value_of(&storage), "the check box toggled back");
        InteractionRegistryHolder::set_current(None);
    });
}
