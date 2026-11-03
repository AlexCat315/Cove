//! Translation of `Tests/CoveUIComposeTests/TabViewTests.swift`.
//!
//! Behaviour checklist:
//! - two tabs hosting the same panel type keep independent mounted state, and
//!   revisiting a tab remounts a fresh panel
//! - only the selected tab's content is mounted
//! - a different initial selection mounts the matching content
//! - a selection that matches no tab mounts no content
//!
//! Model differences (production gaps, reported):
//!
//! 1. **A selection that matches no tab falls back to the first enabled tab.**
//!    Swift's `contentArea` mounts only `tabs.first(where: { $0.id == selection
//!    && $0.isEnabled })`, so a miss renders nothing (`TabView.swift:127`).
//!    Rust's `TabsView::active` (`tab_view.rs:230-237`) returns
//!    `enabled.first()` when the selection is absent, so tab A mounts instead.
//!    `a_selection_matching_no_tab_mounts_the_first_enabled_tab` records this.
//! 2. ~~Tab strip items carry no `Tab` accessibility role.~~ Fixed: each strip
//!    item publishes `AccessibilityRole::Tab` with its own label, matching
//!    `TabView.swift:221`. `the_tab_strip_exposes_tab_roles` verifies it.
//! 3. **Two tabs hosting the same panel type share one panel state.** Swift's
//!    `SwiftObservationTests`-style assertion here is that each tab keeps its own
//!    `@State`, so a bumped first tab still leaves the second at `Count 0`.
//!    In Rust the tab content is re-derived from `RovingFocusHost`'s `children`
//!    (`tab_view.rs:377-395`) with no per-tab identity, so switching tabs reuses
//!    the same materialised panel node and its `State` survives the switch —
//!    measured: after bumping tab A to `Count 1` and switching to tab B, tab B
//!    also reads `Count 1`. Fixed: content is now identified per tab, so each tab

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, PrimitiveView, View, ViewGraph, ViewKind};
use cove_core::State;
use cove_core::scene::{AccessibilityRole, LayoutNode, Node, NodeTree, Recomposer};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::{TabItem, TabView};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

const PROBE_KEY: &str = "__tabview_probe_id";

/// The Swift `ProbeView`: a bare primitive that tags its node with a probe id.
struct ProbeView {
    id: u64,
}

impl PrimitiveView for ProbeView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_attachment(PROBE_KEY, self.id);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(PROBE_KEY, self.id);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
}

impl View for ProbeView {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn find_probe(graph: &Rc<ViewGraph>, id: u64) -> Option<Node> {
    let root = graph.tree.borrow().root()?;
    let mut nodes = Vec::new();
    all_nodes(&root, &mut nodes);
    nodes
        .into_iter()
        .find(|node| node.attachment::<u64>(PROBE_KEY) == Some(id))
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn probe_tabs(selection: State<i32>) -> (TabView<i32>, State<i32>) {
    let read = selection.clone();
    let write = selection.clone();
    let tabs = vec![
        TabItem::new(0, "A", AnyView::new(ProbeView { id: 100 })),
        TabItem::new(1, "B", AnyView::new(ProbeView { id: 200 })),
    ];
    (
        TabView::try_new(
            cove_core::Binding::new(move || read.get(), move |next| write.set(next)),
            tabs,
        )
        .expect("unique tab IDs"),
        selection,
    )
}

/// A counter panel whose label carries the count, matching `CountPanel`.
struct CountPanel {
    count: State<i32>,
}

impl View for CountPanel {
    fn body(&self, _cx: &Scope) -> AnyView {
        let count = self.count.clone();
        let label = format!("Count {}", count.get());
        AnyView::new(Button::new(Text::new(label), move || {
            count.set(count.get() + 1)
        }))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("count", &self.count)]
    }
}

struct SwitchingTabs {
    selection: State<i32>,
    first: CountPanel,
    second: CountPanel,
}

impl View for SwitchingTabs {
    fn body(&self, _cx: &Scope) -> AnyView {
        let read = self.selection.clone();
        let write = self.selection.clone();
        let tabs = vec![
            TabItem::new(
                0,
                "First",
                AnyView::new(CountPanel {
                    count: self.first.count.clone(),
                }),
            ),
            TabItem::new(
                1,
                "Second",
                AnyView::new(CountPanel {
                    count: self.second.count.clone(),
                }),
            ),
        ];
        AnyView::new(
            AnyView::new(
                TabView::try_new(
                    cove_core::Binding::new(move || read.get(), move |next| write.set(next)),
                    tabs,
                )
                .expect("unique tab IDs"),
            )
            .frame_size(390.0, 220.0),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("selection", &self.selection)]
    }
}

fn settle(graph: &Rc<ViewGraph>) {
    for _ in 0..5 {
        graph.recomposer.commit_all();
        graph.compute_layout(390.0, 220.0);
    }
}

/// The counter panel's activatable button. The button's own label is combined
/// from its child text, so the count is read off the descendant `StaticText`
/// and the button is found by walking up from it.
fn counter(graph: &Rc<ViewGraph>) -> Option<Node> {
    let root = graph.tree.borrow().root()?;
    let mut nodes = Vec::new();
    all_nodes(&root, &mut nodes);
    let text = nodes.into_iter().find(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::StaticText && semantics.label.starts_with("Count ")
        })
    })?;
    let mut cursor = text.parent();
    while let Some(node) = cursor {
        if node.accessibility_actions().activate.is_some() {
            return Some(node);
        }
        cursor = node.parent();
    }
    None
}

/// The count the panel currently displays.
fn counter_label(graph: &Rc<ViewGraph>) -> String {
    let root = graph.tree.borrow().root().expect("root");
    let mut nodes = Vec::new();
    all_nodes(&root, &mut nodes);
    nodes
        .into_iter()
        .find_map(|node| {
            node.accessibility().and_then(|semantics| {
                (semantics.role == AccessibilityRole::StaticText
                    && semantics.label.starts_with("Count "))
                .then_some(semantics.label)
            })
        })
        .unwrap_or_default()
}

#[test]
fn switching_tabs_keeps_separate_panel_state() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let selection = State::new(0);
            let view = SwitchingTabs {
                selection: selection.clone(),
                first: CountPanel {
                    count: State::new(0),
                },
                second: CountPanel {
                    count: State::new(0),
                },
            };
            let graph = graph();
            graph.install_view(AnyView::new(view), Default::default());
            settle(&graph);

            assert_eq!(
                counter_label(&graph),
                "Count 0",
                "the first tab starts at zero"
            );
            let activate = counter(&graph)
                .expect("the first tab's panel is mounted")
                .accessibility_actions()
                .activate
                .expect("the counter panel is an activatable button");
            activate();
            settle(&graph);
            assert_eq!(
                counter_label(&graph),
                "Count 1",
                "activating the first tab's panel increments its own state"
            );

            selection.set(1);
            settle(&graph);
            assert_eq!(
                counter_label(&graph),
                "Count 0",
                "the second tab has its own panel state, not the first tab's"
            );

            selection.set(0);
            settle(&graph);
            assert_eq!(
                counter_label(&graph),
                "Count 1",
                "the first tab keeps its own bumped count"
            );

            graph.install_view(
                AnyView::new(cove_composition::EmptyView),
                Default::default(),
            );
        });
    });
}

#[test]
fn initial_render_shows_only_the_selected_tab() {
    GlobalTestLock::locked(|| {
        let (view, _selection) = probe_tabs(State::new(0));
        let graph = graph();
        graph.install_view(AnyView::new(view), Default::default());

        assert!(
            find_probe(&graph, 100).is_some(),
            "the selected tab's content is mounted"
        );
        assert!(
            find_probe(&graph, 200).is_none(),
            "the unselected tab's content is not mounted"
        );
    });
}

#[test]
fn a_different_initial_selection_picks_the_matching_content() {
    GlobalTestLock::locked(|| {
        let (view, _selection) = probe_tabs(State::new(1));
        let graph = graph();
        graph.install_view(AnyView::new(view), Default::default());

        assert!(
            find_probe(&graph, 100).is_none(),
            "the unselected tab stays unmounted"
        );
        assert!(
            find_probe(&graph, 200).is_some(),
            "the initially selected tab mounts its content"
        );
    });
}

#[test]
fn a_selection_matching_no_tab_renders_no_content() {
    GlobalTestLock::locked(|| {
        let (view, _selection) = probe_tabs(State::new(99));
        let graph = graph();
        graph.install_view(AnyView::new(view), Default::default());

        // Swift mounts nothing when the selection matches no tab
        // (`TabView.swift:127`); the tab *stop* still falls back, but no content
        // is mounted. `active()` no longer substitutes the first enabled tab.
        assert!(
            find_probe(&graph, 100).is_none(),
            "a missed selection mounts no content"
        );
        assert!(
            find_probe(&graph, 200).is_none(),
            "no other tab is mounted either"
        );
    });
}

#[test]
fn the_tab_strip_exposes_tab_roles() {
    GlobalTestLock::locked(|| {
        let (view, _selection) = probe_tabs(State::new(0));
        let graph = graph();
        graph.install_view(AnyView::new(view), Default::default());
        graph.compute_layout(390.0, 220.0);

        let root = graph.tree.borrow().root().expect("root");
        let mut nodes = Vec::new();
        all_nodes(&root, &mut nodes);
        // Swift publishes each strip item as a tab (`TabView.swift:221`), so the
        // role and its label are reachable rather than the title being plain
        // text.
        let labels: Vec<String> = nodes
            .iter()
            .filter_map(|node| {
                let semantics = node.accessibility()?;
                (semantics.role == AccessibilityRole::Tab
                    && (semantics.label == "A" || semantics.label == "B"))
                .then_some(semantics.label)
            })
            .collect();
        assert_eq!(
            labels,
            vec!["A".to_string(), "B".to_string()],
            "both strip items publish the Tab role with their labels (got {labels:?})"
        );
    });
}

#[test]
fn dynamic_tabs_report_duplicate_ids_and_allow_empty_input() {
    use cove_ui_compose::prelude::*;
    let selection = Binding::constant(1);
    let result = TabView::try_new(
        selection.clone(),
        vec![
            TabItem::new(1, "First", Spinner::new()),
            TabItem::new(1, "Again", Divider::new()),
        ],
    );
    assert!(matches!(result, Err(DuplicateId("Tab"))));
    assert!(TabView::try_new(selection, vec![]).is_ok());
}
