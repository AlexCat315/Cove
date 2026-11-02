//! Translation of `Tests/CoveUIComposeTests/SidebarTests.swift`.
//!
//! Behaviour checklist:
//! - a destination highlight is inset the same on both sides, by the sidebar's
//!   own 10pt content padding
//! - rows reach the trailing edge of the pane, not of a scrollbar lane
//! - a destination rings for the keyboard (2pt expanded, 1pt rail) and stays
//!   quiet for the pointer, in every presentation
//!
//! Named difference: Swift located a row by the debug name
//! `sidebar-item-<id>`. The Rust `Sidebar` publishes no such debug name, so rows
//! are identified by their accessible label — the same node Swift found, reached
//! structurally instead of by attachment.
//!
//! Production gaps (reported, not fixed):
//!
//! - `SidebarItemStyle::make_body` builds a `SidebarChromeRow` with a fill and an
//!   ink colour but no border, so a destination never rings for keyboard focus.
//!   Swift's `Destination`/`RailDestination` draw a 2pt (expanded) / 1pt (rail)
//!   `focusRing` border. The three ring cases pin that with `GAP:` asserts.

use crate::support;

use std::rc::Rc;

use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, FocusChain, FocusChainHolder, Node};
use cove_ui_compose::primitives::layout::sidebar::{
    Sidebar, SidebarItem, SidebarPresentation, SidebarSection,
};

use support::GlobalTestLock;

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            walk(&child, into);
        }
    }
    walk(root, &mut collected);
    collected
}

/// The destination row showing `title`.
fn row_for(graph: &Rc<cove_composition::ViewGraph>, title: &str) -> Node {
    let root = graph.tree.borrow().root().expect("root");
    nodes(&root)
        .into_iter()
        .find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.label == title && semantics.role == AccessibilityRole::Button
            })
        })
        .unwrap_or_else(|| panic!("a row titled {title} is mounted"))
}

fn mounted(
    presentation: SidebarPresentation,
    test: impl FnOnce(&Rc<cove_composition::ViewGraph>, &Rc<FocusChain>),
) {
    GlobalTestLock::locked(|| {
        let chain = Rc::new(FocusChain::new());
        FocusChainHolder::set_current(Some(chain.clone()));
        let graph = support::graph();
        let mut sidebar = Sidebar::new(
            Binding::constant(Some("button")),
            vec![
                SidebarSection::new(
                    "controls",
                    vec![
                        SidebarItem::new("button", "Button"),
                        SidebarItem::new("checkbox", "Checkbox"),
                    ],
                )
                .title("Controls"),
            ],
        );
        sidebar.presentation = presentation;
        graph.install_view(cove_composition::AnyView::new(sidebar), Default::default());
        graph.compute_layout(226.0, 400.0);

        test(&graph, &chain);

        graph.install_view(
            cove_composition::AnyView::new(cove_composition::EmptyView),
            Default::default(),
        );
        FocusChainHolder::set_current(None);
    });
}

fn rings(graph: &Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    let root = graph.tree.borrow().root().expect("root");
    nodes(&root)
        .into_iter()
        .filter(|node| node.border_width() > 0.0)
        .collect()
}

#[test]
fn highlight_is_symmetric() {
    mounted(SidebarPresentation::Expanded, |graph, _| {
        let root = graph.tree.borrow().root().expect("root");
        let sidebar = root.absolute_frame();
        let item = row_for(graph, "Button").absolute_frame();
        let leading = item.x - sidebar.x;
        let trailing = sidebar.max_x() - item.max_x();
        assert!(
            (leading - trailing).abs() < 0.5,
            "the row is inset evenly (leading {leading}, trailing {trailing})"
        );
        assert!((leading - 10.0).abs() < 0.5, "leading inset {leading}");
    });
}

#[test]
fn rows_fill_the_pane() {
    mounted(SidebarPresentation::Expanded, |graph, _| {
        let root = graph.tree.borrow().root().expect("root");
        let sidebar = root.absolute_frame();
        let item = row_for(graph, "Checkbox").absolute_frame();
        let target = sidebar.max_x() - 10.0;
        assert!(
            (target - item.max_x()).abs() < 0.5,
            "the row reaches the pane's inner edge ({target}), got {}",
            item.max_x()
        );
    });
}

#[test]
fn ring_follows_the_keyboard_expanded() {
    mounted(SidebarPresentation::Expanded, |graph, chain| {
        let item = row_for(graph, "Button");

        chain.focus(Some(&item), false);
        graph.recomposer.commit_all();
        graph.compute_layout(226.0, 400.0);
        assert!(rings(graph).is_empty(), "pointer focus leaves no ring");

        chain.focus(Some(&item), true);
        graph.recomposer.commit_all();
        graph.compute_layout(226.0, 400.0);
        // GAP: Swift asserts a 2pt focus ring here. `SidebarItemStyle` never
        // sets a border, so no node rings.
        assert!(
            !rings(graph).iter().any(|node| node.border_width() == 2.0),
            "GAP: an expanded destination draws no 2pt keyboard ring"
        );

        chain.focus(Some(&item), false);
        graph.recomposer.commit_all();
        graph.compute_layout(226.0, 400.0);
        assert!(rings(graph).is_empty(), "pointer focus clears the ring");
        cove_core::scene::AnimatorScheduler::current().tick(1.0);
        assert!(
            rings(graph).is_empty(),
            "ticking does not resurrect the ring"
        );
    });
}

#[test]
fn ring_follows_the_keyboard_rail_with_labels() {
    mounted(
        SidebarPresentation::Rail { shows_labels: true },
        |graph, chain| {
            let item = row_for(graph, "Button");

            chain.focus(Some(&item), false);
            graph.recomposer.commit_all();
            graph.compute_layout(226.0, 400.0);
            assert!(rings(graph).is_empty(), "pointer focus leaves no ring");

            chain.focus(Some(&item), true);
            graph.recomposer.commit_all();
            graph.compute_layout(226.0, 400.0);
            // GAP: Swift asserts a 1pt focus ring here; the Rust row never sets
            // a border, so no node rings.
            assert!(
                !rings(graph).iter().any(|node| node.border_width() == 1.0),
                "GAP: a rail destination draws no 1pt keyboard ring"
            );

            chain.focus(Some(&item), false);
            graph.recomposer.commit_all();
            graph.compute_layout(226.0, 400.0);
            assert!(rings(graph).is_empty(), "pointer focus clears the ring");
        },
    );
}

#[test]
fn ring_follows_the_keyboard_rail_without_labels() {
    mounted(
        SidebarPresentation::Rail {
            shows_labels: false,
        },
        |graph, chain| {
            let item = row_for(graph, "Button");

            chain.focus(Some(&item), false);
            graph.recomposer.commit_all();
            graph.compute_layout(226.0, 400.0);
            assert!(rings(graph).is_empty(), "pointer focus leaves no ring");

            chain.focus(Some(&item), true);
            graph.recomposer.commit_all();
            graph.compute_layout(226.0, 400.0);
            // GAP: Swift asserts a 1pt focus ring here; the Rust row never sets
            // a border, so no node rings.
            assert!(
                !rings(graph).iter().any(|node| node.border_width() == 1.0),
                "GAP: a label-less rail destination draws no 1pt keyboard ring"
            );

            chain.focus(Some(&item), false);
            graph.recomposer.commit_all();
            graph.compute_layout(226.0, 400.0);
            assert!(rings(graph).is_empty(), "pointer focus clears the ring");
            cove_core::scene::AnimatorScheduler::current().tick(1.0);
            assert!(
                rings(graph).is_empty(),
                "ticking does not resurrect the ring"
            );
        },
    );
}

#[test]
fn leaving_the_rail_restores_the_pane_width() {
    use std::cell::Cell;
    struct Pane(Rc<Cell<SidebarPresentation>>);
    impl cove_composition::View for Pane {
        fn body(&self, _cx: &cove_composition::Scope) -> cove_composition::AnyView {
            let mut sidebar = Sidebar::new(
                Binding::constant(Some("button")),
                vec![SidebarSection::new(
                    "controls",
                    vec![SidebarItem::new("button", "Button")],
                )],
            );
            sidebar.presentation = self.0.get();
            cove_composition::AnyView::new(sidebar)
        }
    }
    GlobalTestLock::locked(|| {
        let presentation = Rc::new(Cell::new(SidebarPresentation::Rail {
            shows_labels: false,
        }));
        let graph = support::graph();
        graph.install_view(
            cove_composition::AnyView::new(Pane(presentation.clone())),
            Default::default(),
        );
        graph.compute_layout(226.0, 400.0);
        assert_eq!(row_for(&graph, "Button").absolute_frame().width, 44.0);

        presentation.set(SidebarPresentation::Expanded);
        graph.install_view(
            cove_composition::AnyView::new(Pane(presentation.clone())),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(226.0, 400.0);
        assert_eq!(
            row_for(&graph, "Button").absolute_frame().width,
            206.0,
            "the row fills the expanded pane"
        );
        graph.install_view(
            cove_composition::AnyView::new(cove_composition::EmptyView),
            Default::default(),
        );
    });
}
