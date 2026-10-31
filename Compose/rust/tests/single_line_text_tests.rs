//! Translation of `Tests/CoveUIComposeTests/SingleLineTextTests.swift`.
//!
//! Behaviour checklist:
//! - the ellipsis preserves emoji, combining sequences and flags, and stays
//!   bounded at zero and sub-glyph widths
//! - explicit line breaks collapse to a single spaced line
//! - shaping, text scaling, live edits and painting all agree on the same
//!   constrained line while semantics keep the original string
//! - a truncated node is marked only when the text really clipped

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View, ViewExt, ViewGraph};
use cove_core::scene::{LayoutNode, Node};
use cove_core::DrawList;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::text::{Text, single_line::SingleLineText};
use cove_ui_compose::theme::tokens::text_scaler::{TextScaler, TextScalerViewExt};

use support::{GlobalTestLock, with_test_text_environment};

fn nodes(graph: &Rc<ViewGraph>) -> Vec<Node> {
    let Some(root) = graph.tree.borrow().root() else {
        return Vec::new();
    };
    let mut collected = vec![root.clone()];
    let mut index = 0;
    while index < collected.len() {
        collected.extend(collected[index].children());
        index += 1;
    }
    collected
}

fn text_node(graph: &Rc<ViewGraph>, text: &str) -> Option<Node> {
    nodes(graph).into_iter().find(|node| {
        node.layout_node()
            .and_then(|layout| layout.text_inputs())
            .is_some_and(|inputs| inputs.text == text)
    })
}

fn settle(graph: &Rc<ViewGraph>, width: f32, height: f32) {
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, height);
    }
}

#[test]
fn ellipsis_preserves_graphemes_and_stays_bounded() {
    let text = "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}e\u{301}\u{1F1E8}\u{1F1F3}B";
    let fitting = |width: f32| {
        SingleLineText::fitting(text, width, |candidate| candidate.chars().count() as f32)
    };

    assert_eq!(fitting(3.0), "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}\u{2026}");
    assert_eq!(
        fitting(4.0),
        "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}e\u{301}\u{2026}"
    );
    assert_eq!(fitting(5.0), text, "a width that fits keeps the whole string");
    assert_eq!(fitting(f32::INFINITY), text, "an unbounded width never clips");
    assert_eq!(fitting(0.0), "", "zero width shows nothing");
    assert_eq!(fitting(0.5), "", "a sub-glyph width shows nothing");
    assert_eq!(fitting(1.0), "\u{2026}", "one unit fits only the ellipsis");

    let collapsed = SingleLineText::fitting("A\r\nB\u{2028}C", 20.0, |candidate| {
        candidate.chars().count() as f32
    });
    assert_eq!(
        collapsed, "A B C",
        "hard breaks collapse to single spaces on one line"
    );
}

#[test]
fn shaping_edits_and_paint_agree_on_one_constrained_line() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            for text in [
                "AVfi office \u{1F469}\u{1F3FD}\u{200D}\u{1F4BB} permissions \u{6587}\u{4EF6}",
                "\u{645}\u{631}\u{62D}\u{628}\u{627} \u{628}\u{627}\u{644}\u{645}\u{633}\u{62A}\u{62E}\u{62F}\u{645} \u{641}\u{64A} \u{645}\u{633}\u{627}\u{62D}\u{629} \u{627}\u{644}\u{639}\u{645}\u{644}",
                "Changed document",
            ] {
                for scale in [1.0_f32, 2.0] {
                    for width in [0.0_f32, 5.0, 60.0, 160.0, 1000.0] {
                        graph.install_view(
                            AnyView::new(
                                AnyView::new(Text::new(text).truncated())
                                    .frame_width(width)
                                    .text_scaler(TextScaler::new(scale)),
                            ),
                            Default::default(),
                        );
                        settle(&graph, 1100.0, 200.0);

                        let node = text_node(&graph, text).unwrap_or_else(|| {
                            panic!("the text node mounts for {text:?} at width {width}")
                        });
                        let layout: LayoutNode =
                            node.layout_node().expect("the text node has a layout node");
                        assert_eq!(
                            node.accessibility().map(|semantics| semantics.label),
                            Some(text.to_string()),
                            "semantics keep the authored string at width {width}"
                        );
                        assert_eq!(
                            layout.text_inputs().and_then(|inputs| inputs.line_limit),
                            Some(1),
                            "truncated text is limited to one line at width {width}"
                        );

                        let mut list = DrawList::default();
                        cove_core::scene::NodeRenderer::new()
                            .render(&graph.tree.borrow().root().expect("root"), &mut list);

                        let measured = layout.text_measure().expect("the text records a measure");
                        assert!(
                            measured.result.lines.len() <= 1,
                            "at most one line is shaped, got {} at width {width}",
                            measured.result.lines.len()
                        );
                        assert!(
                            measured.result.total_width <= width + 1.0,
                            "the shaped line fits the width: {} <= {width}",
                            measured.result.total_width
                        );
                        if width == 0.0 {
                            assert!(
                                list.vertices().is_empty(),
                                "zero width paints nothing"
                            );
                        }
                        if width == 1000.0 {
                            assert_eq!(
                                measured.key.text, text,
                                "a fitting line shapes the whole string"
                            );
                        }
                    }
                }
            }
        });
    });
}

#[test]
fn truncated_text_marks_its_node_only_when_really_clipped() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            let text = "A label wide enough to clip";
            let marks = |width: f32| -> bool {
                graph.install_view(
                    AnyView::new(
                        AnyView::new(Text::new(text).truncated())
                            .frame_width(width),
                    ),
                    Default::default(),
                );
                graph.recomposer.commit_all();
                graph.compute_layout(1200.0, 100.0);
                let root = graph.tree.borrow().root().expect("root");
                let mut list = DrawList::default();
                cove_core::scene::NodeRenderer::new().render(&root, &mut list);
                let node = text_node(&graph, text).expect("the text node mounts");
                node.attachment::<bool>("__text_truncated").unwrap_or(false)
            };

            assert!(marks(40.0), "a narrow width clips and marks the node");
            assert!(
                !marks(1000.0),
                "a width that fits leaves the node unmarked so the tooltip waits"
            );
        });
    });
}
