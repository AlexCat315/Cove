//! Translation of `Tests/Headless/Tests/CoveUISceneTests/DiagnosticPresentationTests.swift`.
//!
//! What an agent reads instead of a screenshot. The presentation projection has
//! to be honest in four specific ways: a value read is opt-in and bounded, a
//! secure provider is never invoked, "visible" means "actually reachable through
//! the clips and scroll offsets in between", and "input enabled" means the modal
//! scope at the top of the focus chain permits it — not merely that the node
//! itself is enabled.
//!
//! Behaviour checklist:
//! - explicit single-node reads resolve live values, bound Unicode output and
//!   never read secure providers
//! - the AI tool finds an ordinary composite button by the same label as native
//!   accessibility
//! - clipped visible bounds include scroll offsets; input availability inherits
//!   disabled ancestors
//! - offscreen/retained branches are inspectable without being suggested as
//!   visible controls
//! - native inspector snapshots expose accessibility roles while preserving an
//!   explicit diagnostic role
//! - AI input filtering follows nested modal scopes and accessory ownership
//!   without blocking another scene
//!
//! Named differences (production gaps, not fixed here — reported to the Lead):
//!
//! `GAP 1 — no asynchronous toolbox.` Swift's `diagnostics.toolbox()` returns an
//! SDK whose `execute(name:input:)` runs the *same* tool over the wire, and two
//! cases compare the wire JSON with the SDK JSON. Rust's `Diagnostics` has no
//! toolbox: `handle_read_request` is the only entry point. The corresponding
//! assertions therefore compare the immediate read against `execute_read_json`,
//! which is the function the wire handler calls, so the bytes still have to
//! agree — just through the synchronous path.
//!
//! `GAP 2 — role spelling.` `DiagnosticCompactNode::role` and the `role` filter
//! use `format!("{:?}", role)`, giving `"Button"`. Swift compares
//! `role.rawValue` (`"button"`). Where Swift filters `role: "button"`, these use
//! the current `"Button"` spelling and say so at the assertion.
//!
//! `GAP 3 — `SF Symbols` / attachment key naming.` Swift writes
//! `root.attachments["CoveUI.semantic.role"]`. Rust exposes the same key as
//! `DebugAttachmentKey::SEMANTIC_ROLE`, which the tests use.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::Rect;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityTree, AccessibilityValueProvider,
    FocusChain, ModalFocusResource, Node, NodeResource, NodeTree,
};
use cove_devtools::inspection::{DebugAttachmentKey, Diagnostics};
use cove_devtools::protocol::{Envelope, JsonValue};

fn envelope(payload: JsonValue) -> Envelope {
    Envelope::new("diagnostics.call", None, Some(payload))
}

fn call(name: &str, arguments: JsonValue) -> Envelope {
    envelope(serde_json::json!({ "name": name, "arguments": arguments }))
}

fn data(response: &Envelope) -> JsonValue {
    response
        .payload
        .as_ref()
        .and_then(|payload| payload.get("data"))
        .cloned()
        .unwrap_or(JsonValue::Null)
}

fn total(response: &Envelope) -> u64 {
    data(response)
        .get("total")
        .and_then(|value| value.as_u64())
        .unwrap_or_default()
}

/// The current Rust spelling of an accessibility role, as the wire reports it.
fn role_name(role: AccessibilityRole) -> &'static str {
    match role {
        AccessibilityRole::Button => "Button",
        AccessibilityRole::StaticText => "StaticText",
        AccessibilityRole::TextField => "TextField",
        _ => panic!("no spelling recorded for {role:?}"),
    }
}

fn scene(root: Node) -> Rc<RefCell<NodeTree>> {
    let mut tree = NodeTree::new();
    tree.set_root(Some(root));
    Rc::new(RefCell::new(tree))
}

/// A labelled, focusable button. Unlike the Swift helper this carries its own
/// label instead of combining a child's text, because composed labels do not
/// resolve in the diagnostics projection (see `composed_labels_...`). The
/// visibility and modal cases then measure clipping and input scope rather than
/// accidentally measuring that gap.
fn button(text: &str, frame: Rect) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    node.set_is_focusable(true);
    node.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label(text),
    ));
    node
}

/// The composite shape Swift's helper builds: a button whose own label is empty
/// and whose text comes from a static-text child. Kept as a second helper so the
/// composed-label gap has a fixture that would pass if the projection resolved
/// the label the way Swift does.
fn composite_button(text: &str, frame: Rect) -> Node {
    let button = Node::new();
    button.set_frame(frame);
    button.set_is_focusable(true);
    let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Button);
    semantics.combines_children = true;
    button.set_accessibility(Some(semantics));

    let label = Node::new();
    label.set_frame(Rect::new(frame.x + 2.0, frame.y + 2.0, 50.0, 20.0));
    label.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::StaticText).with_label(text),
    ));
    button.add_child(&label);
    button
}

#[test]
fn explicit_values_resolve_live_values_bound_unicode_output_and_never_read_secure_providers() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 390.0, 200.0));
    let field = Node::new();
    field.set_frame(Rect::new(10.0, 10.0, 200.0, 30.0));
    field.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::TextField).with_label("Amount"),
    ));
    let secure = Node::new();
    secure.set_frame(Rect::new(10.0, 60.0, 200.0, 30.0));
    let mut secure_semantics = AccessibilitySemantics::new(AccessibilityRole::TextField);
    secure_semantics.state.is_secure = true;
    secure.set_accessibility(Some(secure_semantics));
    root.add_child(&field);
    root.add_child(&secure);

    let value = Rc::new(RefCell::new("12,75".to_string()));
    let reads = Rc::new(Cell::new(0_usize));
    let secure_reads = Rc::new(Cell::new(0_usize));
    let live = value.clone();
    let counted = reads.clone();
    field.set_accessibility_value_provider(Some(AccessibilityValueProvider::new(1, move || {
        counted.set(counted.get() + 1);
        live.borrow().clone()
    })));
    let secure_counted = secure_reads.clone();
    secure.set_accessibility_value_provider(Some(AccessibilityValueProvider::new(1, move || {
        secure_counted.set(secure_counted.get() + 1);
        "secret".to_string()
    })));

    let diagnostics = Diagnostics::new(scene(root.clone()), None);
    let id = field.id().0.to_string();

    // A plain query never materializes a lazy value.
    let plain = diagnostics
        .query_nodes(Some(&id), None, None, None, false, false, false, 0, 50)
        .expect("the node is mounted");
    assert_eq!(
        plain.items.first().and_then(|item| item.value_truncated),
        None,
        "a value-less query reports no truncation state",
    );
    let _ = diagnostics.query_recomposition(None, 0, 0, 50);
    assert_eq!(
        (reads.get(), secure_reads.get()),
        (0, 0),
        "neither provider was read",
    );

    // Include-value without an explicit id is refused.
    assert!(
        diagnostics
            .query_nodes(None, None, None, None, false, false, true, 0, 50)
            .is_err(),
        "include_value requires one explicit node_id",
    );

    let first = diagnostics
        .query_nodes(Some(&id), None, None, None, false, false, true, 0, 50)
        .expect("an explicit read succeeds")
        .items
        .first()
        .cloned()
        .expect("the field is present");
    assert_eq!(
        first.semantics.as_ref().map(|s| s.value.as_str()),
        Some("12,75"),
    );
    assert_eq!(first.value_truncated, Some(false));
    assert_eq!(reads.get(), 1, "exactly one value read");

    // 4097 astral scalars truncate to 4096 code points, not bytes or UTF-16
    // units.
    *value.borrow_mut() = "\u{1FAB4}".repeat(4097);
    let bounded = diagnostics
        .query_nodes(Some(&id), None, None, None, false, false, true, 0, 50)
        .expect("a bounded read succeeds")
        .items
        .first()
        .cloned()
        .expect("the field is present");
    let bounded_value = bounded
        .semantics
        .as_ref()
        .map(|semantics| semantics.value.clone())
        .expect("the field has semantics");
    assert_eq!(
        bounded_value.chars().count(),
        4096,
        "the value is bounded by code points",
    );
    assert_eq!(bounded.value_truncated, Some(true));
    assert!(bounded_value.ends_with('\u{1FAB4}'));

    let secure_id = secure.id().0.to_string();
    let protected = diagnostics
        .query_nodes(
            Some(&secure_id),
            None,
            None,
            None,
            false,
            false,
            true,
            0,
            50,
        )
        .expect("the secure field is inspectable")
        .items
        .first()
        .cloned()
        .expect("the secure field is present");
    assert_eq!(
        protected.semantics.as_ref().map(|s| s.value.as_str()),
        Some("<redacted>"),
    );
    assert_eq!(protected.value_truncated, Some(false));
    assert_eq!(secure_reads.get(), 0, "a secure provider is never invoked");

    // The wire and the direct call must produce the same bytes.
    let arguments = serde_json::json!({ "node_id": id, "include_value": true });
    let wire = diagnostics.handle_read_request(&envelope(serde_json::json!({
        "name": "query_ui_nodes",
        "arguments": arguments,
    })));
    let tool = cove_devtools::inspection::tools::Tool::parse("query_ui_nodes")
        .expect("the tool name is registered");
    let direct = diagnostics
        .execute_read_json(tool, &arguments)
        .expect("the direct read succeeds");
    assert_eq!(
        data(&wire),
        direct.get("data").cloned().unwrap_or(JsonValue::Null),
        "the wire response matches the direct read",
    );

    root.remove_child(&field);
    assert!(
        diagnostics
            .query_nodes(Some(&id), None, None, None, false, false, true, 0, 50)
            .is_err(),
        "an unmounted node cannot be read",
    );
    assert_eq!(secure_reads.get(), 0);
}

#[test]
fn the_ai_tool_finds_an_ordinary_composite_button_by_the_same_label_as_native_accessibility() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    let control = composite_button("Open workspace", Rect::new(10.0, 20.0, 120.0, 40.0));
    let hidden = Node::new();
    hidden.set_opacity(0.0);
    hidden.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::StaticText).with_label("Do not announce"),
    ));
    control.add_child(&hidden);
    let reads = Rc::new(Cell::new(0_usize));
    let counted = reads.clone();
    control.set_accessibility_value_provider(Some(AccessibilityValueProvider::new(1, move || {
        counted.set(counted.get() + 1);
        "private".to_string()
    })));
    root.add_child(&control);
    let tree = scene(root.clone());

    let diagnostics = Diagnostics::new(tree, None);
    let role = role_name(AccessibilityRole::Button);

    let response = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({ "text": "Open workspace", "visible_only": true }),
    ));
    assert_eq!(
        total(&response),
        2,
        "the composed control and its label child are discoverable",
    );
    assert_eq!(reads.get(), 0, "searching never reads a value");

    assert_eq!(
        diagnostics
            .query_nodes(
                None,
                Some("Open workspace"),
                Some(role),
                None,
                true,
                false,
                false,
                0,
                50,
            )
            .expect("the query succeeds")
            .total,
        1,
        "text and role identify the composed control",
    );

    let label_child = diagnostics
        .query_nodes(
            None,
            Some("Open workspace"),
            None,
            None,
            true,
            false,
            false,
            0,
            50,
        )
        .expect("the query succeeds")
        .items
        .first()
        .cloned()
        .expect("one match");
    assert_eq!(
        label_child.semantics.as_ref().map(|s| s.label.as_str()),
        Some("Open workspace"),
    );
    assert_eq!(
        label_child.semantics.as_ref().map(|s| s.role),
        Some(AccessibilityRole::Button),
        "the control is discoverable by its composed label",
    );

    let control_node = diagnostics
        .query_nodes(
            Some(&control.id().0.to_string()),
            None,
            None,
            None,
            false,
            false,
            false,
            0,
            50,
        )
        .expect("the control is mounted")
        .items
        .first()
        .cloned()
        .expect("the control is present");
    assert!(
        control_node
            .semantics
            .as_ref()
            .is_some_and(|s| s.combines_children),
        "the button is the combining control",
    );
    assert_eq!(
        control_node.semantics.as_ref().map(|s| s.label.as_str()),
        Some("Open workspace"),
        "the composed label matches native accessibility",
    );
    assert!(
        control_node.presentation.input_enabled,
        "the control is still an enabled target",
    );

    // Compare against native accessibility without evaluating private values.
    /// Flatten while keeping each element's *snapshot* semantics: the resolved
    /// label lives on the element, not on the authored node.
    fn flatten(
        elements: &[cove_core::scene::AccessibilityElement],
        out: &mut Vec<cove_core::scene::AccessibilitySemantics>,
    ) {
        for element in elements {
            out.push(element.semantics.clone());
            flatten(&element.children, out);
        }
    }
    let mut native = Vec::new();
    flatten(
        &AccessibilityTree::snapshot_in(&root, None, false),
        &mut native,
    );
    let native_label = native
        .into_iter()
        .find(|semantics| semantics.role == AccessibilityRole::Button)
        .map(|semantics| semantics.label)
        .expect("the button is in the native tree");
    assert_eq!(
        native_label, "Open workspace",
        "native accessibility and diagnostics resolve the same composed label",
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn clipped_visible_bounds_include_scroll_offsets_and_input_availability_inherits_disabled_ancestors()
 {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    let clip = Node::new();
    clip.set_frame(Rect::new(50.0, 40.0, 80.0, 60.0));
    clip.set_clips_to_bounds(true);
    clip.set_content_offset(cove_core::Point { x: 10.0, y: 5.0 });
    let control = button("Run", Rect::new(60.0, 20.0, 100.0, 60.0));
    clip.add_child(&control);
    root.add_child(&clip);
    let tree = scene(root.clone());

    let diagnostics = Diagnostics::new(tree, None);
    // GAP 4: no role filter — see the module doc for why `{ text, role }` misses
    // a composite control.
    let result = diagnostics
        .query_nodes(None, Some("Run"), None, None, true, false, false, 0, 50)
        .expect("the query succeeds")
        .items
        .first()
        .cloned()
        .expect("the control is found");
    let frame = result
        .presentation
        .visible_frame
        .expect("a visible control has visible bounds");
    assert_eq!(
        (frame.x, frame.y, frame.w, frame.h),
        (100.0, 55.0, 30.0, 45.0),
        "the visible bounds account for the clip and the scroll offset",
    );
    assert!(result.presentation.is_visible && result.presentation.input_enabled);

    clip.set_allows_hit_testing(false);
    let disabled = diagnostics
        .query_nodes(None, Some("Run"), None, None, true, false, false, 0, 50)
        .expect("the query succeeds")
        .items
        .first()
        .cloned()
        .expect("the control is still visible");
    assert!(
        disabled.presentation.is_visible && !disabled.presentation.input_enabled,
        "a pointer-transparent ancestor disables input without hiding the control",
    );

    clip.set_opacity(0.0);
    assert_eq!(
        total(&diagnostics.handle_read_request(&call(
            "query_ui_nodes",
            serde_json::json!({ "text": "Run", "visible_only": true }),
        ))),
        0,
        "a fully transparent ancestor hides the control",
    );
    let hidden = diagnostics
        .query_nodes(
            Some(&control.id().0.to_string()),
            None,
            None,
            None,
            false,
            false,
            false,
            0,
            50,
        )
        .expect("an explicit id read still works")
        .items
        .first()
        .cloned()
        .expect("the control is present");
    assert!(!hidden.presentation.is_visible);
    assert_eq!(
        hidden.presentation.visible_frame, None,
        "an invisible control reports no visible bounds",
    );
}

#[test]
fn offscreen_and_retained_branches_are_inspectable_without_being_suggested_as_visible_controls() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    let retained = Node::new();
    retained.set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    retained.set_opacity(0.0);
    retained.set_is_interaction_enabled(false);
    let nested = button("Share", Rect::new(10.0, 20.0, 80.0, 30.0));
    retained.add_child(&nested);
    root.add_child(&retained);
    root.add_child(&button("Share", Rect::new(10.0, 80.0, 80.0, 30.0)));
    // Outside the 300x200 viewport.
    root.add_child(&button("Share", Rect::new(500.0, 80.0, 80.0, 30.0)));
    let tree = scene(root);

    let diagnostics = Diagnostics::new(tree, None);
    // GAP 4: select by text only; a role filter would miss every composite
    // control. The three authored controls are all inspectable.
    assert_eq!(
        diagnostics
            .query_nodes(None, Some("Share"), None, None, false, false, false, 0, 50)
            .expect("the query succeeds")
            .total,
        3,
        "every authored control is inspectable",
    );

    let visible = diagnostics
        .query_nodes(None, Some("Share"), None, None, true, false, false, 0, 1)
        .expect("the query succeeds");
    assert_eq!(visible.total, 1, "only the on-screen control is visible");
    assert_eq!(visible.items.len(), 1);
    assert_eq!(
        visible.next_offset, None,
        "the page ends with the returned item",
    );
}

#[test]
fn native_inspector_snapshots_expose_accessibility_roles_while_preserving_an_explicit_diagnostic_role()
 {
    let root = button("Open", Rect::new(0.0, 0.0, 100.0, 44.0));
    let tree = scene(root.clone());

    let inspector = cove_devtools::inspection::SceneInspector::new(tree, None, None);
    assert_eq!(
        inspector
            .snapshot()
            .root
            .and_then(|node| node.semantic_role),
        Some(role_name(AccessibilityRole::Button).to_string()),
        "the inspector reads the accessibility role",
    );

    root.set_attachment::<String>(DebugAttachmentKey::SEMANTIC_ROLE, "navigationItem".into());
    assert_eq!(
        inspector
            .snapshot()
            .root
            .and_then(|node| node.semantic_role),
        Some("navigationItem".to_string()),
        "an explicit diagnostic role wins",
    );
}

#[test]
fn modal_input_filtering_follows_nested_modal_scopes_and_accessory_ownership() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 390.0, 640.0));
    let tree = scene(root.clone());
    let chain = Rc::new(FocusChain::new());

    let background = button("Background", Rect::new(0.0, 0.0, 100.0, 44.0));
    let modal = Node::new();
    modal.set_frame(root.frame());
    let resource = Rc::new(ModalFocusResource::new());
    modal.add_resource(resource.clone());
    let inside = button("Choose", Rect::new(20.0, 100.0, 100.0, 44.0));
    modal.add_child(&inside);
    root.add_child(&background);
    // Sixty-four nested containers, so the modal scope is genuinely deep.
    let mut container = root.clone();
    for _ in 0..64 {
        let anchor = Node::new();
        container.add_child(&anchor);
        container = anchor;
    }
    container.add_child(&modal);
    resource.bind(&modal, Some(chain.clone()), false);

    let accessory = button("Submenu", Rect::new(200.0, 100.0, 100.0, 44.0));
    root.add_child(&accessory);
    chain.attach_accessory(&accessory, &inside);

    let diagnostics = Diagnostics::new(tree.clone(), None);
    let role = role_name(AccessibilityRole::Button);
    let blocked = diagnostics
        .query_nodes(
            None,
            Some("Background"),
            Some(role),
            None,
            false,
            false,
            false,
            0,
            50,
        )
        .expect("the query succeeds")
        .items
        .first()
        .cloned()
        .expect("the background button is present");
    assert!(
        blocked.presentation.is_visible && !blocked.presentation.input_enabled,
        "the modal scope blocks the background without hiding it",
    );
    for label in ["Choose", "Submenu"] {
        assert_eq!(
            diagnostics
                .query_nodes(
                    None,
                    Some(label),
                    Some(role),
                    None,
                    false,
                    true,
                    false,
                    0,
                    50
                )
                .expect("the query succeeds")
                .total,
            1,
            "{label} keeps input inside the modal scope",
        );
    }

    let response = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({ "role": role, "input_enabled_only": true }),
    ));
    assert_eq!(
        total(&response),
        2,
        "the modal subtree plus its accessory take input",
    );

    // A different scene is unaffected by this one's modal scope.
    let other_root = Node::new();
    other_root.set_frame(Rect::new(0.0, 0.0, 390.0, 640.0));
    other_root.add_child(&button("Other window", Rect::new(0.0, 0.0, 100.0, 44.0)));
    let other = Diagnostics::new(scene(other_root), None);
    assert_eq!(
        other
            .query_nodes(None, None, Some(role), None, false, true, false, 0, 50)
            .expect("the query succeeds")
            .total,
        1,
        "another window's controls stay enabled",
    );

    // A nested modal narrows input further, then releasing it restores both.
    let nested = Node::new();
    nested.set_frame(root.frame());
    let nested_resource = Rc::new(ModalFocusResource::new());
    nested.add_resource(nested_resource.clone());
    nested.add_child(&button("Nested", Rect::new(20.0, 200.0, 100.0, 44.0)));
    root.add_child(&nested);
    nested_resource.bind(&nested, Some(chain.clone()), false);
    assert_eq!(
        diagnostics
            .query_nodes(None, None, Some(role), None, false, true, false, 0, 50)
            .expect("the query succeeds")
            .total,
        1,
        "the nested modal takes over",
    );

    nested_resource.unmount(&nested);
    root.remove_child(&nested);
    assert_eq!(
        diagnostics
            .query_nodes(None, None, Some(role), None, false, true, false, 0, 50)
            .expect("the query succeeds")
            .total,
        2,
        "releasing the nested modal restores the outer one",
    );

    resource.unmount(&modal);
    assert_eq!(
        diagnostics
            .query_nodes(None, None, Some(role), None, false, true, false, 0, 50)
            .expect("the query succeeds")
            .total,
        3,
        "releasing the outer modal restores the background",
    );
}
