//! Translation of `Tests/CoveUIComposeTests/FocusRingTests.swift`.
//!
//! Behaviour checklist:
//! - a pointer click focuses without ringing
//! - keyboard focus rings, and the ring is dropped again on a pointer click
//! - losing focus clears the ring
//! - custom-drawn toggle, slider and range slider ring only for keyboard focus
//!
//! The Swift suite installed a provider-less theme read off
//! `graph.tree.root!.theme`; the Rust port resolves the active theme through
//! `theme_for_node`, so the ring colour is read from the same source the style
//! uses.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::scene::{FocusChain, FocusChainHolder, Node};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::fields::range_slider::RangeSlider;
use cove_ui_compose::primitives::controls::fields::range_slider::RangeValue as RangeSliderValue;
use cove_ui_compose::primitives::controls::fields::slider::Slider;
use cove_ui_compose::primitives::controls::toggle::Toggle;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::{
    ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt,
};
use cove_ui_compose::theme::theme_for_node;

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

fn rings(graph: &Rc<cove_composition::ViewGraph>) -> Vec<Node> {
    let root = graph.tree.borrow().root().expect("root");
    nodes(&root)
        .into_iter()
        .filter(|node| node.border_width() > 0.0)
        .collect()
}

/// A ring-only style — the shape every custom list/tab/sidebar style uses.
struct RowStyle;

impl ButtonStyle for RowStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let (color, width) = if configuration.is_focused {
            (configuration.theme.colors.focus_ring, 2.0)
        } else {
            (cove_core::Color::CLEAR, 0.0)
        };
        AnyView::new(
            cove_composition::AnyView::new(configuration.label).with_modifier(
                cove_ui_compose::modifiers::style::BorderModifier::new(color, width),
            ),
        )
    }
}

fn styled() -> AnyView {
    AnyView::new(AnyView::new(Button::new(Text::new("Destination"), || {})).button_style(RowStyle))
}

fn with_scene(
    root: AnyView,
    test: impl FnOnce(&Rc<cove_composition::ViewGraph>, &Rc<FocusChain>, &Node),
) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let chain = Rc::new(FocusChain::new());
            FocusChainHolder::set_current(Some(chain.clone()));
            let graph = support::graph();
            graph.install_view(root, Default::default());
            graph.compute_layout(320.0, 200.0);

            let root_node = graph.tree.borrow().root().expect("root");
            let target = nodes(&root_node)
                .into_iter()
                .find(|node| node.is_focusable())
                .expect("a focusable target mounts");
            test(&graph, &chain, &target);

            graph.install_view(
                AnyView::new(cove_composition::EmptyView),
                Default::default(),
            );
            cove_core::scene::AnimatorScheduler::current().tick(1.0);
            FocusChainHolder::set_current(None);
        });
    });
}

#[test]
fn pointer_focus_is_silent() {
    with_scene(styled(), |graph, chain, target| {
        chain.focus(Some(target), false);
        graph.recomposer.commit_all();
        graph.compute_layout(320.0, 200.0);
        assert_eq!(
            chain.focused().as_ref(),
            Some(target),
            "the pointer still moves the keyboard target"
        );
        assert!(
            rings(graph).is_empty(),
            "a pointer-acquired focus leaves no ring"
        );
    });
}

#[test]
fn keyboard_focus_rings() {
    with_scene(styled(), |graph, chain, target| {
        chain.focus(Some(target), true);
        graph.recomposer.commit_all();
        graph.compute_layout(320.0, 200.0);
        let ring = rings(graph).into_iter().next().expect("a ring is drawn");
        assert_eq!(ring.border_width(), 2.0, "the keyboard ring is 2pt");
        let root = graph.tree.borrow().root().expect("root");
        assert_eq!(
            ring.border_color(),
            Some(theme_for_node(&root).colors.focus_ring),
            "the ring uses the themed focus colour"
        );

        // Same node, pointer-acquired focus: routing stays, the ring goes.
        chain.focus(Some(target), false);
        graph.recomposer.commit_all();
        graph.compute_layout(320.0, 200.0);
        assert_eq!(
            chain.focused().as_ref(),
            Some(target),
            "the pointer keeps the same focused target"
        );
        assert!(rings(graph).is_empty(), "the ring is dropped on click");
    });
}

#[test]
fn blur_clears_ring() {
    with_scene(styled(), |graph, chain, target| {
        chain.focus(Some(target), true);
        graph.recomposer.commit_all();
        graph.compute_layout(320.0, 200.0);
        assert!(!rings(graph).is_empty(), "keyboard focus draws a ring");

        chain.focus(None, false);
        graph.recomposer.commit_all();
        graph.compute_layout(320.0, 200.0);
        assert!(rings(graph).is_empty(), "losing focus clears the ring");
    });
}

#[test]
fn custom_drawn_controls_follow_focus_visibility() {
    use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
    use cove_ui_compose::BoxView;
    use cove_ui_compose::modifiers::layout::LayoutFrameExt;

    let column = AnyView::new(AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(
                    AnyView::new(Toggle::new(cove_core::Binding::constant(false)))
                        .debug_name("ring-toggle"),
                ),
                AnyView::new(
                    AnyView::new(
                        AnyView::new(Slider::new(cove_core::Binding::constant(0.4)))
                            .frame_width(200.0),
                    )
                    .debug_name("ring-slider"),
                ),
                AnyView::new(
                    AnyView::new(
                        AnyView::new(
                            RangeSlider::new(cove_core::Binding::constant(RangeSliderValue::new(
                                20.0, 80.0,
                            )))
                            .range((0.0, 100.0)),
                        )
                        .frame_width(200.0),
                    )
                    .debug_name("ring-range"),
                ),
            ])),
        )
        .frame_size(320.0, 260.0),
    ));

    with_scene(column, |graph, chain, _| {
        let theme = {
            let root = graph.tree.borrow().root().expect("root");
            theme_for_node(&root)
        };
        for name in ["ring-toggle", "ring-slider", "ring-range"] {
            let root = graph.tree.borrow().root().expect("root");
            let target = nodes(&root)
                .into_iter()
                .find(|node| support::debug_name(node).as_deref() == Some(name))
                .unwrap_or_else(|| panic!("{name} mounts"));

            chain.focus(Some(&target), false);
            assert_eq!(
                focus_ring_vertices(&target, theme.colors.focus_ring),
                0,
                "{name} drew a pointer focus ring"
            );

            chain.focus(Some(&target), true);
            assert!(
                focus_ring_vertices(&target, theme.colors.focus_ring) > 0,
                "{name} did not draw a keyboard focus ring"
            );
        }
    });
}

/// Counts the focus-ring-coloured vertices the node paints.
fn focus_ring_vertices(node: &Node, ring: cove_core::Color) -> usize {
    let Some(draw) = node.draw() else {
        return 0;
    };
    let mut list = cove_core::DrawList::default();
    draw(&mut list, node.absolute_origin());
    list.vertices()
        .iter()
        .filter(|vertex| vertex.color == ring.rgba8())
        .count()
}
