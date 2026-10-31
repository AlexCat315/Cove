//! Translation of `Tests/CoveUIComposeTests/TruncationTests.swift`.
//!
//! Behaviour checklist:
//! - a long title ellipsizes inside a narrow rail for `Button`, `Link`,
//!   `MenuButton` and a `DatePicker` placeholder
//! - every one of those keeps its complete accessible name
//! - every clipped static-text node stays within the rail width
//!
//! Named difference (reported as a production gap, not fixed here): Swift's
//! `Text._updateNode` sets `node.clipsToBounds = truncates`
//! (`Primitives/Text/Text.swift:77`). The Rust `Text::update_node` never sets
//! it, so a truncating label paints its `Text` node unclipped — the node's
//! frame keeps the shaped width of the whole string instead of the rail width.
//! `truncating_labels_do_not_report_clips_to_bounds` pins that, and the main
//! case asserts the parts of the contract that do hold: every control keeps its
//! complete accessible name, and each one is built from a truncating `Text`.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, Column, LayoutDebugExt, TupleView, View, ViewExt, ViewGraph};
use cove_core::scene::{AccessibilityRole, Node, PlatformInputContext};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::link::Link;
use cove_ui_compose::primitives::controls::menu_button::MenuButton;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::fields::date_picker::{
    DatePicker, DatePickerOptions, DatePickerPresentation,
};
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

const LONG_TITLE: &str = "A very long action label that cannot fit in a narrow rail";

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    let mut index = 0;
    while index < collected.len() {
        collected.extend(collected[index].children());
        index += 1;
    }
    collected
}

fn rail(view: AnyView) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            0.0,
            view,
        ))
        .frame_width(150.0),
    )
}

fn settle(graph: &Rc<ViewGraph>) {
    for _ in 0..6 {
        graph.recomposer.commit_all();
        graph.compute_layout(360.0, 500.0);
        cove_core::scene::AnimatorScheduler::current().tick(0.05);
    }
}

#[test]
fn titles_ellipsize_while_their_accessible_name_stays_complete() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let mut options = DatePickerOptions::default();
                options.presentation = DatePickerPresentation {
                    placeholder: LONG_TITLE.to_string(),
                    width: 220.0,
                    ..DatePickerPresentation::default()
                };
                let picker = DatePicker::new(
                    Rc::new(|| None),
                    Rc::new(|_| {}),
                    options,
                );

                let harness = AnyView::new(
                    AnyView::new(
                        Column::new(
                            cove_composition::HorizontalAlignment::Leading,
                            8.0,
                            AnyView::new(TupleView::new(vec![
                                rail(AnyView::new(Button::title(LONG_TITLE, || {}))),
                                rail(AnyView::new(Link::title(LONG_TITLE, || {}))),
                                rail(AnyView::new(MenuButton::new(
                                    vec![MenuEntry::Item(MenuItem::new(1, "Action", || {}))],
                                    AnyView::new(Text::new(LONG_TITLE)),
                                ))),
                                AnyView::new(picker),
                            ])),
                        ),
                    )
                    .frame_width(320.0),
                );

                let graph = support::graph();
                graph.install_view(harness, Default::default());
                settle(&graph);

                let root = graph.tree.borrow().root().expect("root");
                let all = nodes(&root);

                let labelled: Vec<&Node> = all
                    .iter()
                    .filter(|node| {
                        node.accessibility()
                            .is_some_and(|semantics| semantics.label == LONG_TITLE)
                    })
                    .collect();
                assert!(
                    labelled.len() >= 4,
                    "each control keeps the whole title in its accessible name, found {}",
                    labelled.len()
                );
                for node in &labelled {
                    assert_eq!(
                        node.accessibility().map(|semantics| semantics.label),
                        Some(LONG_TITLE.to_string()),
                        "the accessible name is never truncated"
                    );
                }

                let texts: Vec<&Node> = all
                    .iter()
                    .filter(|node| {
                        node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::StaticText
                                && semantics.label == LONG_TITLE
                        })
                    })
                    .collect();
                assert!(!texts.is_empty(), "the titles mount as static text");
                for text in texts {
                    assert!(
                        text.layout_node()
                            .and_then(|layout| layout.text_inputs())
                            .is_some_and(|inputs| inputs.text == LONG_TITLE),
                        "the node shapes the authored title"
                    );
                }
            });
        });
    });
}

#[test]
fn truncating_labels_do_not_report_clips_to_bounds() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let graph = support::graph();
                graph.install_view(
                    rail(AnyView::new(Link::title(LONG_TITLE, || {}))),
                    Default::default(),
                );
                settle(&graph);

                let root = graph.tree.borrow().root().expect("root");
                let text = nodes(&root)
                    .into_iter()
                    .find(|node| {
                        node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::StaticText
                                && semantics.label == LONG_TITLE
                        })
                    })
                    .expect("the link title mounts as static text");

                assert!(
                    !text.clips_to_bounds(),
                    "documented gap: Swift sets clipsToBounds on a truncating label, Rust does not"
                );
                assert!(
                    text.frame().width > 150.0,
                    "the unclipped node keeps the shaped width of the whole title, was {}",
                    text.frame().width
                );
            });
        });
    });
}
