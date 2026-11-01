//! Translation of `Tests/CoveUIComposeTests/ChromeRegressionTests.swift`.
//!
//! Visual contracts found by the Gallery audit: controls keep their intrinsic
//! drawing when stretched, and container chrome is not silently erased.
//!
//! Behaviour checklist:
//! - a switch stretched by a value slot keeps a narrow track at the leading edge
//! - a badge with no title and `shows_dot` renders a bare dot, not an empty pill
//! - an empty state centers its capped content in a wide container
//! - an empty Combobox trigger spans its width instead of hugging a placeholder
//! - a Select trigger keeps its input border

mod support;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, DrawList, Point};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Toggle;
use cove_ui_compose::primitives::controls::fields::{Combobox, Select, SelectOption};
use cove_ui_compose::primitives::feedback::{Badge, EmptyState};
use cove_ui_compose::{BoxView, EmptyView};

use support::{GlobalTestLock, with_test_text_environment};

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn nodes(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

/// Installs `view`, settles four frames, and returns the graph.
fn render(view: AnyView, width: f32) -> std::rc::Rc<cove_composition::ViewGraph> {
    let graph = support::graph();
    graph.install_view(view, Default::default());
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, 400.0);
    }
    graph
}

fn boxed(content: AnyView) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        content,
    ))
}

#[test]
fn a_stretched_toggle_keeps_its_track_at_the_leading_edge() {
    GlobalTestLock::locked(|| {
        let graph = render(
            AnyView::new(
                AnyView::new(AnyView::new(Toggle::new(Binding::constant(true))).flex(1.0))
                    .frame_width(300.0),
            ),
            600.0,
        );
        let toggle = nodes(&graph)
            .into_iter()
            .find(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Toggle)
            })
            .expect("the toggle publishes its accessibility role");

        assert!(
            toggle.frame().width > 200.0,
            "the value slot stretches the host, width was {}",
            toggle.frame().width
        );

        let mut draw = DrawList::default();
        toggle.draw().expect("the toggle paints")(&mut draw, Point::default());
        let xs: Vec<f32> = draw.vertices().iter().map(|vertex| vertex.x).collect();
        let min = xs.iter().copied().fold(f32::INFINITY, f32::min);
        let max = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max - min <= 40.0,
            "the track keeps its intrinsic width, painted span was {}",
            max - min
        );
        assert!(
            min < 2.0,
            "the track sits at the leading edge, min x was {min}"
        );
    });
}

#[test]
fn a_titleless_dot_badge_renders_a_bare_dot() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = render(
                boxed(AnyView::new(
                    Badge::new("")
                        .tone(cove_ui_compose::primitives::feedback::StatusTone::Success)
                        .shows_dot(true),
                )),
                600.0,
            );
            let badge = nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::StaticText)
                })
                .expect("the badge publishes a static-text role");
            let dot = badge
                .children()
                .first()
                .cloned()
                .expect("the badge has a child");

            assert_eq!(
                dot.frame().width,
                dot.frame().height,
                "the dot is square, was {:?}",
                dot.frame()
            );
            assert!(
                dot.frame().width <= 10.0 && dot.frame().width >= 7.0,
                "the dot keeps its 7-10pt size, was {:?}",
                dot.frame()
            );
            assert!(
                dot.children().is_empty(),
                "the dot renders no glyph child, has {}",
                dot.children().len()
            );
            assert!(
                dot.background_color().is_some(),
                "the dot is a filled shape, not an empty pill"
            );
        });
    });
}

#[test]
fn an_empty_state_centers_its_content_in_a_wide_container() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = render(
                AnyView::new(
                    AnyView::new(EmptyState::with_action(
                        "No records",
                        "Change your filters.",
                        AnyView::new(EmptyView),
                    ))
                    .frame(
                        Some(cove_ui_compose::modifiers::layout::FrameDimension::Points(
                            900.0,
                        )),
                        Some(cove_ui_compose::modifiers::layout::FrameDimension::Points(
                            300.0,
                        )),
                        None,
                        None,
                        None,
                        None,
                    ),
                ),
                900.0,
            );
            let title = nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == "No records")
                })
                .expect("the title materialises as measured text");

            let frame = title.absolute_frame();
            let mid_x = frame.x + frame.width / 2.0;
            let mid_y = frame.y + frame.height / 2.0;
            assert!(
                (mid_x - 450.0).abs() < 2.0,
                "the title is horizontally centered, midX was {mid_x}"
            );
            assert!(
                (mid_y - 150.0).abs() < 60.0,
                "the title is vertically near the middle, midY was {mid_y}"
            );
        });
    });
}

#[test]
fn an_empty_combobox_trigger_spans_its_width() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = render(
                boxed(AnyView::new(Combobox::new(
                    Binding::constant(None::<i32>),
                    vec![SelectOption::new(1, "One", true)],
                ))),
                600.0,
            );
            let chrome = nodes(&graph)
                .into_iter()
                .filter(|node| {
                    node.border_color()
                        .is_some_and(|color| node.border_width() >= 1.0 && color.a > 0.0)
                })
                .max_by(|a, b| {
                    a.frame()
                        .width
                        .partial_cmp(&b.frame().width)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .expect("the combobox trigger paints an input border");

            assert_eq!(
                chrome.frame().width,
                280.0,
                "the empty trigger spans the control width instead of hugging the placeholder, was {:?}",
                chrome.frame()
            );
        });
    });
}

#[test]
fn a_select_trigger_keeps_its_input_border() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let graph = render(
                AnyView::new(Select::new(
                    Binding::constant(1),
                    vec![SelectOption::new(1, "One", true)],
                )),
                600.0,
            );
            let bordered: Vec<Node> = nodes(&graph)
                .into_iter()
                .filter(|node| {
                    node.border_width() >= 1.0
                        && node.border_color().is_some_and(|color| color.a > 0.0)
                })
                .collect();

            assert!(
                !bordered.is_empty(),
                "the select trigger keeps its input border; the focus ring must not erase it"
            );
        });
    });
}
