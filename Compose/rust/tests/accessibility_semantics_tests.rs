//! Translation of `Tests/CoveUIComposeTests/AccessibilitySemanticsTests.swift`.
//!
//! Behaviour checklist:
//! - `accessibility_described_by` appends the referenced element's description
//!   to the target's help, and a dangling identifier adds nothing
//! - a scroll view exposes a scroll-area element carrying its label, a
//!   percentage value and help, and its increment action scrolls the content
//! - `Select` and `Combobox` triggers expose the combo-box role with their
//!   selected label, a collapsed state and an activate action


mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, TupleView, View};
use cove_core::scene::{
    AccessibilityElement, AccessibilityRole, AccessibilityTree, Align, FlexDirection,
    Justify, Node, PlatformInputContext, Wrap,
};
use cove_core::Binding;
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::fields::{Combobox, Select, SelectOption};
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn flatten(elements: &[AccessibilityElement], into: &mut Vec<AccessibilityElement>) {
    for element in elements {
        into.push(element.clone());
        flatten(&element.children, into);
    }
}

/// Builds the hierarchy after every `commit_all`/`compute_layout` pass and
/// refreshes accessibility values, matching the Swift `settle`.
fn settle(graph: &std::rc::Rc<cove_composition::ViewGraph>) {
    for _ in 0..8 {
        graph.recomposer.commit_all();
        graph.compute_layout(360.0, 400.0);
        cove_core::scene::AnimatorScheduler::current().tick(0.05);
    }
}

/// The Swift `Column(alignment: .leading, spacing:)` shape.
fn column(children: Vec<AnyView>, spacing: f32, width: Option<f32>) -> AnyView {
    let box_view = BoxView::new(
        FlexDirection::Column,
        Align::FlexStart,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(children)),
    );
    AnyView::new(match width {
        Some(width) => AnyView::new(box_view.frame_width(width)),
        None => AnyView::new(box_view),
    })
}

fn graph() -> Rc<cove_composition::ViewGraph> {
    Rc::new(cove_composition::ViewGraph::new(
        Rc::new(RefCell::new(cove_core::scene::NodeTree::new())),
        Rc::new(cove_core::scene::Recomposer::new()),
    ))
}

#[test]
fn described_by_appends_the_referenced_description() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let root = AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        Text::new("Uses the system locale").accessibility_identifier("locale-hint"),
                    ),
                    AnyView::new(
                        Button::new(Text::new("Apply"), || {})
                            .accessibility_described_by("locale-hint"),
                    ),
                    AnyView::new(
                        Button::new(Text::new("Cancel"), || {})
                            .accessibility_described_by("missing-hint"),
                    ),
                ]));

                let graph = graph();
                graph.install_view(root, Default::default());
                settle(&graph);

                let root_node = graph.tree.borrow().root().expect("root node");
                let mut elements = Vec::new();
                flatten(&AccessibilityTree::snapshot(&root_node), &mut elements);
                let buttons: Vec<&AccessibilityElement> = elements
                    .iter()
                    .filter(|element| element.semantics.role == AccessibilityRole::Button)
                    .collect();

                let apply = buttons
                    .iter()
                    .find(|element| element.semantics.label == "Apply")
                    .expect("the Apply button exposes Button semantics");
                assert!(
                    apply.semantics.help.contains("Uses the system locale"),
                    "describedBy appends the target's label to help (got {:?})",
                    apply.semantics.help
                );

                let cancel = buttons
                    .iter()
                    .find(|element| element.semantics.label == "Cancel")
                    .expect("the Cancel button exposes Button semantics");
                assert!(
                    !cancel
                        .semantics
                        .help
                        .contains("Uses the system locale"),
                    "a dangling describedBy identifier adds nothing (got {:?})",
                    cancel.semantics.help
                );
            });
        });
    });
}

fn scroll_harness() -> AnyView {
    let rows: Vec<AnyView> = (0..20)
        .map(|index| {
            AnyView::new(
                AnyView::new(Text::new(format!("Event {index}"))).frame_height(20.0),
            )
        })
        .collect();
    AnyView::new(
        ScrollView::new(
            ScrollAxis::Vertical,
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rows)),
            ),
        )
        .accessibility_label("Timeline")
        .frame_width(220.0)
        .frame_height(100.0),
    )
}

#[test]
fn scroll_view_exposes_a_scroll_area_with_position_and_page_actions() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let graph = graph();
                graph.install_view(scroll_harness(), Default::default());
                settle(&graph);

                let root_node = graph.tree.borrow().root().expect("root node");
                let mut nodes = Vec::new();
                all_nodes(&root_node, &mut nodes);
                let scroll = nodes
                    .iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)
                    })
                    .expect("the scroll view exposes a scroll area");

                let semantics = scroll.accessibility().expect("scroll area semantics");
                assert_eq!(semantics.label, "Timeline", "the label is published");
                assert_eq!(semantics.value, "0%", "an unscrolled area reports 0%");
                assert!(
                    !semantics.help.is_empty(),
                    "the scroll area explains how to scroll"
                );

                let increment = scroll
                    .accessibility_actions()
                    .increment
                    .expect("the scroll area can page forward");
                increment();
                settle(&graph);

                let semantics = scroll.accessibility().expect("scroll area semantics");
                assert_ne!(semantics.value, "0%", "paging forward moves the position");
                assert!(
                    scroll.content_offset().y > 0.0,
                    "paging forward scrolls the content (offset {})",
                    scroll.content_offset().y
                );
            });
        });
    });
}

#[test]
fn select_and_combobox_triggers_use_the_combo_box_role_and_full_name() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let selection = cove_core::State::new(1_i32);
                let combo = cove_core::State::new(Some(1_i32));

                let select = Select::new(
                    Binding::new(
                        {
                            let selection = selection.clone();
                            move || selection.get()
                        },
                        {
                            let selection = selection.clone();
                            move |value| selection.set(value)
                        },
                    ),
                    vec![
                        SelectOption::new(1, "One", true),
                        SelectOption::new(2, "Two", true),
                    ],
                );
                let combobox = Combobox::new(
                    Binding::new(
                        {
                            let combo = combo.clone();
                            move || combo.get()
                        },
                        {
                            let combo = combo.clone();
                            move |value| combo.set(value)
                        },
                    ),
                    vec![
                        SelectOption::new(1, "Alpha", true),
                        SelectOption::new(2, "Beta", true),
                    ],
                );

                let root = AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::FlexStart,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(select),
                            AnyView::new(combobox),
                        ])),
                    )
                    .frame_width(260.0),
                );

                let graph = graph();
                graph.install_view(root, Default::default());
                settle(&graph);

                let root_node = graph.tree.borrow().root().expect("root node");
                let mut nodes = Vec::new();
                all_nodes(&root_node, &mut nodes);
                let combo_boxes: Vec<&Node> = nodes
                    .iter()
                    .filter(|node| {
                        node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::ComboBox
                        })
                    })
                    .collect();
                assert!(
                    combo_boxes.len() >= 2,
                    "both triggers expose the combo-box role (got {})",
                    combo_boxes.len()
                );

                let labels: Vec<String> = combo_boxes
                    .iter()
                    .filter_map(|node| node.accessibility().map(|semantics| semantics.label))
                    .collect();
                assert!(labels.contains(&"One".to_string()), "the select names its value");
                assert!(
                    labels.contains(&"Alpha".to_string()),
                    "the combobox names its value"
                );

                for node in combo_boxes {
                    let semantics = node.accessibility().expect("combo box semantics");
                    assert_eq!(
                        semantics.state.is_expanded,
                        Some(false),
                        "an unopened trigger is collapsed"
                    );
                    assert!(
                        node.accessibility_actions().activate.is_some(),
                        "a combo box can be activated"
                    );
                }
            });
        });
    });
}

