//! Translation of `Tests/CoveUIComposeTests/TextMeasureSlotTests.swift`.
//!
//! Behaviour checklist:
//! - measuring a `Text` populates the layout node's typed text inputs
//! - `LayoutTree::reset_caches` clears the measure slot across the subtree

mod support;

use cove_composition::{AnyView, View, ViewExt};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

#[test]
fn text_inputs_are_populated_by_measure() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(Text::new("hello")), Default::default());
            graph.compute_layout(200.0, 200.0);

            let root = graph.tree.borrow().root().expect("root node");
            let text_node = root.children().first().cloned().expect("text node");
            let layout = graph.layout_node(&text_node).expect("layout node");
            let inputs = layout.text_inputs().expect("the measure slot is populated");
            assert_eq!(inputs.text, "hello");
        });
    });
}

#[test]
fn reset_caches_wipes_the_measure_slot() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(AnyView::new(Text::new("first")), Default::default());
            graph.compute_layout(200.0, 200.0);

            let root = graph.tree.borrow().root().expect("root node");
            let text_node = root.children().first().cloned().expect("text node");
            let layout = graph.layout_node(&text_node).expect("layout node");
            assert!(
                layout.text_measure().is_some(),
                "measuring records a text measure entry"
            );

            graph.layout_tree.reset_caches();
            assert!(
                layout.text_measure().is_none(),
                "reset_caches clears the measure slot"
            );
        });
    });
}
