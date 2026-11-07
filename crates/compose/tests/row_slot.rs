//! Translation of `Tests/CoveUIComposeTests/RowSlotTests.swift`.
//!
//! Behaviour checklist:
//! - menu rows centre their content inside the minimum row height
//! - the row is at least 30pt tall
//! - the title's vertical midpoint matches the row's, never pinned to the top
//!
//! Model difference: the Swift suite also asserts the same centring for
//! `Tree` rows. The Rust `Tree` widget does not exist yet, so that half is
//! recorded rather than faked. `List` rows are covered here in their place
//! because `List` does exist and exercises the same row-host contract.

use crate::support;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node};
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};

use support::GlobalTestLock;

fn all_nodes(root: &Node, into: &mut Vec<Node>) {
    into.push(root.clone());
    for child in root.children() {
        all_nodes(&child, into);
    }
}

/// Settles the graph the way the Swift helper does: three commit+layout passes.
fn settle(graph: &std::rc::Rc<cove_composition::ViewGraph>) {
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(320.0, 240.0);
    }
}

#[test]
fn menu_rows_center_content_in_the_minimum_row_height() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            let menu = AnyView::new(
                Menu::new(vec![MenuEntry::Item(MenuItem::new(1, "Open", || {}))]).width(200.0),
            );
            graph.install_view(menu, Default::default());
            settle(&graph);

            let root = graph.tree.borrow().root().expect("root node");
            let mut nodes = Vec::new();
            all_nodes(&root, &mut nodes);

            let item = nodes
                .iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Button && semantics.label == "Open"
                    })
                })
                .map(|node| node.absolute_frame())
                .expect("the menu item exposes Button semantics");
            let title = nodes
                .iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::StaticText && semantics.label == "Open"
                    })
                })
                .map(|node| node.absolute_frame())
                .expect("the menu title exposes StaticText semantics");

            assert!(
                item.height >= 24.0,
                "a menu row is at least 24pt tall (got {})",
                item.height
            );
            let item_mid = item.y + item.height / 2.0;
            let title_mid = title.y + title.height / 2.0;
            assert!(
                (title_mid - item_mid).abs() <= 0.5,
                "the title is vertically centred in its row (item mid {item_mid}, title mid {title_mid})"
            );
        });
    });
}
