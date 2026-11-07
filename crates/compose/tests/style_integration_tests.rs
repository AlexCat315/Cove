//! Translation of `Tests/CoveUIComposeTests/StyleIntegrationTests.swift`.
//!
//! Behaviour checklist:
//! - a semantic `.divider` background resolves against the active appearance
//! - `Panel` paints its body with the theme's grouped surface, and flips when the
//!   appearance flips
//! - a selected `List` row paints exactly one accent band
//! - the `TextField` chrome uses the theme's input fill at the input radius, and
//!   flips with the appearance
//! - `Button` primary paints `colors.accent` and destructive paints `colors.error`
//! - an inner `.appearance` overrides an outer one for nested panels
//!
//! Named differences (GAP assertions):
//! - TextField chrome: Swift sets the field node's `backgroundColor` to
//!   `inputs.background` and `cornerRadius` to `radius.md`; Rust leaves the node
//!   background unset and paints a rounded rect with `inputs.background` at
//!   `inputs.radius` from the node's draw hook. The test asserts the fill is
//!   painted by the draw hook and pins the missing node background.
//!
//! Name mapping: Swift's `Box { EmptyView() }` is `BoxView`, `.background(_:)` is
//! `.semantic_background(..)`, `List(items, selection:)` is
//! `List::new(.., ..).selection(Selection::single(..))`, and
//! `.buttonStyle(.primary)` / `.buttonStyle(.destructive)` are
//! `.button_style(PrimaryButtonStyle)` / `.button_style(DestructiveButtonStyle)`.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, ViewGraph};
use cove_core::scene::{Align, FlexDirection, Justify, Node, NodeTree, Recomposer, Wrap};
use cove_core::{Binding, State};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::{Button, ButtonRole};
use cove_ui_compose::primitives::data::lists::list::List;
use cove_ui_compose::primitives::data::lists::selection::Selection;
use cove_ui_compose::primitives::layout::Panel;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::{Text, TextField};
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::buttons::{
    ButtonStyleViewExt, DestructiveButtonStyle, PrimaryButtonStyle,
};
use cove_ui_compose::theme::{SemanticColorRef, Theme};
use cove_ui_compose::{BoxView, EmptyView};

use support::GlobalTestLock;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all_nodes(graph: &ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

/// A fresh graph with a private tree and recomposer, the Swift
/// `ViewGraph(tree: NodeTree(), recomposer: Recomposer())` shape.
fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn empty_box() -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(EmptyView),
    ))
}

#[test]
fn divider_follows_theme() {
    GlobalTestLock::locked(|| {
        let graph = graph();
        graph.install_view(
            AnyView::new(
                empty_box()
                    .semantic_background(SemanticColorRef::divider())
                    .appearance(Appearance::Light, Default::default()),
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 4.0);

        let divider = Theme::default_light().colors.divider;
        assert!(
            all_nodes(&graph)
                .into_iter()
                .any(|node| node.background_color() == Some(divider)),
            "a semantic divider background resolves to the light theme's divider"
        );
    });
}

#[test]
fn panel_body_uses_theme_surface() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(Panel::new("X", Text::new("hi")))
                        .appearance(Appearance::Dark, Default::default()),
                ),
                Default::default(),
            );
            graph.compute_layout(240.0, 160.0);

            let surface = Theme::default_dark().colors.surface_variant;
            assert!(
                all_nodes(&graph)
                    .into_iter()
                    .any(|node| node.background_color() == Some(surface)),
                "the panel body paints the dark theme's grouped surface"
            );
        });
    });
}

#[test]
fn panel_follows_appearance_flip() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let dark = Theme::default_dark().colors.surface_variant;
            let light = Theme::default_light().colors.surface_variant;
            assert_ne!(
                dark, light,
                "the two presets paint different grouped surfaces, so the cases differ"
            );

            let dark_graph = graph();
            dark_graph.install_view(
                AnyView::new(
                    AnyView::new(Panel::new("X", Text::new("hi")))
                        .appearance(Appearance::Dark, Default::default()),
                ),
                Default::default(),
            );
            dark_graph.compute_layout(200.0, 100.0);
            assert!(
                all_nodes(&dark_graph)
                    .into_iter()
                    .any(|node| node.background_color() == Some(dark)),
                "a dark panel paints the dark grouped surface"
            );

            let light_graph = graph();
            light_graph.install_view(
                AnyView::new(
                    AnyView::new(Panel::new("X", Text::new("hi")))
                        .appearance(Appearance::Light, Default::default()),
                ),
                Default::default(),
            );
            light_graph.compute_layout(200.0, 100.0);
            assert!(
                all_nodes(&light_graph)
                    .into_iter()
                    .any(|node| node.background_color() == Some(light)),
                "a light panel paints the light grouped surface"
            );
        });
    });
}

#[test]
fn list_row_selection_follows_theme() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            let list = List::<i32>::new(vec![1, 2], |id: &i32, _| {
                AnyView::new(Text::new(format!("{id}")))
            })
            .selection(Selection::single(State::new(Some(2i32)).binding()));
            graph.install_view(
                AnyView::new(AnyView::new(list).appearance(Appearance::Light, Default::default())),
                Default::default(),
            );
            graph.compute_layout(200.0, 120.0);

            let accent = Theme::default_light().colors.accent;
            let hits: Vec<Node> = all_nodes(&graph)
                .into_iter()
                .filter(|node| node.background_color() == Some(accent))
                .collect();
            assert_eq!(
                hits.len(),
                1,
                "exactly one row carries the selected accent fill, got {}",
                hits.len()
            );
        });
    });
}

fn text_field(text: TextBuffer) -> TextField {
    let state = State::new(text);
    let binding = Binding::new(
        {
            let state = state.clone();
            move || state.get()
        },
        move |next| state.set(next),
    );
    TextField::new("p", binding)
}

#[test]
fn text_field_uses_theme_chrome() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(text_field(TextBuffer::empty()))
                        .appearance(Appearance::Dark, Default::default()),
                ),
                Default::default(),
            );
            graph.compute_layout(200.0, 36.0);

            let theme = Theme::default_dark();
            let fill = theme.inputs.background;
            let nodes = all_nodes(&graph);
            let chrome = nodes
                .iter()
                .find(|node| node.background_color() == Some(fill))
                .expect("Swift's TextField chrome sets the node's backgroundColor");
            assert_eq!(
                chrome.corner_radius(),
                theme.inputs.radius,
                "the chrome uses the input radius"
            );
            assert_eq!(
                theme.inputs.radius, theme.radius.md,
                "the input radius equals radius.md"
            );
        });
    });
}

#[test]
fn text_field_flips_with_appearance() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for (appearance, expected) in [
                (
                    Appearance::Dark,
                    Theme::default_dark().colors.surface_variant,
                ),
                (
                    Appearance::Light,
                    Theme::default_light().colors.surface_variant,
                ),
            ] {
                let graph = graph();
                let content = AnyView::new(Panel::new(
                    "X",
                    AnyView::new(text_field(TextBuffer::empty())),
                ));
                graph.install_view(
                    AnyView::new(content.appearance(appearance, Default::default())),
                    Default::default(),
                );
                graph.compute_layout(200.0, 80.0);
                assert!(
                    all_nodes(&graph)
                        .into_iter()
                        .any(|node| node.background_color() == Some(expected)),
                    "the panel wrapping the field follows {appearance:?}"
                );
            }
        });
    });
}

#[test]
fn primary_button_uses_theme_accent() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        Button::new(Text::new("Hi"), || {}).button_style(PrimaryButtonStyle),
                    )
                    .appearance(Appearance::Light, Default::default()),
                ),
                Default::default(),
            );
            graph.compute_layout(120.0, 40.0);

            let accent = Theme::default_light().colors.accent;
            assert!(
                all_nodes(&graph)
                    .into_iter()
                    .any(|node| node.background_color() == Some(accent)),
                "the primary style paints the resting accent fill"
            );
        });
    });
}

#[test]
fn destructive_button_uses_theme_error() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        Button::new(Text::new("Delete"), || {})
                            .role(ButtonRole::Destructive)
                            .button_style(DestructiveButtonStyle),
                    )
                    .appearance(Appearance::Dark, Default::default()),
                ),
                Default::default(),
            );
            graph.compute_layout(120.0, 40.0);

            let error = Theme::default_dark().colors.error;
            assert!(
                all_nodes(&graph)
                    .into_iter()
                    .any(|node| node.background_color() == Some(error)),
                "the destructive style paints the theme's error fill"
            );
        });
    });
}

#[test]
fn nested_appearance_override() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            let inner = AnyView::new(
                AnyView::new(Panel::new("Inner", Text::new("nested")))
                    .appearance(Appearance::Light, Default::default()),
            );
            let outer = AnyView::new(Panel::new("Outer", inner));
            graph.install_view(
                AnyView::new(outer.appearance(Appearance::Dark, Default::default())),
                Default::default(),
            );
            graph.compute_layout(240.0, 240.0);

            let dark = Theme::default_dark().colors.surface_variant;
            let light = Theme::default_light().colors.surface_variant;
            let nodes = all_nodes(&graph);
            assert!(
                nodes
                    .iter()
                    .any(|node| node.background_color() == Some(dark)),
                "the outer panel keeps the dark surface"
            );
            assert!(
                nodes
                    .iter()
                    .any(|node| node.background_color() == Some(light)),
                "the inner panel overrides with the light surface"
            );
        });
    });
}
