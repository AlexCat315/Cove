//! Translation of `Tests/CoveUIComposeTests/KeyedReconcilerTests.swift`.
//!
//! Behaviour checklist:
//! - `.id(_:)` stamps the key onto the produced node
//! - keyed children survive a reorder with their stamped state intact
//! - unkeyed siblings still match by sequential position, updating in place
//! - removed keyed siblings are torn down; the remaining ones survive
//! - adding a new keyed sibling materialises a fresh node in the right slot
//! - every `Node` gets a unique `ElementId`
//! - keys under ordinary modifiers survive updates and reorder
//! - the outermost key wins on materialisation and repeated updates
//! - scope and animation anchors keep child keys local
//!
//! Model difference: Swift's `ViewGraph.slotKey(_:)` is `@testable`-visible and
//! the suite calls it directly. In Rust `slot_key` is `pub(crate)`
//! (`view_graph.rs:792`), so it is not reachable from an integration test. The
//! equivalent claim is checked observationally instead: the outer node carries
//! no key while exactly one descendant does, which is what "the anchor keeps the
//! child's key local" means.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View, ViewExt, ViewGraph};
use cove_core::scene::{LayoutNode, Node, NodeKey, NodeTree, Recomposer};

use support::GlobalTestLock;

const INITIAL: &str = "_initial";
const LATEST: &str = "_latest";

/// Swift's `_TaggedNode`: stamps the current payload, but leaves any existing
/// `_initial` alone so the test can tell reuse from a fresh node.
struct TaggedNode {
    payload: String,
}

impl PrimitiveView for TaggedNode {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        if node.attachment::<String>(INITIAL).is_none() {
            node.set_attachment(INITIAL, self.payload.clone());
        }
        node.set_attachment(LATEST, self.payload.clone());
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
}

impl View for TaggedNode {
    fn kind(&self) -> cove_composition::ViewKind<'_> {
        cove_composition::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn tagged(payload: &str) -> AnyView {
    AnyView::new(TaggedNode {
        payload: payload.to_string(),
    })
}

fn tagged_keyed(payload: &str, key: &'static str) -> AnyView {
    AnyView::new(
        TaggedNode {
            payload: payload.to_string(),
        }
        .id(key),
    )
}

fn graph() -> (Rc<ViewGraph>, Rc<RefCell<NodeTree>>) {
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    let graph = Rc::new(ViewGraph::new(tree.clone(), Rc::new(Recomposer::new())));
    (graph, tree)
}

fn initial(payloads: &[(&str, &'static str)]) -> AnyView {
    AnyView::new(cove_composition::TupleView::new(
        payloads
            .iter()
            .map(|(payload, key)| tagged_keyed(payload, key))
            .collect(),
    ))
}

/// A `TupleView` flattens straight into the graph root, so the root itself is
/// the parent the reconciler operates on.
fn anchor(tree: &Rc<RefCell<NodeTree>>) -> Node {
    tree.borrow().root().expect("the graph has a root")
}

#[test]
fn id_stamps_the_key_onto_the_produced_node() {
    GlobalTestLock::locked(|| {
        let (graph, tree) = graph();
        graph.install(
            TaggedNode {
                payload: "a".to_string(),
            }
            .id("first"),
            Default::default(),
        );
        let node = tree
            .borrow()
            .root()
            .expect("root")
            .children()
            .into_iter()
            .next()
            .expect("the content node");
        assert!(
            node.key() == Some(NodeKey::new("first")),
            "the key is stamped onto the node"
        );
        assert!(
            !node.view_tag().unwrap_or_default().is_empty(),
            "the primitive's view tag survives"
        );
    });
}

#[test]
fn keyed_children_survive_reorder_with_state_intact() {
    GlobalTestLock::locked(|| {
        let (graph, tree) = graph();
        graph.install(
            initial(&[("A", "a"), ("B", "b"), ("C", "c")]),
            Default::default(),
        );
        let parent = anchor(&tree);
        let children = parent.children();
        let (original_a, original_b, original_c) =
            (children[0].clone(), children[1].clone(), children[2].clone());

        graph.reconcile_children(
            &parent,
            Some(&graph.layout_root()),
            &[initial(&[("C", "c"), ("A", "a"), ("B", "b")])],
        );

        let reordered = parent.children();
        assert_eq!(reordered.len(), 3, "all three children survive");
        assert_eq!(reordered[0].id(), original_c.id(), "C moves to the front");
        assert_eq!(reordered[1].id(), original_a.id(), "A moves to the middle");
        assert_eq!(reordered[2].id(), original_b.id(), "B moves to the back");

        for (node, expected) in [
            (&original_a, "A"),
            (&original_b, "B"),
            (&original_c, "C"),
        ] {
            assert_eq!(
                node.attachment::<String>(INITIAL).as_deref(),
                Some(expected),
                "the state stamped on first materialise survives the reorder"
            );
        }
    });
}

#[test]
fn unkeyed_siblings_still_match_by_sequential_position() {
    GlobalTestLock::locked(|| {
        let (graph, tree) = graph();
        graph.install(
            AnyView::new(cove_composition::TupleView::new(vec![
                tagged("A"),
                tagged("B"),
            ])),
            Default::default(),
        );
        let parent = anchor(&tree);
        let children = parent.children();
        let (original_a, original_b) = (children[0].clone(), children[1].clone());

        graph.reconcile_children(
            &parent,
            Some(&graph.layout_root()),
            &[AnyView::new(cove_composition::TupleView::new(vec![
                tagged("A2"),
                tagged("B2"),
            ]))],
        );

        let updated = parent.children();
        assert_eq!(updated[0].id(), original_a.id(), "the first sibling is reused");
        assert_eq!(updated[1].id(), original_b.id(), "the second is reused too");
        assert_eq!(
            original_a.attachment::<String>(LATEST).as_deref(),
            Some("A2"),
            "the reused node takes the new payload"
        );
        assert_eq!(
            original_b.attachment::<String>(LATEST).as_deref(),
            Some("B2"),
            "and so does the second"
        );
    });
}

#[test]
fn removed_keyed_siblings_are_torn_down() {
    GlobalTestLock::locked(|| {
        let (graph, tree) = graph();
        graph.install(
            initial(&[("A", "a"), ("B", "b"), ("C", "c")]),
            Default::default(),
        );
        let parent = anchor(&tree);
        let children = parent.children();
        let (original_a, original_c) = (children[0].clone(), children[2].clone());

        graph.reconcile_children(
            &parent,
            Some(&graph.layout_root()),
            &[initial(&[("A", "a"), ("C", "c")])],
        );

        let trimmed = parent.children();
        assert_eq!(trimmed.len(), 2, "the removed sibling is gone");
        assert_eq!(trimmed[0].id(), original_a.id(), "A survives");
        assert_eq!(trimmed[1].id(), original_c.id(), "C survives");
    });
}

#[test]
fn inserting_a_new_keyed_sibling_materialises_a_fresh_node() {
    GlobalTestLock::locked(|| {
        let (graph, tree) = graph();
        graph.install(
            initial(&[("A", "a"), ("B", "b")]),
            Default::default(),
        );
        let parent = anchor(&tree);
        let children = parent.children();
        let (original_a, original_b) = (children[0].clone(), children[1].clone());

        graph.reconcile_children(
            &parent,
            Some(&graph.layout_root()),
            &[initial(&[("A", "a"), ("X", "x"), ("B", "b")])],
        );

        let inserted = parent.children();
        assert_eq!(inserted.len(), 3, "the new sibling is added");
        assert_eq!(inserted[0].id(), original_a.id(), "A stays put");
        assert_ne!(
            inserted[1].id(),
            original_a.id(),
            "the new sibling is not A reused"
        );
        assert_ne!(
            inserted[1].id(),
            original_b.id(),
            "the new sibling is not B reused"
        );
        assert!(
            inserted[1].key() == Some(NodeKey::new("x")),
            "the new sibling carries its own key"
        );
        assert_eq!(inserted[2].id(), original_b.id(), "B stays put");
    });
}

#[test]
fn element_ids_are_unique_per_node() {
    GlobalTestLock::locked(|| {
        let a = Node::new();
        let b = Node::new();
        assert_ne!(a.id(), b.id(), "each node gets its own ElementId");
    });
}

#[test]
fn keys_under_ordinary_modifiers_survive_updates_and_reorder() {
    GlobalTestLock::locked(|| {
        use cove_ui_compose::modifiers::layout::LayoutFrameExt;

        let build = |payload: &str, key: &'static str| {
            AnyView::new(
                AnyView::new(TaggedNode {
                    payload: payload.to_string(),
                })
                .id(key)
                .frame_height(30.0),
            )
        };

        let (graph, tree) = graph();
        graph.install(build("A", "a"), Default::default());
        let parent = tree.borrow().root().expect("root").clone();
        let original_a = parent.children()[0].clone();
        let original_before = original_a.attachment::<String>(INITIAL);

        let a = build("A2", "a");
        let b = build("B", "b");
        graph.reconcile_children(&parent, Some(&graph.layout_root()), &[a, b]);

        let first_b = parent.children()[1].clone();
        graph.reconcile_children(
            &parent,
            Some(&graph.layout_root()),
            &[build("B", "b"), build("A2", "a")],
        );
        graph.reconcile_children(
            &parent,
            Some(&graph.layout_root()),
            &[build("B", "b"), build("A2", "a")],
        );

        let final_children = parent.children();
        assert_eq!(
            final_children[0].id(),
            first_b.id(),
            "the reordered keyed child keeps its node"
        );
        assert_eq!(
            final_children[1].id(),
            original_a.id(),
            "its sibling keeps its node too"
        );
        assert_eq!(
            original_a.attachment::<String>(INITIAL),
            original_before,
            "the first-stamped state survives repeated reconciliation"
        );
        assert_eq!(
            original_a.attachment::<String>(LATEST).as_deref(),
            Some("A2"),
            "the latest payload is applied"
        );
    });
}

#[test]
fn the_outermost_key_wins_on_materialisation_and_updates() {
    GlobalTestLock::locked(|| {
        use cove_ui_compose::modifiers::layout::LayoutFrameExt;

        let build = || {
            AnyView::new(
                AnyView::new(
                    AnyView::new(TaggedNode {
                        payload: "A".to_string(),
                    })
                    .id("inner")
                    .frame_height(30.0),
                )
                .id("outer"),
            )
        };

        let (graph, tree) = graph();
        graph.install(build(), Default::default());
        let parent = tree.borrow().root().expect("root").clone();
        let original = parent.children()[0].clone();

        for _ in 0..3 {
            graph.reconcile_children(&parent, Some(&graph.layout_root()), &[build()]);
            assert_eq!(
                parent.children()[0].id(),
                original.id(),
                "the node survives repeated reconciliation"
            );
            assert!(
            original.key() == Some(NodeKey::new("outer")),
            "the outermost key is the one that sticks"
        );
        }
    });
}

#[test]
fn scope_and_animation_anchors_keep_child_keys_local() {
    use cove_ui_compose::modifiers::animation_value::AnimationValueExt;
    use cove_ui_compose::modifiers::layout::LayoutFrameExt;
    use cove_ui_compose::theme::{Theme, ThemeViewExt};

    fn keyed_descendants(node: &Node, into: &mut Vec<Node>) {
        if node.key() == Some(NodeKey::new("child")) {
            into.push(node.clone());
        }
        for child in node.children() {
            keyed_descendants(&child, into);
        }
    }

    GlobalTestLock::locked(|| {
        for scoped in [true, false] {
            let build = || {
                let base = AnyView::new(
                    AnyView::new(TaggedNode {
                        payload: "A".to_string(),
                    })
                    .id("child"),
                );
                if scoped {
                    AnyView::new(AnyView::new(base).theme(Theme::default_dark()).frame_height(30.0))
                } else {
                    AnyView::new(
                        AnyView::new(base)
                            .animation_value(None, 0)
                            .frame_height(30.0),
                    )
                }
            };

            let (graph, tree) = graph();
            graph.install(build(), Default::default());
            let parent = tree.borrow().root().expect("root").clone();
            let original = parent.children()[0].clone();

            assert!(
                original.key().is_none(),
                "the outer anchor carries no key of its own"
            );
            let mut keyed = Vec::new();
            keyed_descendants(&original, &mut keyed);
            assert_eq!(
                keyed.len(),
                1,
                "exactly one descendant carries the child key"
            );

            graph.reconcile_children(&parent, Some(&graph.layout_root()), &[build()]);
            assert_eq!(
                parent.children()[0].id(),
                original.id(),
                "the anchor survives reconciliation"
            );
            let mut after = Vec::new();
            keyed_descendants(&original, &mut after);
            assert_eq!(
                after.first().map(|node| node.id()),
                keyed.first().map(|node| node.id()),
                "the keyed descendant survives too"
            );
        }
    });
}
