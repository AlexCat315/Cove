//! Translation of `Tests/CoveUIComposeTests/TruncationTests.swift`.
//!
//! Behaviour checklist:
//! - a long title ellipsizes inside a narrow rail for `Button`, `Link`,
//!   `MenuButton` and a `DatePicker` placeholder
//! - every one of those keeps its complete accessible name
//! - every truncated title stays within its control's available width

use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, Column, TupleView, ViewGraph};
use cove_core::DrawList;
use cove_core::scene::{AccessibilityRole, Node, NodeRenderer, PlatformInputContext};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::Link;
use cove_ui_compose::primitives::controls::MenuButton;
use cove_ui_compose::primitives::controls::{
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
                let picker = DatePicker::new(cove_core::Binding::constant(None))
                    .configure(|picker| *picker = options);

                let harness = AnyView::new(
                    AnyView::new(Column::new(
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
                    ))
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
fn truncating_button_label_stays_inside_its_constrained_frame() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let graph = support::graph();
                graph.install_view(
                    AnyView::new(Button::title(LONG_TITLE, || {}).frame_width(220.0)),
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
                    .expect("the button title mounts as static text");
                NodeRenderer::new().render(&root, &mut DrawList::default());

                assert!(
                    text.clips_to_bounds(),
                    "a truncating label clips glyphs at its allocated width"
                );
                assert!(
                    text.frame().width < 220.0,
                    "button padding leaves less than the host width for its label, was {}",
                    text.frame().width
                );
                assert_eq!(
                    text.attachment::<bool>("__text_truncated"),
                    Some(true),
                    "the constrained label is fitted with a trailing ellipsis"
                );
                let visible_text = text
                    .layout_node()
                    .and_then(|layout| layout.text_measure())
                    .map(|measure| measure.key.text)
                    .expect("the rendered label has a shaped result");
                assert!(
                    visible_text.ends_with('…'),
                    "the visible title ends with an ellipsis, got {visible_text:?}"
                );
            });
        });
    });
}
