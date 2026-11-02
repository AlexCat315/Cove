//! Translation of `Tests/CoveUIComposeTests/ChromeComponentTests.swift`.
//!
//! Behaviour checklist:
//! - a Link configured as external announces the Link accessibility role
//! - a StatusBar with a divider measures `HEIGHT + 1` (the hairline belongs to
//!   the component); one without the divider measures `HEIGHT`
//! - the height is identical whether the bar carries one slot or several

use crate::support;

use std::cell::Cell;

use cove_core::scene::{AccessibilityRole, Node};
use cove_ui_compose::primitives::controls::Link;
use cove_ui_compose::primitives::feedback::StatusTone;
use cove_ui_compose::primitives::layout::status_bar::{StatusBar, StatusBarMetrics, StatusBarText};
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::{AnyView, TupleView};

use support::{GlobalTestLock, with_test_text_environment};

fn collect(root: &Node, into: &mut Vec<Node>) {
    into.push(root.clone());
    for child in root.children() {
        collect(&child, into);
    }
}

fn build_and_collect(
    graph: &std::rc::Rc<cove_composition::ViewGraph>,
    view: AnyView,
    width: f32,
    height: f32,
) -> Vec<Node> {
    graph.install_view(view, Default::default());
    graph.compute_layout(width, height);
    let root = graph.tree.borrow().root().expect("root node");
    let mut nodes = Vec::new();
    collect(&root, &mut nodes);
    nodes
}

#[test]
fn link_announces_itself_as_link_and_keeps_own_action() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let opened = Cell::new(0);
            let link = Link::title("Documentation", move || opened.set(opened.get() + 1))
                .configure(|this| this.is_external = true);

            let graph = support::graph();
            let nodes = build_and_collect(&graph, AnyView::new(link), 240.0, 40.0);
            let has_link = nodes.iter().any(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Link)
            });
            assert!(has_link, "the external link announces the Link role");
        });
    });
}

#[test]
fn status_bar_holds_one_height_with_one_or_two_slots() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            let leading = AnyView::new(TupleView::new(vec![
                AnyView::new(
                    StatusBarText::new("main").icon(ui_icon_catalog::editor::git_branch()),
                ),
                AnyView::new(StatusBarText::new("UTF-8").monospaced(true)),
            ]));
            let trailing =
                AnyView::new(StatusBarText::new("Ln 12").detail("Col 4").monospaced(true));
            let full = StatusBar::new(leading, trailing);
            let nodes = build_and_collect(&graph, AnyView::new(full), 320.0, 60.0);
            let full_node = nodes
                .iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Status bar")
                })
                .expect("the status bar node");
            assert_eq!(
                full_node.frame().height,
                StatusBarMetrics::HEIGHT + 1.0,
                "the hairline above the bar adds one point"
            );

            let graph = support::graph();
            let single = StatusBar::leading_only(AnyView::new(
                StatusBarText::new("Indexing").tone(StatusTone::Info),
            ))
            .shows_divider(false);
            let nodes = build_and_collect(&graph, AnyView::new(single), 320.0, 60.0);
            let single_node = nodes
                .iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Status bar")
                })
                .expect("the single-slot status bar node");
            assert_eq!(
                single_node.frame().height,
                StatusBarMetrics::HEIGHT,
                "without the divider the bar keeps its single height"
            );
        });
    });
}
