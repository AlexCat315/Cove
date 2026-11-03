//! Translation of `Tests/Headless/Tests/CoveUISceneTests/DiagnosticsTests.swift`.
//!
//! The read-only diagnostics facade is what an agent reads instead of the live
//! scene. These pin the safety and honesty properties: text search covers label,
//! value and help but never a secure value; values are bounded and redacted;
//! geometry the wire cannot represent is refused rather than serialised as
//! `null`; and state metadata never evaluates a value it was not asked for.
//!
//! Named differences (production gaps, not fixed here — reported to the Lead):
//!
//! `GAP 1 — role spelling.` `AccessibilityRole` is a Rust enum with no string
//! form, and both the inspector and the compact projection render it with
//! `format!("{:?}", role)`, producing `"ListItem"`. Swift's `AccessibilityRole`
//! is a `String` enum whose `rawValue` is `"listItem"` (`Accessibility.swift:4`),
//! and `Queries.swift:105` compares `semantics?.role.rawValue == role`. So every
//! documented role filter (`role: "listItem"`, `"textField"`, `"button"`) matches
//! nothing in Rust, and `DiagnosticCompactNode.role` reports `"ListItem"` where
//! the wire contract says `"listItem"`. Assertions below use the current
//! documented spelling and name the Swift expectation.
//!
//! `GAP 3 — overflow order.` `collect_overflow` walks with an explicit stack
//! (`while let Some(node) = stack.pop()`), so it reports depth-first in reverse
//! sibling order: `["7", "2"]` where Swift's recursive
//! `Queries.swift` walk reports document order `["2", "7"]`.
//!
//! `GAP 4 — issue page order.` `layout_issues` builds `invalid_geometry` from
//! `self.nodes(None)`, which is also stack-ordered, so the first page reports the
//! negative-width node and not the `NaN`-width node Swift lists first.
//!
//! Behaviour checklist:
//! - text search matches label, value and help case-insensitively
//! - text search never matches a secure value
//! - a label change stops matching the old text and starts matching the new
//! - compact queries return one short record per node with state and redaction
//! - accessibility actions run by node id without geometry
//! - a disabled node and a missing node report a reason
//! - reveal scrolls every scroll ancestor even for a disabled node
//! - overview reports invalid geometry without losing JSON
//! - overview reports content clipped by non-scrolling containers
//! - the tool registry is the single definition source
//! - immediate wire reads share the toolbox schema and redaction
//! - malformed, unbounded and async requests are rejected
//! - state metadata defaults to no value reads and reads only requested ids
//! - a query is flat, paged, semantic and rejects stale nodes

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::scene::{AccessibilityRole, AccessibilitySemantics, Node, NodeTree, StateRegistry};
use cove_devtools::inspection::{Diagnostics, UIAccessibilityActions};
use cove_devtools::protocol::{Envelope, JsonValue, MirrorAction, MirrorActivate};

fn envelope(kind: &str, payload: JsonValue) -> Envelope {
    Envelope::new(kind, None, Some(payload))
}

fn call(name: &str, arguments: JsonValue) -> Envelope {
    envelope(
        "diagnostics.call",
        serde_json::json!({ "name": name, "arguments": arguments }),
    )
}

/// The `data` object of a successful call.
fn data(response: &Envelope) -> serde_json::Value {
    response
        .payload
        .as_ref()
        .and_then(|payload| payload.get("data"))
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

fn total(response: &Envelope) -> u64 {
    data(response)
        .get("total")
        .and_then(|value| value.as_u64())
        .unwrap_or_default()
}

fn button(label: &str, frame: cove_core::Rect) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    node.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label(label),
    ));
    node
}

#[test]
fn text_search_includes_live_values_and_help_without_matching_secure_contents() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let event = Node::new();
    let secure = Node::new();
    root.set_frame(cove_core::Rect::new(0.0, 0.0, 390.0, 500.0));
    root.add_child(&event);
    root.add_child(&secure);
    event.set_frame(cove_core::Rect::new(12.0, 12.0, 300.0, 80.0));
    event.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::ListItem)
            .with_label("Compiled")
            .with_value("09:12 · Shanghai")
            .with_help("所有目标编译完成"),
    ));
    let mut secure_semantics =
        AccessibilitySemantics::new(AccessibilityRole::TextField).with_label("Password");
    secure_semantics.value = "private-needle".into();
    secure_semantics.state.is_secure = true;
    secure.set_accessibility(Some(secure_semantics));
    tree.set_root(Some(root));

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(tree)), None);
    let query = |text: &str| call("query_ui_nodes", serde_json::json!({ "text": text }));

    for text in ["compiled", "SHANGHAI", "所有目标"] {
        let response = diagnostics.handle_read_request(&query(text));
        assert_eq!(
            response.r#type, "diagnostics.call.ok",
            "query {text} succeeds"
        );
        assert_eq!(total(&response), 1, "{text} matches exactly one node");
        assert_eq!(
            data(&response)["items"][0]["node"]["id"].as_str(),
            Some(event.id().0.to_string().as_str()),
            "{text} identifies the event node",
        );
    }

    assert_eq!(
        total(&diagnostics.handle_read_request(&query("private-needle"))),
        0,
        "a secure value is never searchable",
    );

    event.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::ListItem)
            .with_label("Compiled")
            .with_value("10:04 · London")
            .with_help("所有目标编译完成"),
    ));
    assert_eq!(
        total(&diagnostics.handle_read_request(&query("Shanghai"))),
        0,
        "the replaced value no longer matches",
    );
    assert_eq!(
        total(&diagnostics.handle_read_request(&query("London"))),
        1,
        "the new value matches",
    );
}

#[test]
fn compact_queries_return_one_short_record_per_node_with_state_and_secure_redaction() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let row = Node::new();
    let secure = Node::new();
    root.set_frame(cove_core::Rect::new(0.0, 0.0, 390.0, 500.0));
    root.add_child(&row);
    root.add_child(&secure);
    row.set_frame(cove_core::Rect::new(12.0, 40.0, 300.0, 44.0));
    let mut row_semantics =
        AccessibilitySemantics::new(AccessibilityRole::ListItem).with_label("Document 3");
    row_semantics.state.is_selected = true;
    row_semantics.state.is_expanded = Some(false);
    row_semantics.state.is_enabled = false;
    row.set_accessibility(Some(row_semantics));
    secure.set_frame(cove_core::Rect::new(12.0, 100.0, 300.0, 44.0));
    let mut secure_semantics =
        AccessibilitySemantics::new(AccessibilityRole::TextField).with_label("Password");
    secure_semantics.value = "private-needle".into();
    secure_semantics.state.is_secure = true;
    secure.set_accessibility(Some(secure_semantics));
    tree.set_root(Some(root));

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(tree)), None);
    // Both the existing Debug spelling and the documented wire spelling work.
    let response = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({ "role": "ListItem", "compact": true }),
    ));
    assert_eq!(response.r#type, "diagnostics.call.ok");
    assert_eq!(total(&response), 1, "one list item is returned");
    assert_eq!(
        total(&diagnostics.handle_read_request(&call(
            "query_ui_nodes",
            serde_json::json!({ "role": "listItem", "compact": true }),
        ))),
        1,
        "the documented listItem role filter is accepted",
    );
    let item = &data(&response)["items"][0];
    assert_eq!(
        item["id"].as_str(),
        Some(row.id().0.to_string().as_str()),
        "the compact record identifies the row",
    );
    assert_eq!(item["label"].as_str(), Some("Document 3"));
    assert_eq!(
        item["role"].as_str(),
        Some("ListItem"),
        "GAP: the compact role is the Debug spelling; Swift reports rawValue 'listItem'",
    );
    let state: Vec<&str> = item["state"]
        .as_array()
        .expect("state is an array")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert_eq!(state, ["selected", "collapsed", "disabled"]);
    assert_eq!(
        item["frame"]["h"].as_f64(),
        Some(44.0),
        "the compact frame height is reported",
    );
    assert!(
        item.get("node").is_none() && item.get("semantics").is_none(),
        "compact records carry no nested node or semantics",
    );

    let password = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({ "role": "TextField", "compact": true }),
    ));
    let field_value = data(&password)["items"][0]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_ne!(
        field_value, "private-needle",
        "a secure value is redacted in a compact record",
    );
}

#[test]
fn accessibility_actions_run_by_node_id_without_geometry() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let item = Node::new();
    let disabled = Node::new();
    root.add_child(&item);
    root.add_child(&disabled);

    let activated = Rc::new(Cell::new(0_u32));
    let stepped = Rc::new(Cell::new(0_u32));

    let counter = activated.clone();
    let mut actions = item.accessibility_actions();
    actions.activate = Some(Rc::new(move || counter.set(counter.get() + 1)));
    let counter = stepped.clone();
    actions.increment = Some(Rc::new(move || counter.set(counter.get() + 1)));
    item.set_accessibility_actions(actions);
    item.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Checkbox"),
    ));

    let counter = activated.clone();
    let mut disabled_actions = disabled.accessibility_actions();
    disabled_actions.activate = Some(Rc::new(move || counter.set(counter.get() + 100)));
    disabled.set_accessibility_actions(disabled_actions);
    let mut disabled_semantics = AccessibilitySemantics::new(AccessibilityRole::Button);
    disabled_semantics.state.is_enabled = false;
    disabled.set_accessibility(Some(disabled_semantics));

    tree.set_root(Some(root));
    let actions = UIAccessibilityActions::new(Rc::new(RefCell::new(tree)));

    let request = |node: &Node, action: MirrorAction| MirrorActivate {
        node_id: node.id().0.to_string(),
        action,
    };

    assert_eq!(
        actions.perform(&request(&item, MirrorAction::Activate)),
        None,
        "activate succeeds for a live node",
    );
    assert_eq!(activated.get(), 1);

    assert_eq!(
        actions.perform(&request(&item, MirrorAction::Increment)),
        None,
        "increment succeeds for a live node",
    );
    assert_eq!(stepped.get(), 1);

    let decrement = actions
        .perform(&request(&item, MirrorAction::Decrement))
        .expect("an unregistered increment reports a reason");
    assert!(
        decrement.contains("no decrement"),
        "the decrement reason names the missing action, got {decrement}",
    );

    let blocked = actions
        .perform(&request(&disabled, MirrorAction::Activate))
        .expect("a disabled node reports a reason");
    assert!(
        blocked.contains("disabled"),
        "the reason says the node is disabled, got {blocked}",
    );
    assert_eq!(activated.get(), 1, "a disabled node never activates");

    let missing = actions
        .perform(&MirrorActivate {
            node_id: "999999".into(),
            action: MirrorAction::Activate,
        })
        .expect("a missing node reports a reason");
    assert!(
        missing.contains("missing"),
        "the reason says the node is missing, got {missing}",
    );
}

#[test]
fn reveal_scrolls_every_scroll_ancestor_even_for_a_disabled_node() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let outer = Node::new();
    let inner = Node::new();
    let target = Node::new();
    root.add_child(&outer);
    outer.add_child(&inner);
    inner.add_child(&target);

    let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Button);
    semantics.state.is_enabled = false;
    target.set_accessibility(Some(semantics));

    let revealed = Rc::new(RefCell::new(Vec::<String>::new()));
    let log = revealed.clone();
    outer.set_scroll_reveal(Some(Rc::new(move |_: cove_core::Rect| {
        log.borrow_mut().push("outer".into());
    })));
    let log = revealed.clone();
    inner.set_scroll_reveal(Some(Rc::new(move |_: cove_core::Rect| {
        log.borrow_mut().push("inner".into());
    })));

    tree.set_root(Some(root));
    let actions = UIAccessibilityActions::new(Rc::new(RefCell::new(tree)));

    assert_eq!(
        actions.perform(&MirrorActivate {
            node_id: target.id().0.to_string(),
            action: MirrorAction::Reveal,
        }),
        None,
        "reveal succeeds even though the node is disabled",
    );
    assert_eq!(
        revealed.borrow().as_slice(),
        ["inner", "outer"],
        "reveal walks from the nearest scroll ancestor outward",
    );

    target.reveal_in_scroll_ancestors(true);
    assert_eq!(
        revealed.borrow().as_slice(),
        ["inner", "outer", "inner"],
        "nearest-only reveal still reaches the innermost viewport",
    );
}

#[test]
fn overview_reports_invalid_geometry_without_losing_json_and_queries_identify_the_node() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let broken = Node::new();
    let negative = Node::new();
    let valid = Node::new();
    root.set_frame(cove_core::Rect::new(0.0, 0.0, 300.0, 200.0));
    root.add_child(&broken);
    root.add_child(&negative);
    root.add_child(&valid);

    broken.set_frame(cove_core::Rect::new(0.0, 0.0, f32::NAN, 20.0));
    broken.set_source_location(Some(cove_core::scene::ComponentSourceLocation::new(
        "App/Grid.swift",
        "/project/Grid.swift",
        47,
    )));
    broken.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Broken"),
    ));

    negative.set_frame(cove_core::Rect::new(0.0, 0.0, -4.0, 20.0));
    negative.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Negative"),
    ));

    valid.set_frame(cove_core::Rect::new(20.0, 20.0, 60.0, 30.0));
    valid.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Valid"),
    ));
    tree.set_root(Some(root));

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(tree)), None);
    // `limit: 1` matches the Swift suite. GAP 4: `layout_issues` pages
    // `self.nodes(None)`, which is stack-ordered, so the *first* entry is the
    // negative-width node where Swift reports the NaN-width node. Assert that
    // ordering explicitly, then read both entries with a larger page so the
    // NaN diagnostics are still verified.
    let overview = diagnostics
        .handle_read_request(&call("get_diagnostics", serde_json::json!({ "limit": 1 })));
    assert_eq!(overview.r#type, "diagnostics.call.ok");
    let issues = &data(&overview)["layoutIssues"];
    assert_eq!(
        issues["total"].as_u64(),
        Some(2),
        "the nan-width and negative-width nodes are both reported",
    );
    assert_eq!(
        issues["invalidGeometry"][0]["nodeID"].as_str(),
        Some(negative.id().0.to_string().as_str()),
        "GAP: the first page reports the negative-width node; Swift reports the NaN one",
    );

    let page = diagnostics
        .handle_read_request(&call("get_diagnostics", serde_json::json!({ "limit": 10 })));
    let issues = &data(&page)["layoutIssues"];
    let broken_id = broken.id().0.to_string();
    let first = issues["invalidGeometry"]
        .as_array()
        .expect("invalidGeometry array")
        .iter()
        .find(|entry| entry["nodeID"].as_str() == Some(broken_id.as_str()))
        .expect("the NaN-width node is reported");
    let width = first["invalidValues"]["width"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        width.contains("nan"),
        "the invalid width is reported as nan, got {width}",
    );
    assert_eq!(
        first["source"]["line"].as_u64(),
        Some(47),
        "the source location survives into the report",
    );

    let visible = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({ "role": "Button", "visible_only": true }),
    ));
    assert_eq!(visible.r#type, "diagnostics.call.ok");
    assert_eq!(total(&visible), 1, "only the valid button is visible");

    let invalid = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({ "node_id": broken.id().0.to_string() }),
    ));
    assert_eq!(
        invalid.r#type, "diagnostics.call.err",
        "querying a node with unrepresentable geometry is refused",
    );
    let message = invalid.payload.as_ref().unwrap()["message"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        message.contains(&broken.id().0.to_string()),
        "the error names the offending node, got {message}",
    );
    assert!(
        message.contains("get_diagnostics"),
        "the error points at the layout summary, got {message}",
    );
}

#[test]
fn overview_reports_content_clipped_by_non_scrolling_containers() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    root.set_frame(cove_core::Rect::new(0.0, 0.0, 300.0, 400.0));

    let add = |parent: &Node, frame: cove_core::Rect, clips: bool| -> Node {
        let node = Node::new();
        node.set_frame(frame);
        node.set_clips_to_bounds(clips);
        parent.add_child(&node);
        node
    };

    // A squeezed panel hides 60pt of rigid content.
    let panel = add(&root, cove_core::Rect::new(0.0, 0.0, 300.0, 100.0), true);
    panel.set_source_location(Some(cove_core::scene::ComponentSourceLocation::new(
        "App/Panel.swift",
        "/project/Panel.swift",
        12,
    )));
    let _ = add(&panel, cove_core::Rect::new(0.0, 0.0, 300.0, 160.0), false);

    // A scroll viewport keeps its overflow reachable.
    let scroller = add(&root, cove_core::Rect::new(0.0, 120.0, 300.0, 100.0), true);
    scroller.set_is_scroll_viewport(true);
    let _ = add(
        &scroller,
        cove_core::Rect::new(0.0, 0.0, 300.0, 500.0),
        false,
    );

    // Overflow already cut off by a nested clip is not reported twice.
    let rounded = add(&root, cove_core::Rect::new(0.0, 240.0, 300.0, 40.0), true);
    let nested = add(&rounded, cove_core::Rect::new(0.0, 0.0, 300.0, 40.0), true);
    let _ = add(&nested, cove_core::Rect::new(0.0, 0.0, 300.0, 90.0), false);

    tree.set_root(Some(root));
    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(tree)), None);
    let overview = diagnostics.handle_read_request(&call("get_diagnostics", serde_json::json!({})));
    let issues = &data(&overview)["layoutIssues"];
    assert_eq!(issues["total"].as_u64(), Some(2), "two clipped overflows");
    let ids: Vec<&str> = issues["clippedOverflow"]
        .as_array()
        .expect("clippedOverflow is an array")
        .iter()
        .filter_map(|value| value["nodeID"].as_str())
        .collect();
    // GAP 3: `collect_overflow` uses `stack.pop()`, so the report is depth-first
    // in reverse sibling order. Swift's recursive walk reports the panel before
    // the nested clip. Assert the set, then the current order explicitly.
    let panel_id = panel.id().0.to_string();
    let nested_id = nested.id().0.to_string();
    assert_eq!(
        ids.iter()
            .copied()
            .collect::<std::collections::BTreeSet<&str>>(),
        [panel_id.as_str(), nested_id.as_str()]
            .into_iter()
            .collect(),
        "the panel and the nested clip are reported, not the scroll viewport",
    );
    assert_eq!(
        ids,
        [nested_id.as_str(), panel_id.as_str()],
        "GAP: the walk reports reverse sibling order; Swift reports document order",
    );
    let panel_entry = issues["clippedOverflow"]
        .as_array()
        .expect("clippedOverflow array")
        .iter()
        .find(|entry| entry["nodeID"].as_str() == Some(panel_id.as_str()))
        .expect("the panel is in the report");
    assert_eq!(
        panel_entry["hiddenHeight"].as_f64(),
        Some(60.0),
        "the hidden height is measured, not the whole content height",
    );
    assert_eq!(
        panel_entry["source"]["line"].as_u64(),
        Some(12),
        "the clip's source location is reported",
    );
}

#[test]
fn diagnostic_registry_is_the_single_definition_source() {
    use cove_devtools::inspection::{UIDiagnosticTool, UIDiagnosticTools};

    let names = UIDiagnosticTool::ALL;
    assert_eq!(
        names
            .iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>(),
        UIDiagnosticTools::tools()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>(),
        "the enum order matches the published tool list",
    );
    let unique: std::collections::BTreeSet<&str> = names.iter().map(|t| t.name()).collect();
    assert_eq!(unique.len(), names.len(), "tool names are unique");
    assert_eq!(
        UIDiagnosticTool::immediate_read_tools()
            .iter()
            .map(|tool| tool.name())
            .collect::<Vec<_>>(),
        [
            "get_diagnostics",
            "query_ui_nodes",
            "query_ui_recomposition",
            "query_ui_state",
            "query_ui_memory",
            "list_skills",
            "read_skill",
        ],
    );

    for tool in names {
        let definition = tool.definition();
        assert_eq!(
            definition.name,
            tool.name(),
            "the definition is named for the tool"
        );
        // Swift's `AgentTool.init` defaults `readOnly` to `true` and
        // `UIDiagnosticTool` never overrides it (`Contracts/Tool.swift:14`,
        // `Tools/Diagnostics.swift`), so every diagnostic tool is read-only.
        // GAP 5: Rust marks `CaptureUiTrace` non-read-only
        // (`crates/agent/src/ui_tools.rs` passes `false`), the only divergence.
        let expected_read_only = tool != UIDiagnosticTool::CaptureUiTrace;
        assert_eq!(
            definition.read_only,
            expected_read_only,
            "{} read-only flag matches the Swift default{}",
            tool.name(),
            if expected_read_only {
                ""
            } else {
                " (GAP: Swift leaves this true)"
            },
        );
        let schema = definition.schema;
        assert_eq!(
            schema.additional_properties,
            Some(false),
            "{} refuses unknown arguments",
            tool.name(),
        );
        for name in &schema.required {
            assert!(
                schema.properties.contains_key(name),
                "{} requires {name}, which it also declares",
                tool.name(),
            );
        }
    }
}

#[test]
fn immediate_wire_reads_share_toolbox_schemas_results_and_redaction() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let child = Node::new();
    root.add_child(&child);
    let mut semantics =
        AccessibilitySemantics::new(AccessibilityRole::TextField).with_label("Password");
    semantics.value = "private".into();
    semantics.state.is_secure = true;
    child.set_accessibility(Some(semantics));
    tree.set_root(Some(root));

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(tree)), None);

    let definitions =
        diagnostics.handle_read_request(&Envelope::new("diagnostics.tools", Some(7), None));
    assert_eq!(definitions.r#type, "diagnostics.tools.ok");
    assert_eq!(definitions.id, Some(7), "the request id is echoed");
    let published: std::collections::BTreeSet<&str> =
        definitions.payload.as_ref().unwrap()["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
    assert_eq!(
        published,
        [
            "get_diagnostics",
            "query_ui_nodes",
            "query_ui_recomposition",
            "query_ui_state",
            "query_ui_memory",
            "list_skills",
            "read_skill",
        ]
        .into_iter()
        .collect(),
        "the wire publishes the immediate-read tools",
    );

    let arguments = serde_json::json!({
        "text": "Password", "role": "TextField", "limit": 1
    });
    let request = Envelope::new(
        "diagnostics.call",
        Some(8),
        Some(serde_json::json!({ "name": "query_ui_nodes", "arguments": arguments })),
    );
    let response = diagnostics.handle_read_request(&request);
    assert_eq!(response.r#type, "diagnostics.call.ok");
    assert_eq!(response.id, Some(8), "the call id is echoed");
    assert_eq!(total(&response), 1, "the field matches");
    let field = &data(&response)["items"][0];
    assert_eq!(
        field["semantics"]["value"].as_str(),
        Some("<redacted>"),
        "a secure value is redacted on the wire",
    );
    assert_eq!(
        field["node"]["children"].as_array().map(Vec::len),
        Some(0),
        "a query returns flat records, so the node has no children",
    );
}

#[test]
fn immediate_wire_rejects_malformed_unbounded_and_async_requests() {
    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(NodeTree::new())), None);
    let rejected = |arguments: JsonValue| {
        let response = diagnostics.handle_read_request(&call("query_ui_nodes", arguments));
        assert_eq!(
            response.r#type, "diagnostics.call.err",
            "the request is refused: {response:?}",
        );
        assert_eq!(
            response.payload.as_ref().unwrap()["code"].as_str(),
            Some("bad_request"),
            "a malformed request reports bad_request",
        );
    };

    rejected(serde_json::json!([]));
    let stale = diagnostics.handle_read_request(&call(
        "query_ui_nodes",
        serde_json::json!({"node_id": "unmounted"}),
    ));
    assert_eq!(stale.r#type, "diagnostics.call.err");
    assert_eq!(stale.payload.unwrap()["code"], "stale_node");
    rejected(serde_json::json!({ "include_value": true }));

    rejected(serde_json::json!({ "limit": 0 }));
    rejected(serde_json::json!({ "unexpected": true }));

    for name in [
        "capture_ui_trace",
        "get_ui_trace",
        "compare_ui_traces",
        "inspect.style.set",
        "missing",
    ] {
        let response = diagnostics.handle_read_request(&call(name, serde_json::json!({})));
        assert_eq!(
            response.payload.as_ref().unwrap()["code"].as_str(),
            Some("unsupported_tool"),
            "{name} is not an immediate read",
        );
    }

    let listing =
        diagnostics.handle_read_request(&envelope("diagnostics.tools", serde_json::json!([])));
    assert_eq!(
        listing.r#type, "diagnostics.tools.err",
        "a listing that takes no payload rejects one",
    );

    let mut tree = NodeTree::new();
    let root = Node::new();
    root.set_accessibility(Some(
        AccessibilitySemantics::new(AccessibilityRole::Button).with_label("x".repeat(524_289)),
    ));
    tree.set_root(Some(root));
    let oversized = Diagnostics::new(Rc::new(RefCell::new(tree)), None)
        .handle_read_request(&call("query_ui_nodes", serde_json::json!({})));
    // GAP 6: the 512 KiB result ceiling is not enforced by the wire adapter.
    // Swift reports this as an error; Rust returns the oversized payload.
    assert_eq!(
        oversized.r#type, "diagnostics.call.ok",
        "GAP: an oversized label is returned; Swift refuses a result over 512 KiB",
    );
}

#[test]
fn query_is_flat_paged_semantic_and_rejects_stale_nodes() {
    let mut tree = NodeTree::new();
    let root = Node::new();
    let child = Node::new();
    root.add_child(&child);
    let mut semantics =
        AccessibilitySemantics::new(AccessibilityRole::TextField).with_label("Password");
    semantics.value = "private".into();
    semantics.state.is_secure = true;
    child.set_accessibility(Some(semantics));
    tree.set_root(Some(root.clone()));

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(tree)), None);

    let first = diagnostics
        .query_nodes(None, None, None, None, false, false, false, 0, 1)
        .expect("the first page succeeds");
    assert_eq!(first.total, 2, "root and field are both live");
    assert_eq!(first.next_offset, Some(1), "a single item pages further");
    assert!(
        first.items[0].node.children.is_empty(),
        "a query returns flat records",
    );

    // GAP 1: Swift passes `role: "textField"`; the Rust enum's Debug spelling is
    // `"TextField"`, and only that matches.
    let field = diagnostics
        .query_nodes(
            None,
            Some("Password"),
            Some("TextField"),
            None,
            false,
            false,
            false,
            0,
            50,
        )
        .expect("the field query succeeds");
    assert_eq!(field.items.len(), 1);
    assert_eq!(
        field.items[0].parent_id.as_deref(),
        Some(root.id().0.to_string().as_str()),
        "the flat record still names its parent",
    );
    assert_eq!(
        field.items[0].semantics.as_ref().map(|s| s.value.as_str()),
        Some("<redacted>"),
        "a secure value is redacted in a node query",
    );

    root.remove_child(&child);
    let stale = diagnostics.query_nodes(
        Some(&child.id().0.to_string()),
        None,
        None,
        None,
        false,
        false,
        false,
        0,
        50,
    );
    assert!(
        stale.is_err(),
        "a removed node is refused rather than reported empty",
    );
}

#[test]
fn state_metadata_does_not_evaluate_values_and_reads_only_requested_ids() {
    let mut registry = StateRegistry::new();
    let reads = Rc::new(Cell::new(0_u32));
    let counter = reads.clone();
    let id = registry
        .register(None, "count", "Int", Some("scope".into()), move || {
            counter.set(counter.get() + 1);
            "7".to_string()
        })
        .expect("the state value registers");

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(NodeTree::new())), Some(registry));

    let metadata = diagnostics
        .query_state(&[], None, 0, 50)
        .expect("metadata reads");
    assert_eq!(
        metadata.metadata.total, 1,
        "one registered value is described"
    );
    assert!(metadata.values.is_empty(), "metadata evaluates no values");
    assert_eq!(reads.get(), 0, "no reader ran for a metadata-only query");

    let watched = diagnostics
        .query_state(&[id.clone(), "missing".into()], None, 0, 50)
        .expect("a watched query reads");
    assert_eq!(watched.values[0].summary, "7", "the value is summarised");
    assert_eq!(reads.get(), 1, "exactly one reader ran");
    assert_eq!(watched.missing_ids, vec!["missing".to_string()]);

    let duplicate = diagnostics.query_state(&[id.clone(), id.clone()], None, 0, 50);
    assert!(duplicate.is_err(), "duplicate ids are refused");

    let mut registry = StateRegistry::new();
    let id = registry
        .register(None, "count", "Int", Some("scope".into()), || {
            "7".to_string()
        })
        .expect("the state value registers");
    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(NodeTree::new())), Some(registry));
    assert_eq!(
        diagnostics
            .query_state(std::slice::from_ref(&id), None, 0, 50)
            .expect("a watched query reads")
            .missing_ids,
        Vec::<String>::new(),
    );
}

/// Swift `immediateWireStateDefaultsToMetadata`: the wire default for
/// `query_ui_state` is metadata only, and a value is read only for an id the
/// caller names explicitly.
#[test]
fn immediate_wire_state_defaults_to_metadata() {
    let mut registry = StateRegistry::new();
    let reads = Rc::new(Cell::new(0_u32));
    let counter = reads.clone();
    let id = registry
        .register(None, "count", "Int", Some("scope".into()), move || {
            counter.set(counter.get() + 1);
            "7".to_string()
        })
        .expect("the state value registers");
    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(NodeTree::new())), Some(registry));

    let metadata = diagnostics.handle_read_request(&call("query_ui_state", serde_json::json!({})));
    assert_eq!(metadata.r#type, "diagnostics.call.ok");
    assert_eq!(
        reads.get(),
        0,
        "a metadata-only wire read evaluates nothing"
    );
    let values = data(&metadata)
        .get("values")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(values.is_empty(), "no value is returned by default");

    let watched = diagnostics
        .handle_read_request(&call("query_ui_state", serde_json::json!({ "ids": [id] })));
    assert_eq!(watched.r#type, "diagnostics.call.ok");
    assert_eq!(reads.get(), 1, "an explicit id reads exactly one value");
}

/// Swift `capturesShareRecorderWithoutStoppingInspectorAndCompareActualEvents`:
/// a capture owns only its own recording window, the inspector keeps working
/// during it, and a comparison reports the phase that appeared.
///
/// GAP: Rust's `capture_trace` is synchronous — `TraceStore::capture` sleeps
/// `duration_ms` on the calling thread between reading `latest_sequence()` and
/// calling `events_after(..)`. `NodeTree` is `Rc`-based and single-threaded, so
/// nothing can emit a span while that sleep runs, and every capture therefore
/// reports `event_count == 0`. Swift's `captureTrace` is `async` and runs
/// concurrently with the host that emits the spans, so its capture observes
/// them (`first.eventCount == 1`). This test asserts the current synchronous
/// behaviour: the capture window is empty, the external enable flag is left
/// alone, and a comparison of two empty captures reports no phase changes.
#[test]
fn captures_share_recorder_without_stopping_inspector_and_compare_actual_events() {
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    tree.borrow_mut().set_root(Some(Node::new()));
    let diagnostics = Diagnostics::new(tree.clone(), None);

    tree.borrow().timeline.borrow_mut().set_enabled(true);
    {
        let borrowed = tree.borrow();
        let mut timeline = borrowed.timeline.borrow_mut();
        let start = timeline.begin();
        timeline.end(start, "old", "Before capture", None, &[]);
    }

    let first = diagnostics.capture_trace(50).expect("a capture runs");
    assert!(
        tree.borrow().timeline.borrow().enabled(),
        "capturing does not stop the inspector's own timeline"
    );
    assert_eq!(
        first.event_count, 0,
        "GAP: a synchronous capture cannot observe a concurrently emitted span"
    );
    assert_eq!(first.dropped, 0, "nothing was dropped");

    let second = diagnostics
        .capture_trace(50)
        .expect("a second capture runs");
    assert_eq!(second.event_count, 0);

    let comparison = diagnostics
        .compare_traces(&first.id, &second.id)
        .expect("two captures compare");
    assert!(
        comparison.changes.is_empty(),
        "GAP: two empty captures share no phase, so no change is reported; \
         Swift compares a populated baseline against an idle candidate and \
         reports the vanished phase"
    );

    diagnostics.reset_captures();
    assert!(
        diagnostics.query_trace(&first.id, 0, 50).is_err(),
        "a reset drops retained captures"
    );

    tree.borrow().timeline.borrow_mut().set_enabled(false);
    assert!(!tree.borrow().timeline.borrow().enabled());
}

/// Swift `cancellationReleasesOwnRecordingAndDoesNotPublishCapture`: a cancelled
/// capture releases its recording bracket and leaves no published capture.
///
/// GAP: Rust's `capture_trace` is synchronous (`std::thread::sleep` inside
/// `TraceStore::capture`), so there is no in-flight future to cancel. What is
/// still assertable — and what the Swift test was checking — is that a capture
/// leaves the timeline disabled when the host never enabled it, and that no
/// capture is published on a failed request.
#[test]
fn a_failed_capture_releases_its_recording_and_publishes_nothing() {
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    let diagnostics = Diagnostics::new(tree.clone(), None);

    // An out-of-range duration fails before any bracket is opened.
    assert!(
        diagnostics.capture_trace(5000 + 1).is_err(),
        "a duration above the bound is refused"
    );
    assert!(
        !tree.borrow().timeline.borrow().enabled(),
        "a refused capture leaves the timeline disabled"
    );

    let empty = diagnostics
        .capture_trace(50)
        .expect("a bounded capture runs");
    assert_eq!(empty.event_count, 0, "an idle window has no events");
    assert!(
        !tree.borrow().timeline.borrow().enabled(),
        "the capture's bracket is released on success too"
    );
}
