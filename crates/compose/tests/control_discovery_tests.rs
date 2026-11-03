//! Translation of `Tests/CoveUIComposeTests/Agent/ControlDiscoveryTests.swift`.
//!
//! Behaviour checklist:
//! - an explicit accessibility group is discoverable by label and role and
//!   reports its authored 300x120 frame
//! - zero, one and two action buttons are counted through role-filtered
//!   discovery, and the label filter matches only the authored button
//! - a Form field and its footer button are discoverable by label and role
//! - a discovered control reports a visible, input-enabled presentation with a
//!   visible frame
//! - a field's private draft is absent from the default record and present when
//!   the record asks for that node's value
//! - an unfiltered default page serialises inside the 512 KiB tool limit
//!
//! Named differences from the Swift suite:
//! - `UIDiagnostics` and the agent toolbox live in `cove-devtools` and
//!   `cove-agent`, which `cove-ui-compose` does not depend on. The suite
//!   reproduces the shipped `query_nodes` predicate, its presentation projection
//!   and its record shape locally, the same approach as
//!   `sidebar_hierarchy_tests.rs`, and asserts the contract on `Node`.
//! - Role filters use the raw-value spelling (`"group"`, `"button"`,
//!   `"textField"`), matching Swift's `role.rawValue` comparison.
//! - A Rust `Button` does not derive its accessible label from its title text,
//!   so the harness labels each button explicitly; the Swift suite relied on the
//!   implicit label.
//! - A field's draft is private: the default record omits it and only an
//!   `include_value` request with an explicit `node_id` reads it.

use crate::support;

use serde_json::{Value, json};

use cove_composition::ViewGraph;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, Rect};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::form::form::{Form, FormField};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::TextField;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;

use support::{GlobalTestLock, with_test_text_environment};

const DRAFT: &str = "Local private draft";
const DEFAULT_PAGE_LIMIT: usize = 50;
const TOOL_RESULT_LIMIT: usize = 512 * 1024;

/// One query hit, mirroring `DiagnosticNode`'s fields the suite reads.
struct Record {
    node: Node,
    semantics: Option<AccessibilitySemantics>,
    is_visible: bool,
    input_enabled: bool,
    visible_frame: Option<Rect>,
}

/// One page of hits, mirroring `DiagnosticPage`.
struct Page {
    items: Vec<Record>,
    total: usize,
}

impl Page {
    fn json(&self, include_value: bool) -> Value {
        json!({
            "total": self.total,
            "items": self
                .items
                .iter()
                .map(|record| record.json(include_value))
                .collect::<Vec<_>>(),
        })
    }

    fn first(&self) -> &Record {
        self.items.first().expect("the query returned an item")
    }
}

/// The wire spelling of a role: Swift publishes the raw value (`textField`).
fn role_name(role: AccessibilityRole) -> String {
    serde_json::to_value(role)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{role:?}"))
}

impl Record {
    fn json(&self, include_value: bool) -> Value {
        // A field's draft is only read on an explicit request; the default
        // record reports the meaning.
        let value = if include_value {
            self.node.resolved_accessibility_value()
        } else if self
            .semantics
            .as_ref()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
        {
            String::new()
        } else {
            self.semantics
                .as_ref()
                .map(|semantics| semantics.value.clone())
                .unwrap_or_default()
        };
        json!({
            "node": { "id": self.node.id().0.to_string() },
            "semantics": {
                "role": self.semantics.as_ref().map(|s| role_name(s.role)),
                "label": self.semantics.as_ref().map(|s| s.label.clone()),
                "value": value,
            },
            "presentation": {
                "isVisible": self.is_visible,
                "inputEnabled": self.input_enabled,
                "visibleFrame": self.visible_frame.map(|frame| json!({
                    "x": frame.x,
                    "y": frame.y,
                    "w": frame.width,
                    "h": frame.height,
                })),
            },
        })
    }
}

struct Query<'a> {
    text: Option<&'a str>,
    role: Option<&'a str>,
    visible_only: bool,
    input_enabled_only: bool,
}

impl<'a> Query<'a> {
    fn new() -> Self {
        Self {
            text: None,
            role: None,
            visible_only: false,
            input_enabled_only: false,
        }
    }

    fn text(mut self, text: &'a str) -> Self {
        self.text = Some(text);
        self
    }

    fn role(mut self, role: &'a str) -> Self {
        self.role = Some(role);
        self
    }

    fn visible_only(mut self) -> Self {
        self.visible_only = true;
        self
    }
}

fn all_nodes(graph: &ViewGraph) -> Vec<Node> {
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            walk(&child, into);
        }
    }
    let mut nodes = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        walk(&root, &mut nodes);
    }
    nodes
}

fn record(node: Node) -> Record {
    let frame = node.absolute_frame();
    let is_visible = node.is_layout_visible() && frame.width > 0.0 && frame.height > 0.0;
    Record {
        semantics: node.accessibility(),
        input_enabled: is_visible && node.accepts_subtree_input(),
        is_visible,
        visible_frame: is_visible.then_some(frame),
        node,
    }
}

/// The shipped `UIDiagnostics.query_nodes` predicate, minus the filters this
/// suite does not use.
fn query(graph: &ViewGraph, query: Query<'_>) -> Page {
    let text = query.text.map(str::to_lowercase);
    let matches: Vec<Record> = all_nodes(graph)
        .into_iter()
        .map(record)
        .filter(|item| {
            if query.visible_only && !item.is_visible {
                return false;
            }
            if query.input_enabled_only && !item.input_enabled {
                return false;
            }
            if let Some(text) = &text {
                let semantics = item.semantics.as_ref();
                let candidates = [
                    support::debug_name(&item.node),
                    item.node.view_tag(),
                    semantics.map(|s| s.label.clone()),
                    semantics.map(|s| s.value.clone()),
                    semantics.map(|s| s.help.clone()),
                ];
                if !candidates
                    .into_iter()
                    .flatten()
                    .any(|candidate| candidate.to_lowercase().contains(text))
                {
                    return false;
                }
            }
            if let Some(role) = query.role {
                let matches = item
                    .semantics
                    .as_ref()
                    .is_some_and(|semantics| format!("{:?}", semantics.role) == role);
                if !matches {
                    return false;
                }
            }
            true
        })
        .collect();
    Page {
        total: matches.len(),
        items: matches.into_iter().take(DEFAULT_PAGE_LIMIT).collect(),
    }
}

fn settle(graph: &ViewGraph, width: f32, height: f32, passes: usize) {
    for _ in 0..passes {
        graph.recomposer.commit_all();
        graph.compute_layout(width, height);
    }
}

fn install_and_settle<V: cove_composition::View + 'static>(
    view: V,
    width: f32,
    height: f32,
    passes: usize,
) -> std::rc::Rc<ViewGraph> {
    let graph = support::graph();
    graph.install(view, Default::default());
    settle(&graph, width, height, passes);
    graph
}

fn asset_actions(count: usize) -> impl cove_composition::View + 'static {
    let mut buttons = Vec::new();
    if count >= 1 {
        buttons.push(cove_composition::AnyView::new(
            Button::new(Text::new("Pick asset"), || {}).accessibility_label("Pick asset"),
        ));
    }
    if count >= 2 {
        buttons.push(cove_composition::AnyView::new(
            Button::new(Text::new("Clear asset"), || {}).accessibility_label("Clear asset"),
        ));
    }
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        cove_composition::AnyView::new(cove_composition::TupleView::new(buttons)),
    )
    .frame_size(300.0, 120.0)
    .accessibility(|semantics| {
        semantics.role = cove_core::scene::AccessibilityRole::Group;
        semantics.label = "Asset actions".to_string();
    })
}

#[test]
fn explicit_groups_preserve_zero_one_and_multiple_action_roles() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            for count in 0..=2 {
                let graph = install_and_settle(asset_actions(count), 390.0, 200.0, 4);

                let group = query(&graph, Query::new().text("Asset actions").role("Group"));
                assert_eq!(
                    group.total, 1,
                    "the group is discoverable by label and role for count {count}"
                );
                let frame = group.first().node.absolute_frame();
                assert_eq!(
                    (frame.width, frame.height),
                    (300.0, 120.0),
                    "the group reports its authored frame for count {count}"
                );

                let buttons = query(&graph, Query::new().role("Button"));
                assert_eq!(
                    buttons.total, count,
                    "role-filtered discovery reports {count} buttons"
                );

                let pick = query(&graph, Query::new().text("Pick asset").role("Button"));
                assert_eq!(
                    pick.total,
                    usize::from(count > 0),
                    "the label filter matches only the authored button for count {count}"
                );
            }
        });
    });
}

fn form_view() -> impl cove_composition::View + 'static {
    Form::try_new(vec![FormField::new(
        "name",
        "Display name",
        TextField::new("Name", Binding::constant(TextBuffer::new(DRAFT))),
    )])
    .expect("unique form field IDs")
    .footer(Button::new(Text::new("Save changes"), || {}).accessibility_label("Save changes"))
    .frame_size(640.0, 400.0)
}

#[test]
fn validated_tools_discover_form_fields_and_buttons_without_private_values() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = install_and_settle(form_view(), 640.0, 400.0, 8);

            for (text, role) in [("Display name", "TextField"), ("Save changes", "Button")] {
                let page = query(&graph, Query::new().text(text).role(role).visible_only());
                assert_eq!(
                    page.total, 1,
                    "{role} `{text}` is discoverable exactly once"
                );

                let item = page.first();
                assert!(
                    item.is_visible && item.input_enabled,
                    "{role} `{text}` is visible and accepts input"
                );
                assert!(
                    item.visible_frame.is_some(),
                    "{role} `{text}` reports a visible frame"
                );

                let default_json = page.json(false).to_string();
                if role == "TextField" {
                    // A field's draft is private; the default record reports the
                    // meaning only. Reading it takes an explicit value request.
                    assert!(
                        !default_json.contains(DRAFT),
                        "the default projection omits the draft: {default_json}"
                    );
                } else {
                    assert!(
                        !default_json.contains(DRAFT),
                        "`{text}` does not carry the field's private draft: {default_json}"
                    );
                }

                if role == "TextField" {
                    let id = item.node.id().0.to_string();
                    let record = query(&graph, Query::new())
                        .items
                        .into_iter()
                        .find(|candidate| candidate.node.id().0.to_string() == id)
                        .expect("the field stays mounted");
                    let value_json = json!({
                        "node": { "id": id },
                        "semantics": {
                            "value": record.node.resolved_accessibility_value(),
                        },
                    })
                    .to_string();
                    assert!(
                        value_json.contains(DRAFT),
                        "an explicit value record includes the field's draft"
                    );
                }
            }

            let all = query(&graph, Query::new()).json(false);
            let encoded = serde_json::to_string(&all).expect("the page serialises");
            assert!(
                encoded.len() <= TOOL_RESULT_LIMIT,
                "the unfiltered default page is {} bytes, within the {TOOL_RESULT_LIMIT} byte limit",
                encoded.len()
            );
        });
    });
}
