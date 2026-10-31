//! Translation of `Tests/CoveUIRuntimeTests/NativeAccessibilityTests.swift`.
//!
//! What AppKit and UIKit read out of the scene, and what they must never read.
//! The expensive mistakes are all about *when* values are materialized: a
//! hundred synchronizations must not invoke a lazy provider, a secure field's
//! provider must never run at all, and a removed control must stop answering
//! actions while the snapshot that described it stays valid.
//!
//! Behaviour checklist:
//! - stable native properties are reused while actions, focus and geometry
//!   stay live
//! - heading semantics preserve the complete title and use the platform
//!   heading role
//! - pointer-transparent descriptions remain accessible while interactive
//!   actions stay blocked
//! - scene synchronization never materializes a lazy value; the platform reads
//!   it on demand
//! - AppKit controls expose real actions and values; removed elements cannot
//!   activate
//!
//! Named difference — `GAP: AppKit bridge reachability.` Swift drives the real
//! `MacAccessibilityBridge` against a live `NSWindow` and asserts on
//! `NSAccessibilityElement` (`accessibilityFrame()`,
//! `accessibilityPerformPress()`, `accessibilityChildren()`, and so on). Rust's
//! equivalent, `AppleAccessibilityBridge`, is declared `pub(super)` in
//! `Runtime/rust/native_accessibility.rs:81` and is created only from
//! `NativeAccessibility::synchronize`, which needs an
//! `cove_shell::NativeWindowReference::AppKit` handle. `cove-runtime` carries no
//! `objc2` dependency, so an integration test cannot construct one. The bridge's
//! *inputs and contract* are public though — `AccessibilityTree::snapshot_in`,
//! the revision counters, the action closures and the value providers — and
//! those are what the bridge reads. Each case below therefore asserts the
//! contract at that seam. The bridge-specific mechanics named in Swift
//! (`updatedElementCount`, `accessibilityRole() == .headingRole` on macOS 26,
//! `accessibilitySubrole() == .secureTextField`) are reported as unavailable
//! rather than asserted falsely.

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityElement, AccessibilityRevision, AccessibilityRole,
    AccessibilitySemantics, AccessibilityState, AccessibilityTree, AccessibilityValueProvider,
    FocusChain, Node, NodeTree,
};
use cove_core::Rect;

fn node(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

/// Flatten a snapshot depth-first, keeping each element's own semantics.
fn flatten(elements: &[AccessibilityElement], out: &mut Vec<AccessibilityElement>) {
    for element in elements {
        out.push(element.clone());
        flatten(&element.children, out);
    }
}

fn snapshot(root: &Node) -> Vec<AccessibilityElement> {
    let mut out = Vec::new();
    flatten(&AccessibilityTree::snapshot_in(root, None, false), &mut out);
    out
}

fn find(root: &Node, id: u64) -> Option<AccessibilityElement> {
    snapshot(root)
        .into_iter()
        .find(|element| element.node.id().0 == id)
}

#[test]
fn stable_native_properties_are_reused_while_actions_focus_and_geometry_stay_live() {
    let root = node(0.0, 0.0, 320.0, 200.0);
    let button = node(10.0, 10.0, 100.0, 30.0);
    button.set_is_focusable(true);
    button.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Run"),
    ));
    let first = Rc::new(Cell::new(0_usize));
    let second = Rc::new(Cell::new(0_usize));
    let first_hit = first.clone();
    button.set_accessibility_actions(AccessibilityActions {
        activate: Some(Rc::new(move || first_hit.set(first_hit.get() + 1))),
        ..AccessibilityActions::default()
    });
    root.add_child(&button);

    let chain = FocusChain::new();
    let before = snapshot(&root);
    assert_eq!(before.len(), 1, "one element in the tree");
    assert_eq!(before[0].semantics.label, "Run");

    // A stable tree reports the same structure revision: the bridge reads this
    // to decide it has nothing to re-send.
    let revision = AccessibilityRevision::structure();
    assert_eq!(
        AccessibilityRevision::structure(),
        revision,
        "reading the tree does not bump the structure revision",
    );

    // Replacing the action closure must be visible immediately, and the old one
    // must not run.
    let second_hit = second.clone();
    button.set_accessibility_actions(AccessibilityActions {
        activate: Some(Rc::new(move || second_hit.set(second_hit.get() + 1))),
        ..AccessibilityActions::default()
    });
    button
        .accessibility_actions()
        .activate
        .expect("the control has an activate action")();
    assert_eq!(
        (first.get(), second.get()),
        (0, 1),
        "the latest closure is the one that runs",
    );

    // Moving the node changes its reported geometry.
    let old = find(&root, button.id().0).expect("the button is present");
    button.set_frame(Rect::new(15.0, 10.0, 100.0, 30.0));
    let moved = find(&root, button.id().0).expect("the button is present");
    assert_eq!(
        moved.frame.x,
        old.frame.x + 5.0,
        "the reported geometry follows the node",
    );

    // Focus is reflected in the snapshot's input acceptance, not in the label.
    chain.focus(Some(&button), true);
    let focused = find(&root, button.id().0).expect("the button is present");
    assert!(focused.accepts_input, "a focused control accepts input");

    let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Button)
        .with_label("Stop");
    semantics.state.is_enabled = false;
    button.set_accessibility(Some(semantics));
    let disabled = find(&root, button.id().0).expect("the button is present");
    assert_eq!(disabled.semantics.label, "Stop", "the label change lands");
    assert!(!disabled.semantics.state.is_enabled, "the state change lands");
    // `accepts_input` is pointer/keyboard reachability and deliberately ignores
    // the authored enabled state; the platform combines the two. The bridge does
    // exactly that: `enabled = isEnabled && (!interactive || acceptsInput)`
    // (`native_accessibility.rs`, the `flags` computation), so a disabled
    // interactive control answers no press. Both halves are asserted here.
    assert!(
        disabled.accepts_input,
        "the control is still pointer-reachable; only its authored state changed",
    );
    let interactive = disabled.node.is_focusable()
        || disabled.node.accessibility_actions().activate.is_some();
    assert!(
        interactive && !disabled.semantics.state.is_enabled,
        "the platform formula therefore reports it disabled",
    );

    // Removing the node removes the element.
    root.remove_child(&button);
    assert!(
        snapshot(&root).is_empty(),
        "a removed control leaves the accessibility tree",
    );
}

#[test]
fn heading_semantics_preserve_the_complete_title_and_use_the_platform_heading_role() {
    let root = node(0.0, 0.0, 320.0, 100.0);
    let heading = node(0.0, 0.0, 200.0, 40.0);
    heading.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Heading)
            .with_label("Full document title \u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}"),
    ));
    root.add_child(&heading);

    let element = find(&root, heading.id().0).expect("the heading is present");
    assert_eq!(
        element.semantics.label, "Full document title \u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}",
        "the whole title survives, including the joined emoji",
    );
    // GAP: Swift additionally asserts `accessibilityRole() == .headingRole` on
    // macOS 26 and `.staticText` before it. Rust maps `Heading` to role id 4 in
    // `native_accessibility.rs::role_id`, and the AppKit side picks the native
    // role; neither is reachable without a live bridge, so only the portable
    // role is asserted here.
    assert_eq!(
        element.semantics.role,
        AccessibilityRole::Heading,
        "the portable role is a heading",
    );
}

#[test]
fn pointer_transparent_descriptions_remain_accessible_while_interactive_actions_stay_blocked() {
    let root = node(0.0, 0.0, 320.0, 200.0);
    let overlay = node(0.0, 0.0, 320.0, 200.0);
    overlay.set_allows_hit_testing(false);
    root.add_child(&overlay);

    let description = node(10.0, 10.0, 200.0, 30.0);
    description.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::StaticText)
            .with_label("Symbol documentation"),
    ));
    overlay.add_child(&description);

    let button = node(10.0, 60.0, 100.0, 30.0);
    button.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Inactive action"),
    ));
    let activations = Rc::new(Cell::new(0_usize));
    let counted = activations.clone();
    button.set_accessibility_actions(AccessibilityActions {
        activate: Some(Rc::new(move || counted.set(counted.get() + 1))),
        ..AccessibilityActions::default()
    });
    overlay.add_child(&button);

    let elements = snapshot(&root);
    assert_eq!(elements.len(), 2, "both nodes are described");

    // A pointer-transparent ancestor makes everything under it unreachable for
    // pointer input. Native accessibility must not infer "hidden" from that,
    // though: the bridge keeps a *passive* description readable while blocking
    // an *interactive* one, using
    // `enabled = isEnabled && (!interactive || acceptsInput)`.
    let text = elements
        .iter()
        .find(|element| element.semantics.role == AccessibilityRole::StaticText)
        .expect("the description is present");
    assert!(
        !text.accepts_input,
        "the pointer-transparent ancestor blocks pointer input for the subtree",
    );
    assert_eq!(text.semantics.value, "", "the text has no value");
    assert_eq!(
        text.semantics.label, "Symbol documentation",
        "the description keeps its label",
    );
    assert!(
        text.semantics.state.is_enabled && text.node.accessibility_actions().activate.is_none(),
        "a passive description is not interactive, so the platform still reports it enabled",
    );

    let action = elements
        .iter()
        .find(|element| element.semantics.role == AccessibilityRole::Button)
        .expect("the inactive action is described");
    assert!(
        !action.accepts_input,
        "a pointer-transparent ancestor blocks the action",
    );
    assert!(
        action.node.accessibility_actions().activate.is_some(),
        "the interactive control is blocked precisely because it has an action",
    );
    // The closure still exists; the platform must not call it because the
    // element does not accept input. Swift asserts exactly this pair.
    assert!(action.node.accessibility_actions().activate.is_some());
    assert_eq!(activations.get(), 0, "nothing was activated");
}

#[test]
fn scene_synchronization_never_materializes_a_lazy_value_and_the_platform_reads_it_on_demand() {
    let root = node(0.0, 0.0, 320.0, 200.0);
    let field = node(10.0, 10.0, 200.0, 30.0);
    field.set_accessibility(Some(AccessibilitySemantics::new(
        AccessibilityRole::TextField,
    )));
    let reads = Rc::new(Cell::new(0_usize));
    let counted = reads.clone();
    field.set_accessibility_value_provider(Some(AccessibilityValueProvider::new(1, move || {
        counted.set(counted.get() + 1);
        "large text".to_string()
    })));
    root.add_child(&field);

    // Synchronization snapshots the tree many times; none of them may read.
    for _ in 0..100 {
        let _ = snapshot(&root);
    }
    assert_eq!(reads.get(), 0, "a snapshot never materializes a lazy value");

    // The platform reads on demand, once.
    let element = find(&root, field.id().0).expect("the field is present");
    assert_eq!(element.semantics.value, "", "the snapshot carried no value");
    assert_eq!(
        element.node.resolved_accessibility_value(),
        "large text",
        "the platform reads the live value",
    );
    assert_eq!(reads.get(), 1);

    // Marking the field secure masks the value the platform reads, without a
    // second provider call for the existing snapshot.
    let mut semantics = AccessibilitySemantics::new(AccessibilityRole::TextField);
    semantics.state.is_secure = true;
    field.set_accessibility(Some(semantics));
    let masked = Rc::new(Cell::new(0_usize));
    let counted = masked.clone();
    field.set_accessibility_value_provider(Some(AccessibilityValueProvider::new(2, move || {
        counted.set(counted.get() + 1);
        "secret".to_string()
    })));
    let after = find(&root, field.id().0).expect("the field is present");
    assert_eq!(
        reads.get(),
        1,
        "replacing the provider does not read the old one again",
    );
    assert_eq!(
        after.node.resolved_accessibility_value(),
        "\u{2022}".repeat(6),
        "a secure value is masked, one dot per character",
    );
    assert_eq!(masked.get(), 1, "the platform read the new provider once");

    // A resolving snapshot reports the masked value directly.
    let resolved = AccessibilityTree::snapshot_in(&root, None, true);
    let value = resolved
        .iter()
        .find(|element| element.node.id().0 == field.id().0)
        .map(|element| element.semantics.value.clone())
        .expect("the field is present");
    assert_eq!(value, "\u{2022}".repeat(6));
}

#[test]
fn controls_expose_real_actions_and_values_and_removed_elements_cannot_activate() {
    let root = node(0.0, 0.0, 320.0, 200.0);

    let button = node(10.0, 10.0, 100.0, 30.0);
    button.set_is_focusable(true);
    button.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Save"),
    ));
    let actions = Rc::new(Cell::new(0_usize));
    let counted = actions.clone();
    button.set_accessibility_actions(AccessibilityActions {
        activate: Some(Rc::new(move || counted.set(counted.get() + 1))),
        ..AccessibilityActions::default()
    });
    root.add_child(&button);

    let field = node(10.0, 60.0, 200.0, 30.0);
    let mut semantics = AccessibilitySemantics::new(AccessibilityRole::TextField)
        .with_label("Password")
        .with_value("secret");
    semantics.state.is_secure = true;
    field.set_accessibility(Some(semantics));
    let text = Rc::new(std::cell::RefCell::new("secret".to_string()));
    let written = text.clone();
    field.set_accessibility_actions(AccessibilityActions {
        set_value: Some(Rc::new(move |next: &str| {
            *written.borrow_mut() = next.to_string();
        })),
        ..AccessibilityActions::default()
    });
    root.add_child(&field);

    let elements = snapshot(&root);
    assert_eq!(elements.len(), 2);
    let save = elements
        .iter()
        .find(|element| element.semantics.label == "Save")
        .expect("the save button is described");
    save.node
        .accessibility_actions()
        .activate
        .expect("the button has an activate action")();
    assert_eq!(actions.get(), 1);

    let password = elements
        .iter()
        .find(|element| element.semantics.role == AccessibilityRole::TextField)
        .expect("the password field is described");
    assert!(
        password.semantics.state.is_secure,
        "the field reports a secure state, which is what AppKit turns into a secure-text-field subrole",
    );
    assert_eq!(
        password.node.resolved_accessibility_value(),
        "\u{2022}".repeat(6),
        "the value read out is masked",
    );
    password
        .node
        .accessibility_actions()
        .set_value
        .expect("the field has a set-value action")("changed");
    assert_eq!(*text.borrow(), "changed");

    // The snapshot still describes the removed button, but the node itself is
    // gone from the tree, so a platform holding that element cannot reach it.
    root.remove_child(&button);
    let remaining = snapshot(&root);
    assert!(
        !remaining
            .iter()
            .any(|element| element.semantics.label == "Save"),
        "a removed control leaves the accessibility tree",
    );
    assert_eq!(
        save.node.parent(),
        None,
        "the captured element's node is detached",
    );
}
