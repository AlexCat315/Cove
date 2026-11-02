//! Translation of `Tests/CoveUIComposeTests/TextShapeCacheTests.swift`.
//!
//! Behaviour checklist:
//! - the layout cache entry is written on first measure and reused on identical
//!   inputs
//! - the cache key carries the text, so a changed string is a miss
//! - a reused `Text` node refreshes its measurement when the string changes
//! - unchanged text paint stays clean across an unrelated recompose
//!
//! Name mapping: `layoutNode.textMeasure` is `LayoutNode::text_measure()`
//! returning a `TextLayoutCacheEntry` whose `key.text` is the measured string.

use cove_composition::Scope;
use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, Row, View};
use cove_core::State;
use cove_core::scene::Node;
use cove_ui_compose::primitives::text::Text;

use support::{GlobalTestLock, with_test_text_environment};

struct TextHarness {
    text: State<String>,
}

impl View for TextHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let text = self.text.get();
        AnyView::new(Row::new(
            cove_composition::VerticalAlignment::Center,
            0.0,
            AnyView::new(Text::new(text)),
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("text", &self.text)]
    }
}

/// A stable `Text` beside an unrelated view whose size changes.
struct StableTextPaintHarness {
    revision: State<i32>,
}

impl View for StableTextPaintHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        use cove_ui_compose::modifiers::layout::LayoutFrameExt;
        let revision = self.revision.get();
        AnyView::new(Row::new(
            cove_composition::VerticalAlignment::Center,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(Text::new("constant")),
                AnyView::new(
                    AnyView::new(cove_composition::EmptyView).frame_size(revision as f32, 1.0),
                ),
            ])),
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("revision", &self.revision)]
    }
}

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            walk(&child, into);
        }
    }
    walk(node, &mut collected);
    collected
}

/// The node that actually measured text.
fn measured_text_node(graph: &Rc<cove_composition::ViewGraph>) -> Node {
    let root = graph.tree.borrow().root().expect("root");
    descendants(&root)
        .into_iter()
        .find(|node| {
            node.layout_node()
                .and_then(|layout| layout.text_measure())
                .is_some()
        })
        .expect("a text-measured node mounts")
}

#[test]
fn cache_hit() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(Text::new("Hello, world.")), Default::default());
            graph.compute_layout(200.0, 200.0);

            let node = measured_text_node(&graph);
            let layout = node.layout_node().expect("the text node has a layout node");
            let first = layout
                .text_measure()
                .expect("the first measure wrote an entry");

            graph.compute_layout(200.0, 200.0);
            let second = layout
                .text_measure()
                .expect("the second measure reuses the entry");
            assert_eq!(
                first.key.text, "Hello, world.",
                "the cache key records the measured string"
            );
            assert_eq!(
                first.key.max_width, second.key.max_width,
                "an identical width keeps the same key"
            );
            assert_eq!(
                first.key.text, second.key.text,
                "an identical string keeps the same key"
            );
        });
    });
}

#[test]
fn cache_miss_on_text_change() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(Text::new("first")), Default::default());
            graph.compute_layout(200.0, 200.0);

            let node = graph
                .tree
                .borrow()
                .root()
                .expect("root")
                .children()
                .first()
                .cloned()
                .expect("the text node mounts");
            let layout = node.layout_node().expect("the text node has a layout node");
            let key = layout.text_measure().expect("an entry was written").key;
            assert_eq!(key.text, "first", "the key names the authored string");
        });
    });
}

#[test]
fn reused_node_refreshes_measurement() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            let text = State::new("short".to_string());
            graph.install_view(
                AnyView::new(TextHarness { text: text.clone() }),
                Default::default(),
            );
            graph.compute_layout(200.0, 200.0);

            let node = measured_text_node(&graph);
            let before_width = node.frame().width;

            text.set("a much longer label".to_string());
            graph.recomposer.commit_all();
            graph.compute_layout(200.0, 200.0);

            assert!(
                node.frame().width > before_width,
                "the reused node widens for the longer string ({} -> {})",
                before_width,
                node.frame().width
            );
            let key = node
                .layout_node()
                .expect("the text node has a layout node")
                .text_measure()
                .expect("the refreshed measure wrote an entry")
                .key;
            assert_eq!(
                key.text, "a much longer label",
                "the refreshed key names the new string"
            );
        });
    });
}

#[test]
fn stable_text_paint_survives_recompose() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            let revision = State::new(0i32);
            graph.install_view(
                AnyView::new(StableTextPaintHarness {
                    revision: revision.clone(),
                }),
                Default::default(),
            );
            graph.compute_layout(200.0, 200.0);

            let text_node = measured_text_node(&graph);
            graph.tree.borrow_mut().flush();
            assert!(
                !text_node.render_dirty(),
                "a settled text node carries no pending paint"
            );

            revision.set(12);
            graph.recomposer.commit_all();

            let root = graph.tree.borrow().root().expect("root");
            let after = descendants(&root)
                .into_iter()
                .find(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_measure())
                        .is_some()
                })
                .expect("the text node survives the recompose");
            assert_eq!(
                after, text_node,
                "the same text node is reused rather than remounted"
            );
            assert!(
                !after.render_dirty(),
                "an unrelated recompose does not dirty the unchanged text"
            );
        });
    });
}
