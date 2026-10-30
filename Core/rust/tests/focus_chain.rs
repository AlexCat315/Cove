//! Translation of `Tests/CoveUIRuntimeTests/FocusChainTests.swift`.
//!
//! Behaviour checklist:
//! - automatic modal focus preserves pointer or keyboard visibility through
//!   delayed children and nested scopes
//! - parent menu columns stay pointer-accessible above a modal, a newer modal
//!   blocks the older hierarchy, and only the deepest transient column takes
//!   keyboard input
//! - `focus_next` from nothing takes the first focusable in tree order
//! - `focus_next` walks tree order and wraps
//! - `focus_previous` wraps backwards
//! - an empty focusable set yields `None` and leaves focus unset
//! - pointer capture acquires and releases
//! - the interaction registry starts empty and accepts/removes a pointer handler

use std::rc::Rc;

use cove_core::scene::{
    EventPhase, EventResult, FocusChain, InputHandlerRoute, InteractionRegistry, Node,
    PointerCapture, PointerPhase,
};
use cove_platform::MouseButtonEvent;

fn focusable() -> Node {
    let node = Node::new();
    node.set_is_focusable(true);
    node
}

fn same(actual: Option<Node>, expected: &Node) -> bool {
    actual.is_some_and(|node| node.id() == expected.id())
}

#[test]
fn scope_modality() {
    for visible in [false, true] {
        let tree = Node::new();
        let opener = focusable();
        let modal = focusable();
        let nested = focusable();
        let button = focusable();
        for node in [&opener, &modal, &nested] {
            tree.add_child(node);
        }
        let focus = FocusChain::new();
        focus.focus(Some(&opener), visible);
        focus.push_scope(&modal, false);
        assert!(same(focus.focused(), &modal), "pushing a scope focuses its root");
        assert_eq!(focus.is_focus_visible(), visible);

        modal.add_child(&button);
        focus.settle_scope(&modal);
        assert!(
            same(focus.focused(), &button),
            "a child arriving after the scope settles takes focus"
        );
        assert_eq!(focus.is_focus_visible(), visible);

        focus.clear();
        focus.settle_scope(&modal);
        assert!(
            same(focus.focused(), &button),
            "clearing refocuses the scope's first focusable"
        );
        assert_eq!(focus.is_focus_visible(), visible);

        focus.push_scope(&nested, false);
        focus.settle_scope(&nested);
        assert!(same(focus.focused(), &nested), "the nested scope takes over");
        assert_eq!(focus.is_focus_visible(), visible);

        focus.pop_scope(&nested);
        assert!(
            same(focus.focused(), &button),
            "popping the nested scope restores the outer focus"
        );
        assert_eq!(focus.is_focus_visible(), visible);

        focus.focus_next(&tree);
        assert!(focus.is_focus_visible(), "keyboard traversal is always visible");

        focus.pop_scope(&modal);
        assert!(
            same(focus.focused(), &opener),
            "popping the modal restores the opener"
        );
        assert_eq!(focus.is_focus_visible(), visible);

        focus.push_scope(&modal, false);
        tree.remove_child(&opener);
        focus.pop_scope(&modal);
        assert!(
            focus.focused().is_none(),
            "the restored target is gone, so nothing holds focus"
        );
        assert_eq!(focus.is_focus_visible(), visible);
    }
}

#[test]
fn transient_columns() {
    let tree = Node::new();
    let dialog = Node::new();
    let parent_menu = Node::new();
    let child_menu = Node::new();
    let newer_dialog = Node::new();
    for node in [&dialog, &parent_menu, &child_menu, &newer_dialog] {
        tree.add_child(node);
    }
    let outside = Node::new();
    tree.add_child(&outside);

    let focus = FocusChain::new();
    focus.push_scope(&dialog, false);
    focus.push_scope(&parent_menu, true);
    focus.push_scope(&child_menu, true);
    assert!(focus.permits_input(&dialog));
    assert!(focus.permits_input(&parent_menu));
    assert!(focus.permits_input(&child_menu));
    assert!(!focus.permits_input(&outside), "the background is blocked");
    assert!(
        !focus.permits_keyboard_input(&parent_menu),
        "only the deepest menu column owns the keyboard"
    );
    assert!(focus.permits_keyboard_input(&child_menu));

    focus.push_scope(&newer_dialog, false);
    assert!(
        !focus.permits_input(&dialog) && !focus.permits_input(&parent_menu) && !focus.permits_input(&child_menu),
        "a newer modal blocks the whole older hierarchy"
    );
    assert!(focus.permits_input(&newer_dialog));
}

/// root → A, container → (B, C), D; `A`, `B` and `D` are focusable.
fn make_tree() -> (Node, Vec<Node>) {
    let root = Node::new();
    let a = focusable();
    let container = Node::new();
    let b = focusable();
    let c = Node::new();
    let d = focusable();

    root.add_child(&a);
    root.add_child(&container);
    container.add_child(&b);
    container.add_child(&c);
    root.add_child(&d);
    (root, vec![a, b, d])
}

#[test]
fn next_from_nil() {
    let (root, focusables) = make_tree();
    let focus = FocusChain::new();
    let next = focus.focus_next(&root);
    assert!(same(next.clone(), &focusables[0]), "first focusable in tree order");
    assert!(same(focus.focused(), &focusables[0]));
}

#[test]
fn next_wraps() {
    let (root, focusables) = make_tree();
    let focus = FocusChain::new();
    focus.focus(Some(&focusables[0]), false);
    assert!(same(focus.focus_next(&root), &focusables[1]));
    assert!(same(focus.focus_next(&root), &focusables[2]));
    assert!(same(focus.focus_next(&root), &focusables[0]), "wraps to the start");
}

#[test]
fn previous_wraps() {
    let (root, focusables) = make_tree();
    let focus = FocusChain::new();
    focus.focus(Some(&focusables[0]), false);
    assert!(same(focus.focus_previous(&root), &focusables[2]), "wraps to the end");
    assert!(same(focus.focus_previous(&root), &focusables[1]));
    assert!(same(focus.focus_previous(&root), &focusables[0]));
}

#[test]
fn empty() {
    let root = Node::new();
    root.add_child(&Node::new());
    let focus = FocusChain::new();
    assert!(focus.focus_next(&root).is_none());
    assert!(focus.focused().is_none());
}

#[test]
fn pointer_capture_acquire_then_release() {
    let capture = PointerCapture::new();
    let node = Node::new();
    assert!(!capture.is_active());
    capture.acquire(&node, false);
    assert!(capture.is_active());
    assert!(same(capture.target(), &node));
    capture.release();
    assert!(!capture.is_active());
}

#[test]
fn interaction_registry_starts_empty() {
    let registry = InteractionRegistry::new();
    let node = Node::new();
    assert!(registry.handlers(&node).is_empty());
    assert_eq!(registry.count(), 0);
}

#[test]
fn interaction_registry_set_and_remove_pointer_handler() {
    let registry = InteractionRegistry::new();
    let node = Node::new();
    let handler = Rc::new(
        |_: &MouseButtonEvent, _: PointerPhase, _: EventPhase| EventResult::Handled,
    );
    registry.set_pointer(&node, InputHandlerRoute::control(), handler);
    assert!(
        registry.handlers(&node).pointer.is_some(),
        "the pointer handler is registered"
    );
    registry.remove(&node);
    assert!(registry.handlers(&node).is_empty());
}
