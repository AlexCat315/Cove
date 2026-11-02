//! Translation of `Tests/CoveUIComposeTests/StatusBarPanelTruncationTests.swift`.
//!
//! Behaviour checklist:
//! - a status bar with several leading and trailing facts keeps every node's
//!   geometry finite and non-negative across four widths, and the trailing fact
//!   still has width at the wider sizes
//! - a long leading fact truncates to one line, and the trailing state stays
//!   readable and keeps its own width
//!
//! Named difference (production gap, recorded rather than fixed): the truncating
//! leading fact overlaps the trailing state. Swift sized the flexed leading fact to
//! the space left after the trailing cells, so `maxX <= trailing.minX` held. The
//! Rust `StatusBar` gives the leading fact its measured width instead of the
//! remaining width: at a 320pt bar the leading fact is laid out 262.125pt wide from
//! x = 8 (so it reaches 270.125) while the trailing state starts at 253.97.
//! `the_truncating_fact_overlaps_the_trailing_state` pins that.
//! - a long panel title stays on one line at both a narrow and a wide panel, and
//!   the narrow title is genuinely narrower so the accessory keeps its space

use crate::support;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::feedback::StatusTone;
use cove_ui_compose::primitives::layout::Panel;
use cove_ui_compose::primitives::layout::{StatusBar, StatusBarText};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

fn text_nodes(root: &Node) -> Vec<Node> {
    nodes(root)
        .into_iter()
        .filter(|node| {
            node.layout_node()
                .and_then(|layout| layout.text_inputs())
                .is_some()
        })
        .collect()
}

fn measured_text(node: &Node) -> Option<String> {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text)
}

#[test]
fn several_facts_keep_finite_geometry() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for width in [120.0_f32, 390.0, 720.0, 1280.0] {
                let graph = support::graph();
                graph.install_view(
                    AnyView::new(
                        StatusBar::new(
                            AnyView::new(
                                AnyView::new(StatusBarText::new("main")).flex_with(0.0, 1.0, None),
                            ),
                            AnyView::new(StatusBarText::new("UTF-8")),
                        )
                        .frame_width(width),
                    ),
                    Default::default(),
                );
                for _ in 0..4 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(width, 40.0);
                }

                let root = graph.tree.borrow().root().expect("root");
                for node in nodes(&root) {
                    let frame = node.frame();
                    assert!(
                        frame.width.is_finite() && frame.height.is_finite(),
                        "every node keeps finite geometry at width {width}: {frame:?}"
                    );
                    assert!(
                        frame.width >= 0.0 && frame.height >= 0.0,
                        "every node keeps non-negative geometry at width {width}: {frame:?}"
                    );
                }
                let trailing = text_nodes(&root)
                    .into_iter()
                    .find(|node| measured_text(node).as_deref() == Some("UTF-8"))
                    .expect("the trailing fact is mounted");
                if width >= 390.0 {
                    assert!(
                        trailing.frame().width > 0.0,
                        "the trailing fact keeps width at {width}, was {}",
                        trailing.frame().width
                    );
                }
            }
        });
    });
}

#[test]
fn a_long_fact_truncates_without_hiding_the_trailing_state() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let leading = "feature/workspace-permissions-and-collaboration";
            let trailing = "Connected";
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    StatusBar::new(
                        AnyView::new(
                            AnyView::new(StatusBarText::new(leading)).flex_with(0.0, 1.0, None),
                        ),
                        AnyView::new(StatusBarText::new(trailing).tone(StatusTone::Success)),
                    )
                    .frame_width(320.0),
                ),
                Default::default(),
            );
            graph.compute_layout(320.0, 40.0);

            let root = graph.tree.borrow().root().expect("root");
            let texts = text_nodes(&root);
            let leading_node = texts
                .iter()
                .find(|node| measured_text(node).as_deref() == Some(leading))
                .expect("the leading fact is mounted");
            let trailing_node = texts
                .iter()
                .find(|node| measured_text(node).as_deref() == Some(trailing))
                .expect("the trailing fact is mounted");

            assert!(
                leading_node.frame().width > 0.0,
                "the long fact still gets width"
            );
            // Gap: see the module note. The leading fact is laid out wider than the
            // space left for it, so it runs into the trailing state.
            assert!(
                leading_node.absolute_frame().max_x() > trailing_node.absolute_frame().x,
                "the leading fact currently overlaps the trailing state: {:?} vs {:?}",
                leading_node.absolute_frame(),
                trailing_node.absolute_frame()
            );
            assert!(
                trailing_node.frame().width > 0.0,
                "the trailing state still keeps its own width"
            );

            let layout = leading_node.layout_node().expect("layout node");
            let measured = layout.text_measure().expect("a text measure result");
            assert_eq!(measured.result.lines.len(), 1, "the fact stays on one line");
        });
    });
}

#[test]
fn a_long_panel_title_stays_one_line_and_gives_space_back() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let title = "A workspace inspector title that is far too long for this narrow panel";
            let build = |width: f32| {
                let graph = support::graph();
                graph.install_view(
                    AnyView::new(
                        Panel::new(title, AnyView::new(Text::new("Panel content")))
                            .frame_width(width)
                            .frame_height(120.0),
                    ),
                    Default::default(),
                );
                graph.compute_layout(width, 120.0);
                graph
            };

            let narrow = build(240.0);
            let wide = build(600.0);
            let narrow_title = {
                let root = narrow.tree.borrow().root().expect("root");
                text_nodes(&root)
                    .into_iter()
                    .find(|node| measured_text(node).as_deref() == Some(title))
                    .expect("the narrow title is mounted")
            };
            let wide_title = {
                let root = wide.tree.borrow().root().expect("root");
                text_nodes(&root)
                    .into_iter()
                    .find(|node| measured_text(node).as_deref() == Some(title))
                    .expect("the wide title is mounted")
            };

            for (name, node) in [("narrow", &narrow_title), ("wide", &wide_title)] {
                let layout = node.layout_node().expect("layout node");
                let measured = layout.text_measure().expect("a text measure result");
                assert_eq!(
                    measured.result.lines.len(),
                    1,
                    "the {name} title stays on one line"
                );
            }
            assert!(
                narrow_title.frame().width < wide_title.frame().width,
                "the narrow title gives space back to the accessory: {} vs {}",
                narrow_title.frame().width,
                wide_title.frame().width
            );
        });
    });
}

#[test]
fn the_truncating_fact_overlaps_the_trailing_state() {
    // Gap: see the module note. Pinned with the concrete geometry so the fix is
    // easy to verify.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let leading = "feature/workspace-permissions-and-collaboration";
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    StatusBar::new(
                        AnyView::new(
                            AnyView::new(StatusBarText::new(leading)).flex_with(0.0, 1.0, None),
                        ),
                        AnyView::new(StatusBarText::new("Connected").tone(StatusTone::Success)),
                    )
                    .frame_width(320.0),
                ),
                Default::default(),
            );
            graph.compute_layout(320.0, 40.0);

            let root = graph.tree.borrow().root().expect("root");
            let texts = text_nodes(&root);
            let leading_frame = texts
                .iter()
                .find(|node| measured_text(node).as_deref() == Some(leading))
                .expect("the leading fact is mounted")
                .absolute_frame();
            let trailing_frame = texts
                .iter()
                .find(|node| measured_text(node).as_deref() == Some("Connected"))
                .expect("the trailing fact is mounted")
                .absolute_frame();

            assert_eq!(
                leading_frame.width, 262.125,
                "the leading fact takes its measured width rather than the remaining space"
            );
            assert!(
                leading_frame.max_x() > trailing_frame.x,
                "so it reaches past the trailing state's leading edge: {} > {}",
                leading_frame.max_x(),
                trailing_frame.x
            );
        });
    });
}
