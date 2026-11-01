//! Translation of `Tests/CoveUIRuntimeTests/NodeLifecycleCleanupTests.swift`.
//!
//! Every node-keyed external registration is released by the node lifecycle,
//! with no `ViewGraph` teardown side effect: removing a node from its parent
//! releases its interaction handlers, pointer capture and focus for the whole
//! departing subtree.
//!
//! The holders are process-global thread-locals, so every test runs under
//! [`GlobalTestLock`].
//!
//! Behaviour checklist:
//! - an interaction handler is removed when the node leaves the tree
//! - a handler deep in a removed subtree is released
//! - pointer capture is released when the captured node is torn down
//! - focus is cleared when the focused node is torn down
//! - capture for a node that stays in the tree is untouched

use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_core::scene::{
    EventPhase, EventResult, FocusChain, FocusChainHolder, InputHandlerRoute, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder, PointerPhase,
};
use cove_platform::{KeyEvent, MouseButtonEvent};

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

/// Installs `registry` as the current holder for the body's scope, then restores
/// the previous one, the Swift `defer` pattern.
fn with_registry<T>(registry: Rc<InteractionRegistry>, body: impl FnOnce() -> T) -> T {
    let saved = InteractionRegistryHolder::current();
    InteractionRegistryHolder::set_current(Some(registry));
    let result = body();
    InteractionRegistryHolder::set_current(saved);
    result
}

fn with_capture<T>(capture: Rc<PointerCapture>, body: impl FnOnce() -> T) -> T {
    let saved = PointerCaptureHolder::current();
    PointerCaptureHolder::set_current(Some(capture));
    let result = body();
    PointerCaptureHolder::set_current(saved);
    result
}

fn with_focus<T>(chain: Rc<FocusChain>, body: impl FnOnce() -> T) -> T {
    let saved = FocusChainHolder::current();
    FocusChainHolder::set_current(Some(chain));
    let result = body();
    FocusChainHolder::set_current(saved);
    result
}

#[test]
fn interaction_handler_released_on_removal() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        with_registry(registry.clone(), || {
            let parent = Node::new();
            let child = Node::new();
            parent.add_child(&child);
            registry.set_pointer(
                &child,
                InputHandlerRoute::control(),
                Rc::new(|_: &MouseButtonEvent, _: PointerPhase, _: EventPhase| {
                    EventResult::Handled
                }),
            );
            assert_eq!(registry.count(), 1);

            parent.remove_child(&child);
            assert_eq!(
                registry.count(),
                0,
                "the node lifecycle released the handler"
            );
        });
    });
}

#[test]
fn interaction_released_for_subtree() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        with_registry(registry.clone(), || {
            let root = Node::new();
            let mid = Node::new();
            let leaf = Node::new();
            root.add_child(&mid);
            mid.add_child(&leaf);
            registry.set_key(
                &leaf,
                InputHandlerRoute::control(),
                Rc::new(|_: &KeyEvent, _: EventPhase| EventResult::Handled),
            );
            assert_eq!(registry.count(), 1);

            root.remove_child(&mid);
            assert_eq!(
                registry.count(),
                0,
                "the whole departing subtree released its handlers"
            );
        });
    });
}

#[test]
fn capture_released_on_removal() {
    GlobalTestLock::locked(|| {
        let capture = Rc::new(PointerCapture::new());
        with_capture(capture.clone(), || {
            let parent = Node::new();
            let child = Node::new();
            parent.add_child(&child);
            capture.acquire(&child, false);
            assert_eq!(
                capture.target().map(|node| node.id()),
                Some(child.id()),
                "the child owns capture",
            );

            // A detached node must not keep capture: every later event would
            // route to it.
            parent.remove_child(&child);
            assert!(capture.target().is_none(), "teardown released the capture");
        });
    });
}

#[test]
fn focus_cleared_on_removal() {
    GlobalTestLock::locked(|| {
        let focus = Rc::new(FocusChain::new());
        with_focus(focus.clone(), || {
            let parent = Node::new();
            let child = Node::new();
            child.set_is_focusable(true);
            parent.add_child(&child);
            focus.focus(Some(&child), false);
            assert_eq!(focus.focused().map(|node| node.id()), Some(child.id()));

            parent.remove_child(&child);
            assert!(
                focus.focused().is_none(),
                "teardown cleared the focused node"
            );
        });
    });
}

#[test]
fn capture_for_retained_node_untouched() {
    GlobalTestLock::locked(|| {
        let capture = Rc::new(PointerCapture::new());
        with_capture(capture.clone(), || {
            let parent = Node::new();
            let a = Node::new();
            let b = Node::new();
            parent.add_child(&a);
            parent.add_child(&b);
            capture.acquire(&a, false);

            // Removing a *different* node must not disturb the capture.
            parent.remove_child(&b);
            assert_eq!(
                capture.target().map(|node| node.id()),
                Some(a.id()),
                "an unrelated removal leaves the capture alone",
            );
        });
    });
}
