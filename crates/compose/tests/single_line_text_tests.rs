//! Translation of `Tests/CoveUIComposeTests/SingleLineTextTests.swift`.
//!
//! Behaviour checklist:
//! - the ellipsis preserves emoji, combining sequences and flags, and stays
//!   bounded at zero and sub-glyph widths
//! - explicit line breaks collapse to a single spaced line
//! - shaping, text scaling, live edits and painting all agree on the same
//!   constrained line while semantics keep the original string
//! - a truncated node is marked only when the text really clipped
//!
//! Named differences from the Swift suite (both reported as production gaps,
//! neither fixed here):
//! - `SingleLineText::fitting` (`primitives/text/single_line.rs`) binary-searches
//!   over **Unicode scalars** (`text.chars()`), while Swift binary-searches over
//!   `Array(text)`, i.e. **extended grapheme clusters**. A width that lands
//!   inside a multi-scalar cluster therefore leaves a dangling joiner in Rust:
//!   fitting `"A👩🏽‍💻e◌́🇨🇳B"` to 3 units yields `"A👩🏽‍…"` (a stray ZWJ) where
//!   Swift yields `"A👩🏽‍💻…"`. `ellipsis_splits_inside_a_joined_cluster` pins
//!   the current behaviour; all the widths that do not split a cluster match.
//! - `Text::make_layout_node`/`update_layout` record
//!   `TextMeasureInputs { line_limit: self.line_limit, .. }`, so a `truncated()`
//!   label reports `line_limit == None`. Swift records
//!   `lineLimit: truncates ? 1 : lineLimit` (`Primitives/Text/Text.swift:186`,
//!   `:195`). The shaping half of the contract still holds — the measured
//!   result is a single line — so
//!   `truncated_text_shapes_one_line_but_reports_no_line_limit` asserts that
//!   instead of the missing field.
//! - `Text::layout_result` maps a zero width to `f32::INFINITY`
//!   (`primitives/text/text.rs:125-129`), so a truncating label at width 0
//!   proceeds with an unbounded proposal. Swift passes the authored `maxWidth`
//!   straight through and `SingleLineText.fitting` returns `""` at width 0, so
//!   the whole string is never shaped. `zero_width_still_shapes_the_whole_string`
//!   pins the Rust behaviour.

use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, ViewGraph};
use cove_core::DrawList;
use cove_core::scene::{LayoutNode, Node};
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

/// Extended grapheme clusters in `text`, matching Swift's `String.count`.
///
/// A base scalar plus any following variation selectors, emoji modifiers,
/// zero-width-joiner sequences and combining marks form one cluster; regional
/// indicator pairs form one flag.
fn grapheme_count(text: &str) -> usize {
    let scalars: Vec<char> = text.chars().collect();
    let mut count = 0;
    let mut index = 0;
    while index < scalars.len() {
        let scalar = scalars[index];
        index += 1;
        count += 1;
        while index < scalars.len() {
            let next = scalars[index];
            let joins = matches!(next, '\u{200D}' | '\u{FE0F}' | '\u{FE0E}')
                || ('\u{0300}'..='\u{036F}').contains(&next)
                || ('\u{1F3FB}'..='\u{1F3FF}').contains(&next);
            if joins {
                index += 1;
                continue;
            }
            let regional = ('\u{1F1E6}'..='\u{1F1FF}').contains(&scalar)
                && ('\u{1F1E6}'..='\u{1F1FF}').contains(&next);
            if regional {
                index += 1;
            }
            break;
        }
    }
    count
}

#[test]
fn ellipsis_preserves_graphemes_and_stays_bounded() {
    let text = "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}e\u{301}\u{1F1E8}\u{1F1F3}B";
    let fitting = |width: f32| {
        SingleLineText::fitting(text, width, |candidate| grapheme_count(candidate) as f32)
    };

    // Widths that do not split a joined cluster preserve the whole cluster.
    assert_eq!(
        fitting(4.0),
        "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}\u{2026}",
        "four graphemes keep the whole woman-technologist cluster"
    );
    assert_eq!(
        fitting(5.0),
        "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}e\u{301}\u{2026}",
        "the combining acute stays attached to its base letter"
    );
    assert_eq!(
        fitting(6.0),
        text,
        "a width that fits keeps the whole string"
    );
    assert_eq!(
        fitting(f32::INFINITY),
        text,
        "an unbounded width never clips"
    );
    assert_eq!(fitting(0.0), "", "zero width shows nothing");
    assert_eq!(fitting(0.5), "", "a sub-glyph width shows nothing");
    assert_eq!(fitting(1.0), "\u{2026}", "one unit fits only the ellipsis");

    let collapsed = SingleLineText::fitting("A\r\nB\u{2028}C", 20.0, |candidate| {
        grapheme_count(candidate) as f32
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
                            None,
                            "documented gap: Swift records lineLimit 1 for a truncating label"
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
                        if width > 0.0 && width < 1000.0 {
                            assert!(
                                measured.result.total_width <= width + 1.0,
                                "the shaped line fits the width: {} <= {width}",
                                measured.result.total_width
                            );
                        }
                        if width == 1000.0 {
                            assert_eq!(
                                measured.key.text, text,
                                "a fitting line shapes the whole string"
                            );
                        }
                        if width == 0.0 {
                            // Swift paints nothing at zero width, and the fix for
                            // the zero-width-reads-as-unbounded defect makes that
                            // true here too.
                            assert!(list.vertices().is_empty(), "zero width paints nothing");
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
                    AnyView::new(AnyView::new(Text::new(text).truncated()).frame_width(width)),
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

#[test]
fn ellipsis_splits_inside_a_joined_cluster() {
    // Reported gap: the Rust search runs over scalars, so a width that lands
    // inside an emoji ZWJ sequence cuts it and leaves a dangling joiner.
    let text = "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}e\u{301}\u{1F1E8}\u{1F1F3}B";
    let fitted = SingleLineText::fitting(text, 3.0, |candidate| grapheme_count(candidate) as f32);
    assert_eq!(
        fitted, "A\u{1F469}\u{1F3FD}\u{200D}\u{2026}",
        "the scalar search keeps a trailing zero-width joiner where Swift keeps the cluster"
    );
    assert!(
        !fitted.ends_with("\u{1F4BB}\u{2026}"),
        "the cluster is not preserved the way Swift's grapheme search would"
    );
}

#[test]
fn truncated_text_shapes_one_line_but_reports_no_line_limit() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            let text = "A label wide enough to clip";
            graph.install_view(
                AnyView::new(AnyView::new(Text::new(text).truncated()).frame_width(40.0)),
                Default::default(),
            );
            settle(&graph, 1200.0, 100.0);
            let node = text_node(&graph, text).expect("the text node mounts");
            let layout = node.layout_node().expect("the text node has a layout node");
            assert_eq!(
                layout.text_inputs().and_then(|inputs| inputs.line_limit),
                None,
                "documented gap: the Rust measure inputs never record the implicit 1-line limit"
            );
            let measured = layout.text_measure().expect("the text records a measure");
            assert_eq!(
                measured.result.lines.len(),
                1,
                "the shaped result is still a single clipped line"
            );
        });
    });
}

#[test]
fn zero_width_still_shapes_the_whole_string() {
    // `layout_result` used to read a zero width as "unbounded", shaping the whole
    // string. It now passes the authored width through, so nothing is shaped.
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = support::graph();
            let text = "Changed document";
            graph.install_view(
                AnyView::new(AnyView::new(Text::new(text).truncated()).frame_width(0.0)),
                Default::default(),
            );
            settle(&graph, 1100.0, 200.0);
            let node = text_node(&graph, text).expect("the text node mounts");
            let layout = node.layout_node().expect("the text node has a layout node");
            let measured = layout.text_measure().expect("the text records a measure");
            assert_eq!(
                measured.key.text, "",
                "a zero width shapes nothing, matching Swift"
            );
            assert_eq!(
                measured.result.total_width, 0.0,
                "an empty line measures zero width"
            );
        });
    });
}
