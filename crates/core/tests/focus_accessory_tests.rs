//! Translation of `Tests/Headless/Tests/CoveUISceneTests/Input/FocusAccessoryTests.swift`.
//!
//! A portal that is a *focus accessory* belongs to its owner's modal scope: its
//! contents take input because the owner does, but hovering it never steals
//! focus. Ownership is logical, so it nests, and a cycle has to be refused
//! rather than resolved into one.
//!
//! Behaviour checklist:
//! - a hovered portal stays in its owner's modal scope without stealing focus
//! - a portal owned by something outside the modal scope does not admit input
//! - detaching an accessory restores previous focus and drops the input grant
//! - logical ownership supports nested portals
//! - ownership cycles and disabled owners are refused
//! - leaf-first teardown restores previous focus
//! - an unrelated new focus survives teardown
//! - teardown with no valid restore target clears focus

use cove_core::scene::{FocusChain, Node};

fn focusable(node: &Node) {
    node.set_is_focusable(true);
}

fn same(a: Option<&Node>, b: &Node) -> bool {
    a.is_some_and(|candidate| candidate.id() == b.id())
}

#[test]
fn modal_ownership() {
    let chain = FocusChain::new();
    let window = Node::new();
    let modal = Node::new();
    let owner = Node::new();
    let previous = Node::new();
    let portal = Node::new();
    let action = Node::new();
    let background_owner = Node::new();
    let background_portal = Node::new();
    let forbidden = Node::new();

    window.add_child(&modal);
    window.add_child(&portal);
    window.add_child(&background_owner);
    window.add_child(&background_portal);
    modal.add_child(&previous);
    modal.add_child(&owner);
    portal.add_child(&action);
    background_portal.add_child(&forbidden);
    focusable(&previous);
    focusable(&action);
    focusable(&forbidden);

    chain.begin_modal(&modal);
    chain.focus(Some(&previous), false);
    chain.attach_accessory(&portal, &owner);
    chain.attach_accessory(&background_portal, &background_owner);

    assert!(
        same(chain.focused().as_ref(), &previous),
        "attaching an accessory does not steal focus"
    );
    assert!(!chain.is_focus_visible(), "focus visibility is unchanged");
    assert!(
        chain.permits_input(&action),
        "the accessory admits pointer input"
    );
    assert!(
        chain.permits_keyboard_input(&action),
        "the accessory admits keyboard input"
    );
    assert!(
        !chain.permits_input(&forbidden),
        "a portal owned outside the modal scope takes no input"
    );
    assert!(
        !chain.permits_keyboard_input(&forbidden),
        "a portal owned outside the modal scope takes no keys"
    );
    assert_eq!(chain.accessory_roots().len(), 1, "one accessory is exposed");
    assert!(
        chain
            .accessory_roots()
            .iter()
            .any(|root| root.id() == portal.id()),
        "the exposed accessory root is the portal",
    );

    chain.focus(Some(&action), false);
    chain.detach_accessory(&portal);
    assert!(
        same(chain.focused().as_ref(), &previous),
        "detaching restores the focus the accessory displaced"
    );
    assert!(!chain.is_focus_visible(), "restored focus is not visible");
    assert!(
        !chain.permits_input(&action),
        "a detached accessory no longer takes input"
    );
    assert!(chain.accessory_roots().is_empty(), "no accessories remain");
}

#[test]
fn nesting_and_cycles() {
    let chain = FocusChain::new();
    let window = Node::new();
    let modal = Node::new();
    let owner = Node::new();
    let portal = Node::new();
    let nested = Node::new();
    let action = Node::new();

    window.add_child(&modal);
    modal.add_child(&owner);
    window.add_child(&portal);
    window.add_child(&nested);
    nested.add_child(&action);

    chain.begin_modal(&modal);
    chain.attach_accessory(&portal, &owner);
    chain.attach_accessory(&nested, &portal);
    assert!(
        chain.permits_input(&action),
        "ownership nests through the accessory chain"
    );

    owner.set_allows_hit_testing(false);
    assert!(
        !chain.permits_input(&action),
        "a disabled owner revokes the whole chain"
    );
    assert!(
        chain.accessory_roots().is_empty(),
        "a disabled owner exposes no accessory roots"
    );

    owner.set_allows_hit_testing(true);
    chain.detach_accessory(&portal);
    // The cycle case: a portal owned by a node inside itself cannot admit
    // input, because resolving ownership would never reach the modal scope.
    chain.attach_accessory(&portal, &nested);
    assert!(
        !chain.permits_input(&action),
        "an ownership cycle is refused"
    );
    assert!(
        chain.accessory_roots().is_empty(),
        "a refused cycle exposes no accessory roots"
    );
}

#[test]
fn teardown() {
    let chain = FocusChain::new();
    let window = Node::new();
    let previous = Node::new();
    let other = Node::new();
    let portal = Node::new();
    let action = Node::new();

    window.add_child(&previous);
    window.add_child(&other);
    window.add_child(&portal);
    portal.add_child(&action);
    focusable(&previous);
    focusable(&other);
    focusable(&action);

    chain.focus(Some(&previous), false);
    chain.attach_accessory(&portal, &previous);
    chain.focus(Some(&action), false);

    // A node teardown clears its focused leaf before the portal resource
    // unmounts, so the accessory has to restore from its own snapshot.
    chain.clear();
    chain.detach_accessory(&portal);
    assert!(
        same(chain.focused().as_ref(), &previous),
        "leaf-first teardown restores the focus the portal displaced"
    );

    chain.attach_accessory(&portal, &previous);
    chain.focus(Some(&other), false);
    chain.detach_accessory(&portal);
    assert!(
        same(chain.focused().as_ref(), &other),
        "an unrelated new focus is preserved"
    );

    chain.attach_accessory(&portal, &previous);
    chain.focus(Some(&action), false);
    window.remove_child(&other);
    chain.detach_accessory(&portal);
    assert!(
        chain.focused().is_none(),
        "teardown with no valid restore target clears focus"
    );
}
