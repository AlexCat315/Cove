//! Translation of `Tests/CoveUIComposeTests/ViewGraphTests.swift`.
//!
//! Behaviour checklist:
//! - installing a single primitive creates one child node
//! - a structural view expands into sibling nodes
//! - a modifier mutates the materialised node
//! - multiple modifiers stack in declaration order
//! - a reorder across a transparent anchor repositions the stable leaf
//! - a state write recomposes on the next commit, and several writes collapse
//! - `compute_layout_if_needed` skips stable frames and reruns after a change
//! - a same-shape recompose reuses the node and its attachments
//! - a tag mismatch tears the node down
//! - a modifier re-applies to the reused node when its value changes

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, TupleView, View, ViewExt, ViewModifier};
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::{BoxView, Row, VerticalAlignment};

use support::{DebugNode, GlobalTestLock};

/// Mirrors the Swift `_DebugNode`, which stamps its label length into `x`.
struct Labelled(usize);

impl cove_composition::PrimitiveView for Labelled {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        let mut frame = node.frame();
        frame.x = self.0 as f32;
        node.set_frame(frame);
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}

impl View for Labelled {
    fn kind(&self) -> cove_composition::ViewKind<'_> {
        cove_composition::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn labelled(text: &str) -> AnyView {
    AnyView::new(Labelled(text.chars().count()))
}

/// Stamps a marker into `frame.y`, the Swift `MarkerModifier`.
struct MarkerModifier(f32);

impl ViewModifier for MarkerModifier {
    fn apply_node(&self, node: &Node) {
        let mut frame = node.frame();
        frame.y = self.0;
        node.set_frame(frame);
    }
}

/// Flips hit testing off, the Swift `DisableHitTestModifier`.
struct DisableHitTestModifier;

impl ViewModifier for DisableHitTestModifier {
    fn apply_node(&self, node: &Node) {
        node.set_is_hit_testable(false);
    }
}

/// A view whose `body` reads `state`, and which declares that read so a write
/// invalidates the graph — the Rust counterpart of a Swift `@State` property.
struct StateHarness {
    state: State<usize>,
}

impl View for StateHarness {
    fn body(&self) -> AnyView {
        AnyView::new(Labelled(self.state.get()))
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("value", &self.state)]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// A primitive that counts how many times it updated one node.
struct CountingNode(Rc<Cell<i32>>, i32);

impl cove_composition::PrimitiveView for CountingNode {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        let mut frame = node.frame();
        frame.x = self.1 as f32;
        node.set_frame(frame);
        let prior: i32 = node.attachment::<i32>("__count").unwrap_or(0);
        node.set_attachment("__count", prior + 1);
        self.0.set(self.0.get() + 1);
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}

impl View for CountingNode {
    fn kind(&self) -> cove_composition::ViewKind<'_> {
        cove_composition::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// The materialised children of the graph root.
///
/// Model difference: the Swift image nests a transparent anchor between the
/// root and the view's own nodes (`root -> anchor -> children`). The Rust graph
/// materialises the view's nodes directly under the root, so these helpers read
/// `root.children()` rather than descending through an anchor.
fn children(graph: &Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    graph
        .tree
        .borrow()
        .root()
        .map(|root| root.children())
        .unwrap_or_default()
}

fn first_child(graph: &Rc<cove_composition::ViewGraph>) -> Node {
    children(graph)
        .into_iter()
        .next()
        .expect("a materialised child")
}

/// The view's own materialised node below the anchor a user view introduces.
///
/// A `View` with a `body()` gets a transparent anchor node between the root and
/// its content (the Rust equivalent of Swift's anchor), so state-driven suites
/// descend one level. Primitive/structural roots have no anchor.
fn body_child(graph: &Rc<cove_composition::ViewGraph>) -> Node {
    let anchor = first_child(graph);
    anchor.children().into_iter().next().unwrap_or(anchor)
}

fn root_node(graph: &Rc<cove_composition::ViewGraph>) -> Node {
    graph.tree.borrow().root().expect("root node")
}

#[test]
fn installing_a_single_primitive_creates_one_child() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(labelled("hi"), Default::default());
        assert_eq!(children(&graph).len(), 1, "root -> one debug node");
        assert_eq!(
            first_child(&graph).frame().x,
            2.0,
            "the label length is stamped into x"
        );
    });
}

#[test]
fn a_tuple_expands_into_siblings() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(TupleView::new(vec![
                labelled("a"),
                labelled("bb"),
                labelled("ccc"),
            ])),
            Default::default(),
        );
        let xs: Vec<f32> = children(&graph)
            .iter()
            .map(|child| child.frame().x)
            .collect();
        assert_eq!(xs, vec![1.0, 2.0, 3.0], "each element keeps its own length");
    });
}

#[test]
fn a_modifier_mutates_the_materialised_node() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(AnyView::new(Labelled(0)).modifier(MarkerModifier(42.0))),
            Default::default(),
        );
        assert_eq!(
            first_child(&graph).frame().y,
            42.0,
            "the modifier writes into the node the primitive created"
        );
    });
}

#[test]
fn modifiers_stack_in_declaration_order() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(Labelled(0))
                    .modifier(MarkerModifier(10.0))
                    .modifier(DisableHitTestModifier),
            ),
            Default::default(),
        );
        let node = first_child(&graph);
        assert_eq!(node.frame().y, 10.0, "the marker modifier ran");
        assert!(!node.is_hit_testable(), "the later modifier also ran");
    });
}

#[test]
fn a_reorder_across_a_transparent_anchor_repositions_the_stable_leaf() {
    GlobalTestLock::locked(|| {
        let show_leading = Rc::new(State::new(false));
        let first = show_leading.clone();
        let graph = support::graph();

        let leaf = |name: &str, width: f32| -> AnyView {
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(cove_composition::EmptyView),
                ))
                .frame_width(width)
                .frame_height(10.0)
                .debug_name(name),
            )
        };

        // A row whose leading leaf appears only after the state flips.
        let build = {
            let first = first.clone();
            move || {
                let mut children = Vec::new();
                if first.get() {
                    children.push(leaf("leading", 40.0));
                }
                children.push(leaf("stable", 60.0));
                AnyView::new(
                    AnyView::new(Row::new(
                        VerticalAlignment::Top,
                        0.0,
                        AnyView::new(TupleView::new(children)),
                    ))
                    .frame_width(200.0)
                    .frame_height(10.0),
                )
            }
        };

        graph.install_view(build(), Default::default());
        graph.compute_layout(200.0, 20.0);
        let stable = graph
            .layout_snapshot()
            .into_iter()
            .find(|entry| entry.debug_name.as_deref() == Some("stable"))
            .expect("the stable leaf");
        assert_eq!(
            stable.absolute_frame.x, 0.0,
            "only child starts at the origin"
        );

        show_leading.set(true);
        graph.recomposer.commit_all();
        graph.install_view(build(), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 20.0);

        let snapshot = graph.layout_snapshot();
        let leading = snapshot
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("leading"))
            .expect("the leading leaf appears");
        let stable = snapshot
            .iter()
            .find(|entry| entry.debug_name.as_deref() == Some("stable"))
            .expect("the stable leaf survives");
        assert_eq!(
            leading.absolute_frame.x, 0.0,
            "the new leaf takes the origin"
        );
        assert_eq!(
            stable.absolute_frame.x, 40.0,
            "the stable leaf shifts right by the leading leaf's width"
        );
    });
}

#[test]
fn a_state_write_recomposes_on_the_next_commit() {
    GlobalTestLock::locked(|| {
        let value = State::new(1usize);
        let graph = support::graph();
        graph.install(
            StateHarness {
                state: value.clone(),
            },
            Default::default(),
        );
        assert_eq!(body_child(&graph).frame().x, 1.0);

        value.set(3);
        assert!(
            graph.recomposer.has_pending(),
            "the declared state read schedules a recompose"
        );
        graph.recomposer.commit_all();
        assert_eq!(
            body_child(&graph).frame().x,
            3.0,
            "the recompose applied the new value"
        );
    });
}

#[test]
fn several_writes_before_one_commit_collapse() {
    GlobalTestLock::locked(|| {
        let value = State::new(0usize);
        let graph = support::graph();
        graph.install(
            StateHarness {
                state: value.clone(),
            },
            Default::default(),
        );

        value.set(1);
        value.set(2);
        value.set(5);
        graph.recomposer.commit_all();

        assert_eq!(
            body_child(&graph).frame().x,
            5.0,
            "only the final value is materialised"
        );
    });
}

#[test]
fn compute_layout_if_needed_gates_on_geometry_changes() {
    GlobalTestLock::locked(|| {
        let width = Rc::new(State::new(40.0f32));
        let graph = support::graph();
        let build = {
            let width = width.clone();
            move || {
                AnyView::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::EmptyView),
                    ))
                    .frame_width(width.get())
                    .frame_height(10.0),
                )
            }
        };
        graph.install_view(build(), Default::default());

        assert!(
            graph.compute_layout_if_needed(200.0, 200.0),
            "the first layout always runs"
        );
        assert!(
            !graph.compute_layout_if_needed(200.0, 200.0),
            "a stable frame is skipped"
        );

        width.set(80.0);
        graph.install_view(build(), Default::default());
        graph.recomposer.commit_all();

        assert!(
            graph.layout_needs_update(200.0, 200.0),
            "the width change invalidates layout"
        );
        assert!(
            graph.compute_layout_if_needed(200.0, 200.0),
            "the rerun happens after the change"
        );
        assert!(
            !graph.compute_layout_if_needed(200.0, 200.0),
            "and settles again"
        );
    });
}

#[test]
fn a_same_shape_recompose_reuses_the_node_and_its_attachments() {
    GlobalTestLock::locked(|| {
        // A recompose keeps the materialised node and its attachments; only
        // `update_node` runs again. Driven by a state write so the graph
        // reconciles rather than re-installing the root.
        let builds = Rc::new(Cell::new(0));
        let value = Rc::new(State::new(1i32));
        let counting = CountingNode(builds.clone(), 1);

        struct Harness {
            value: Rc<State<i32>>,
            counting: CountingNode,
        }
        impl View for Harness {
            fn body(&self) -> AnyView {
                AnyView::new(CountingNode(self.counting.0.clone(), self.value.get()))
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![cove_composition::DynamicField::state("value", &self.value)]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let graph = support::graph();
        graph.install(
            Harness {
                value: value.clone(),
                counting,
            },
            Default::default(),
        );

        let first = body_child(&graph);
        assert_eq!(first.attachment::<i32>("__count"), Some(1));
        first.set_attachment("__user", "preserved".to_string());

        value.set(7);
        graph.recomposer.commit_all();

        let after = body_child(&graph);
        assert_eq!(after.frame().x, 7.0, "the new value was applied");
        assert!(
            after.attachment::<i32>("__count").unwrap_or(0) >= 2,
            "the same node was updated, not rebuilt"
        );
        assert_eq!(
            after.attachment::<String>("__user").as_deref(),
            Some("preserved"),
            "user attachments survive a same-shape recompose"
        );
    });
}

#[test]
fn a_tag_mismatch_tears_down_the_old_node() {
    GlobalTestLock::locked(|| {
        let flag = Rc::new(State::new(false));

        struct Harness {
            flag: Rc<State<bool>>,
        }
        impl View for Harness {
            fn body(&self) -> AnyView {
                if self.flag.get() {
                    AnyView::new(CountingNode(Rc::new(Cell::new(0)), 100))
                } else {
                    labelled("abc")
                }
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![cove_composition::DynamicField::state("flag", &self.flag)]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let graph = support::graph();
        graph.install(Harness { flag: flag.clone() }, Default::default());
        assert_eq!(
            body_child(&graph).frame().x,
            3.0,
            "the initial branch is the labelled debug node"
        );

        flag.set(true);
        graph.recomposer.commit_all();

        let after = body_child(&graph);
        assert_eq!(after.frame().x, 100.0, "the counting node replaced it");
        assert_eq!(
            after.attachment::<i32>("__count"),
            Some(1),
            "a fresh node starts its own counter"
        );
    });
}

#[test]
fn a_modifier_reapplies_to_the_reused_node() {
    GlobalTestLock::locked(|| {
        struct Harness {
            y: Rc<State<f32>>,
        }
        impl View for Harness {
            fn body(&self) -> AnyView {
                AnyView::new(AnyView::new(Labelled(0)).modifier(MarkerModifier(self.y.get())))
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![cove_composition::DynamicField::state("y", &self.y)]
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }

        let y = Rc::new(State::new(5.0f32));
        let graph = support::graph();
        graph.install(Harness { y: y.clone() }, Default::default());
        assert_eq!(body_child(&graph).frame().y, 5.0);

        y.set(99.0);
        graph.recomposer.commit_all();

        assert_eq!(
            body_child(&graph).frame().y,
            99.0,
            "the reused node received the new modifier value"
        );
    });
}

#[test]
fn debug_node_from_support_keeps_its_label_tag() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(AnyView::new(DebugNode::new("tagged")), Default::default());
        let node = first_child(&graph);
        assert_eq!(
            node.view_tag().as_deref(),
            Some("tagged"),
            "the support primitive records its label as the view tag"
        );
    });
}
