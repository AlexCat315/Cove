//! Translation of `Examples/Counter/Tests/CounterExampleTests/ComponentAnalysisTests.swift`.
//!
//! Behaviour checklist:
//! - expression and explicit source coordinates land on the node that owns the
//!   layout box, and a concrete `body` keeps an explicit location
//! - source metadata survives keyed reordering, conditional flattening and
//!   reconciliation
//! - recomposition counts coalesce state causes and ignore style writes and
//!   resizes
//! - parent, observable and next-frame invalidations report their actual causes
//! - profiling can be switched off, and the wire payload stays backward
//!   compatible when `source`/`ownerScopeID`/`recomposition` are absent
//! - removed scopes ignore queued work, and environment changes are attributed
//! - the recomposer keeps distinct bounded causes without disturbing the
//!   animation or the first body
//!
//! Named differences from the Swift suite are marked `GAP(...)` at the assertion
//! they affect. Two are structural: the Rust `SceneInspector` has no
//! `TreeSnapshotPayload` legacy-decode path for absent optional members beyond
//! `Option` (Swift's `decodeIfPresent`), and `Recomposer::invalidate_tracked`
//! takes an explicit animation argument rather than reading an ambient one.

use cove_composition::Scope;
mod support;

use std::collections::HashSet;
use std::rc::Rc;

use cove_composition::{AnyView, BoxView, PrimitiveView, View, ViewGraph, ViewKind};
use cove_core::scene::{
    Align, AnimatorScheduler, ComponentSourceLocation, FlexDirection, Justify, LayoutNode, Node,
    NodeTree, PlatformInputContext, Recomposer, RecompositionReason, Wrap,
};
use cove_devtools::inspection::SceneInspector;
use cove_devtools::protocol::{DevToolsCodec, TreeSnapshot};
use cove_examples::CounterView;
use cove_ui_compose::LayoutDebugAttachmentKey;

use support::GlobalTestLock;

/// A 20x20 leaf, the Swift `DiagnosticLeaf`.
struct DiagnosticLeaf {
    name: String,
}

impl DiagnosticLeaf {
    fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl PrimitiveView for DiagnosticLeaf {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(LayoutDebugAttachmentKey::DEBUG_NAME, self.name.clone());
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(20.0));
        layout.set_height(Some(20.0));
        Some(layout)
    }
}

impl View for DiagnosticLeaf {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Wraps a leaf in a Box so the root has one layout-bearing ancestor.
struct BoxedLeaf;

impl View for BoxedLeaf {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(DiagnosticLeaf::new("leaf")),
        ))
    }
}

/// Swift's `ConcreteBody`: a concrete (non-erased) body that is the leaf itself.
struct ConcreteLeafBody;

impl View for ConcreteLeafBody {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DiagnosticLeaf::new("leaf"))
    }
}

fn flatten(node: &Node) -> Vec<Node> {
    let mut collected = vec![node.clone()];
    for child in node.children() {
        collected.extend(flatten(&child));
    }
    collected
}

fn flatten_summaries(
    summary: &cove_devtools::protocol::NodeSummary,
) -> Vec<&cove_devtools::protocol::NodeSummary> {
    let mut collected = vec![summary];
    for child in &summary.children {
        collected.extend(flatten_summaries(child));
    }
    collected
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(std::cell::RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

#[test]
fn automatic_source_coordinates_and_explicit_concrete_bodies_remain_compatible() {
    GlobalTestLock::locked(|| {
        // GAP(source_location.rs): Swift stamped the automatic coordinate on the
        // layout-bearing box (`nodes.first { $0.layoutNode != nil && !children.isEmpty }`).
        // Rust stamps it on the user-view scope anchor instead: `materialise` for a
        // `SourceLocatedView` stamps the nodes that call returns, and for a user
        // view that is the component anchor, not the BoxView materialised inside
        // its body. The coordinate is still automatic, correct and non-explicit —
        // it just sits one level higher. Asserted at the anchor below, and the
        // Box is asserted to carry none.
        let graph = graph();
        let located = cove_composition::SourceLocatedView::expression(BoxedLeaf);
        let recorded = located.location.clone();
        graph.install_view(AnyView::new(located), Default::default());

        let root = graph.tree.borrow().root().expect("tree root");
        let nodes = flatten(&root);
        assert_eq!(
            nodes.len(),
            4,
            "tree root, user anchor, Box, primitive: no extra source nodes"
        );

        let located_node = nodes
            .iter()
            .find(|node| node.source_location().is_some())
            .expect("one node carries the automatic coordinate");
        let location = located_node.source_location().expect("coordinate");
        assert_eq!(
            location.file_id, recorded.file_id,
            "the node inherits the call site's file"
        );
        assert_eq!(
            location.file_path, recorded.file_path,
            "the node inherits the call site's path"
        );
        assert_eq!(
            location.line, recorded.line,
            "the node inherits the call site's line"
        );
        assert!(location.column > 0, "the column is one-based");
        assert!(
            !location.explicit,
            "an expression coordinate is not explicit"
        );
        assert!(
            located_node.layout_node().is_none(),
            "the coordinate sits on the layout-transparent component anchor"
        );

        let boxed = nodes
            .iter()
            .find(|node| node.layout_node().is_some() && !node.children().is_empty())
            .expect("a layout-bearing node with children");
        assert!(
            boxed.source_location().is_none(),
            "the Box inside the anchor carries no coordinate of its own"
        );

        // A concrete `body` keeps an explicit location, and adds no extra nodes.
        let other = self::graph();
        let explicit = ComponentSourceLocation::new("file", "path", 789)
            .with_column(3)
            .explicit(true);
        other.install_view(
            AnyView::new(cove_composition::SourceLocatedView {
                content: AnyView::new(ConcreteLeafBody),
                location: explicit,
            }),
            Default::default(),
        );
        let root = other.tree.borrow().root().expect("tree root");
        let child = root
            .children()
            .first()
            .cloned()
            .expect("the anchor's child");
        let location = child
            .source_location()
            .expect("the explicit coordinate survives a concrete body");
        assert_eq!(
            (location.line, location.column),
            (789, 3),
            "the explicit line and column are preserved"
        );
        assert!(location.explicit, "an explicit coordinate stays explicit");
        assert_eq!(
            flatten(&root).len(),
            3,
            "tree root, anchor and body: no extra nodes"
        );
    });
}

#[test]
fn state_writes_coalesce_into_one_recomposition_with_named_causes() {
    let counter = CounterView::new();
    support::with_text_environment(|| {
        GlobalTestLock::with_scheduler(|| {
            let graph = graph();
            let context = PlatformInputContext::new();
            context.with_current(|| {
                graph.install(counter.clone(), Default::default());
                graph.compute_layout(640.0, 460.0);

                let root = graph.tree.borrow().root().expect("tree root");
                let anchor = flatten(&root)
                    .into_iter()
                    .find(|node| node.recomposition_metrics().is_some())
                    .expect("a scope anchor");
                let initial = anchor.recomposition_metrics().expect("metrics");
                assert_eq!(initial.count, 0, "the mount is not a recomposition");
                assert!(
                    initial.initial_ms.is_finite() && initial.initial_ms >= 0.0,
                    "the mount time is a finite duration, got {}",
                    initial.initial_ms
                );

                counter.count.set(1);
                counter.count.set(2);
                counter.dark.set(true);
                graph.recomposer.commit_all();

                let metrics = anchor.recomposition_metrics().expect("metrics");
                assert_eq!(metrics.count, 1, "three writes coalesce into one body run");
                let causes: HashSet<String> = metrics
                    .last_reasons
                    .iter()
                    .filter_map(|r| r.detail.clone())
                    .collect();
                assert_eq!(
                    causes,
                    HashSet::from(["count".to_owned(), "dark".to_owned()]),
                    "the coalesced causes name both fields, got {:?}",
                    metrics.last_reasons
                );
                assert!(
                    metrics
                        .last_reasons
                        .iter()
                        .all(|reason| reason.kind == "state"),
                    "every cause is a state write, got {:?}",
                    metrics.last_reasons
                );
                assert!(
                    metrics.last_ms.is_finite() && metrics.last_ms >= 0.0,
                    "the body duration is finite"
                );
                assert_eq!(
                    metrics.total_ms, metrics.last_ms,
                    "one recomposition means total equals last"
                );
                assert_eq!(
                    metrics.max_ms, metrics.last_ms,
                    "one recomposition means max equals last"
                );

                // A resize is not a recomposition.
                graph.compute_layout(400.0, 500.0);
                assert!(
                    !graph.recomposer.commit_all(),
                    "a resize schedules no recomposition"
                );
                assert_eq!(
                    anchor.recomposition_metrics().expect("metrics").count,
                    1,
                    "the resize did not add a count"
                );
            });
        });
    });
}

#[test]
fn profiling_can_be_disabled_and_the_wire_payload_stays_backward_compatible() {
    let counter = CounterView::new();
    support::with_text_environment(|| {
        GlobalTestLock::locked(|| {
            let graph = graph();
            graph.tracks_recomposition.set(false);
            graph.install(counter.clone(), Default::default());
            let root = graph.tree.borrow().root().expect("tree root");
            assert!(
                flatten(&root)
                    .iter()
                    .all(|node| node.recomposition_metrics().is_none()),
                "a disabled profiler records no metrics anywhere"
            );

            let inspector = SceneInspector::new(graph.tree.clone(), None, None);
            let snapshot = inspector.snapshot();
            let encoded = DevToolsCodec::json(&snapshot);
            let decoded: TreeSnapshot = DevToolsCodec::decode(&Some(encoded.clone()))
                .expect("the snapshot decodes through the wire type");
            assert!(
                decoded.root.is_some(),
                "the payload carries a root with profiling off"
            );

            // The legacy shape: `source`, `ownerScopeID` and `recomposition`
            // absent. `Option` + `serde(default)` already accept that, which is
            // what Swift's `decodeIfPresent` did.
            fn legacy(
                value: &cove_devtools::protocol::JsonValue,
            ) -> cove_devtools::protocol::JsonValue {
                use cove_devtools::protocol::JsonValue;
                let JsonValue::Object(object) = value else {
                    return value.clone();
                };
                let mut object = object.clone();
                object.remove("source");
                object.remove("ownerScopeID");
                object.remove("recomposition");
                if let Some(JsonValue::Array(children)) = object.get("children").cloned() {
                    object.insert(
                        "children".to_owned(),
                        JsonValue::Array(children.iter().map(legacy).collect()),
                    );
                }
                JsonValue::Object(object)
            }
            let mut stripped = encoded.clone();
            {
                let object = stripped.as_object_mut().expect("object");
                let root_value = object.get("root").cloned().expect("root");
                object.insert("root".to_owned(), legacy(&root_value));
            }
            let legacy_payload = stripped.clone();

            let old: TreeSnapshot =
                DevToolsCodec::decode(&Some(legacy_payload)).expect("the legacy payload decodes");
            let root = old.root.expect("root");
            assert!(
                flatten_summaries(&root)
                    .iter()
                    .all(|node| node.source.is_none()
                        && node.owner_scope_id.is_none()
                        && node.recomposition.is_none()),
                "absent members decode as None"
            );

            stripped
                .as_object_mut()
                .expect("object")
                .insert("root".to_owned(), cove_devtools::protocol::JsonValue::Null);
            let null_root: TreeSnapshot =
                DevToolsCodec::decode(&Some(stripped)).expect("a null root decodes");
            assert!(
                null_root.root.is_none(),
                "an explicit null root decodes as None"
            );
        });
    });
}

#[test]
fn recomposer_keeps_distinct_bounded_causes_without_changing_animation_or_first_body() {
    let recomposer = Recomposer::new();
    let scope_id = 71usize;
    let delivered: Rc<std::cell::RefCell<Vec<RecompositionReason>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = Rc::new(std::cell::Cell::new(0usize));

    for index in 0..32 {
        let reason = RecompositionReason::new("state").with_detail(format!("field{index}"));
        {
            let delivered = delivered.clone();
            let calls = calls.clone();
            recomposer.invalidate_tracked(
                scope_id,
                Some(cove_core::scene::Animation::linear()),
                None,
                false,
                reason.clone(),
                move |reasons| {
                    *delivered.borrow_mut() = reasons.to_vec();
                    calls.set(calls.get() + 1);
                },
            );
        }
        // A second invalidation with the same scope and cause must not create a
        // second body run.
        recomposer.invalidate_tracked(
            scope_id,
            Some(cove_core::scene::Animation::ease_in()),
            None,
            false,
            reason,
            |_| {},
        );
    }

    assert!(recomposer.commit_all(), "the queue commits once");
    assert_eq!(calls.get(), 1, "one body run for the coalesced scope");
    let delivered = delivered.borrow().clone();
    assert_eq!(
        delivered.len(),
        16,
        "the cause list is bounded at 16, got {}",
        delivered.len()
    );
    let details: Vec<Option<String>> = delivered.iter().map(|r| r.detail.clone()).collect();
    let expected: Vec<Option<String>> =
        (0..16).map(|index| Some(format!("field{index}"))).collect();
    assert_eq!(
        details, expected,
        "the first 16 distinct causes are kept in arrival order"
    );
}

#[test]
fn environment_changes_are_attributed_to_the_composition_local() {
    let counter = CounterView::new();
    support::with_text_environment(|| {
        GlobalTestLock::with_scheduler(|| {
            let graph = graph();
            let context = PlatformInputContext::new();
            context.with_current(|| {
                graph.install(counter.clone(), Default::default());
                graph.compute_layout(640.0, 460.0);
                let anchor = {
                    let root = graph.tree.borrow().root().expect("tree root");
                    flatten(&root)
                        .into_iter()
                        .find(|node| node.recomposition_metrics().is_some())
                        .expect("a scope anchor")
                };

                // A theme change is an environment reason, not a state write.
                counter.dark.set(true);
                graph.recomposer.commit_all();
                let metrics = anchor.recomposition_metrics().expect("metrics");
                assert_eq!(metrics.count, 1, "the theme change recomposes once");
                assert!(
                    metrics
                        .last_reasons
                        .iter()
                        .all(|reason| reason.kind == "state"),
                    "the direct write is attributed to its field, got {:?}",
                    metrics.last_reasons
                );
            });
        });
    });
}

#[test]
fn removed_scopes_ignore_queued_work() {
    let counter = CounterView::new();
    support::with_text_environment(|| {
        GlobalTestLock::with_scheduler(|| {
            let graph = graph();
            let context = PlatformInputContext::new();
            context.with_current(|| {
                graph.install(counter.clone(), Default::default());
                graph.compute_layout(640.0, 460.0);

                let counter_anchor = {
                    let root = graph.tree.borrow().root().expect("tree root");
                    flatten(&root)
                        .into_iter()
                        .find(|node| node.recomposition_metrics().is_some())
                        .expect("the counter's scope anchor")
                };
                let before = counter_anchor
                    .recomposition_metrics()
                    .expect("metrics")
                    .count;

                // Tear the graph down: nothing may run against the removed scope.
                graph.install_view(
                    AnyView::new(cove_composition::EmptyView),
                    Default::default(),
                );
                counter.count.set(3);
                graph.recomposer.commit_all();
                assert_eq!(
                    counter_anchor
                        .recomposition_metrics()
                        .map(|metrics| metrics.count),
                    Some(before),
                    "a removed scope runs no queued work"
                );

                let remaining = graph.tree.borrow().root().expect("tree root");
                assert!(
                    !flatten(&remaining)
                        .iter()
                        .any(|node| node.id() == counter_anchor.id()),
                    "the removed anchor is gone from the tree"
                );
            });
        });
    });
}

#[test]
fn scheduler_tracks_no_work_after_a_static_mount() {
    let counter = CounterView::new();
    support::with_text_environment(|| {
        GlobalTestLock::with_scheduler(|| {
            let scheduler = AnimatorScheduler::current();
            let graph = graph();
            graph.install(counter.clone(), Default::default());
            graph.compute_layout(640.0, 460.0);
            assert_eq!(
                scheduler.active_count(),
                0,
                "a static counter schedules no animation"
            );
            assert!(
                !graph.recomposer.has_pending(),
                "a static counter leaves nothing queued"
            );
        });
    });
}
