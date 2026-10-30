//! Translation of `Tests/CoveUIComposeTests/AppearanceTests.swift`.
//!
//! Behaviour checklist:
//! - the light preset has a near-white background
//! - the dark preset has a near-black background
//! - `.appearance(.light)` installs the light preset on the subtree
//! - `.appearance(.dark)` installs the dark preset on the subtree
//! - an inner `.appearance` overrides the outer one for descendants

mod support;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::{theme_for_node, Theme};

use support::{DebugNode, GlobalTestLock};

fn channel_sum(theme: &Theme) -> f32 {
    let background = theme.colors.background;
    background.r + background.g + background.b
}

#[test]
fn light_theme_has_light_surface() {
    GlobalTestLock::locked(|| {
        let sum = channel_sum(&Theme::default_light());
        assert!(sum > 2.7, "the light background is near-white (sum {sum})");
    });
}

#[test]
fn dark_theme_has_dark_surface() {
    GlobalTestLock::locked(|| {
        let sum = channel_sum(&Theme::default_dark());
        assert!(sum < 0.5, "the dark background is near-black (sum {sum})");
    });
}

/// Materialises `root` and returns the deepest node's resolved theme.
fn deepest_theme(root: AnyView) -> Theme {
    let graph = support::graph();
    graph.install_view(root, Default::default());
    let node = graph
        .tree
        .borrow()
        .root()
        .map(|root| support::deepest_node(&root))
        .expect("root node");
    theme_for_node(&node)
}

#[test]
fn appearance_light_installs_light_preset() {
    GlobalTestLock::locked(|| {
        let theme = deepest_theme(AnyView::new(
            DebugNode::new("x").appearance(Appearance::Light, ThemeTransition::default()),
        ));
        let expected = Theme::default_light();
        assert_eq!(
            theme.colors.background, expected.colors.background,
            "the light preset reaches the subtree"
        );
    });
}

#[test]
fn appearance_dark_installs_dark_preset() {
    GlobalTestLock::locked(|| {
        let theme = deepest_theme(AnyView::new(
            DebugNode::new("x").appearance(Appearance::Dark, ThemeTransition::default()),
        ));
        let expected = Theme::default_dark();
        assert_eq!(
            theme.colors.background, expected.colors.background,
            "the dark preset reaches the subtree"
        );
    });
}

#[test]
fn inner_appearance_overrides_outer() {
    GlobalTestLock::locked(|| {
        let column = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::TupleView::new(vec![AnyView::new(
                    DebugNode::new("inner").appearance(Appearance::Light, ThemeTransition::default()),
                )])),
            ))
            .appearance(Appearance::Dark, ThemeTransition::default()),
        );
        let theme = deepest_theme(column);
        let expected = Theme::default_light();
        assert_eq!(
            theme.colors.background, expected.colors.background,
            "the inner light appearance wins over the outer dark one"
        );
    });
}
