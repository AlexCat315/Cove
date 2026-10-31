//! Translation of `Tests/CoveUIComposeTests/ThemeEnvironmentTests.swift`.
//!
//! Behaviour checklist:
//! - `.theme(_)` makes descendants resolve the provided theme
//! - without `.theme(_)` descendants fall back to the dark preset
//! - a semantic background resolves against the provided theme on first install
//! - a semantic foreground resolves against the provided theme
//! - a semantic font writes both the font and its line height
//! - a nearer `.theme(_)` overrides an outer one for descendants

mod support;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::Color;
use cove_core::scene::Node;
use cove_ui_compose::modifiers::style::{
    FONT as FONT_KEY, LINE_HEIGHT as LINE_HEIGHT_KEY, SemanticStyleViewExt,
};
use cove_ui_compose::theme::theme_environment::{ThemeViewExt, theme_for_node};
use cove_ui_compose::theme::{SemanticColorRef, SemanticFontRef, Theme};

use support::{DebugNode, GlobalTestLock};

fn descendant(root: &Node) -> Node {
    let mut cursor = root.clone();
    while let Some(next) = cursor.children().first().cloned() {
        cursor = next;
    }
    cursor
}

fn deepest_theme(root: AnyView) -> Theme {
    let graph = support::graph();
    graph.install_view(root, Default::default());
    let root = graph.tree.borrow().root().expect("root node");
    theme_for_node(&descendant(&root))
}

#[test]
fn theme_provision_reaches_descendants() {
    GlobalTestLock::locked(|| {
        let mut custom = Theme::default_dark();
        custom.colors.accent = Color {
            r: 0.9,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        };

        let theme = deepest_theme(AnyView::new(
            AnyView::new(DebugNode::new("leaf")).theme(custom),
        ));
        assert_eq!(
            theme.colors.accent,
            Color {
                r: 0.9,
                g: 0.1,
                b: 0.1,
                a: 1.0
            },
            "the provided theme reaches the leaf"
        );
    });
}

#[test]
fn absent_provider_falls_back_to_the_dark_preset() {
    GlobalTestLock::locked(|| {
        let theme = deepest_theme(AnyView::new(DebugNode::new("leaf")));
        assert_eq!(
            theme.colors.accent,
            Theme::default_dark().colors.accent,
            "without a provider the dark preset applies"
        );
    });
}

#[test]
fn semantic_background_resolves_against_the_provided_theme() {
    GlobalTestLock::locked(|| {
        let mut custom = Theme::default_dark();
        custom.colors.surface = Color {
            r: 0.5,
            g: 0.0,
            b: 0.5,
            a: 1.0,
        };

        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(DebugNode::new("leaf"))
                    .semantic_background(SemanticColorRef::surface())
                    .theme(custom),
            ),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root node");
        let leaf = descendant(&root);
        assert_eq!(
            leaf.background_color(),
            Some(Color {
                r: 0.5,
                g: 0.0,
                b: 0.5,
                a: 1.0
            }),
            "the semantic background resolved on first install"
        );
    });
}

#[test]
fn semantic_foreground_resolves_against_the_provided_theme() {
    GlobalTestLock::locked(|| {
        let mut custom = Theme::default_dark();
        custom.colors.on_surface = Color {
            r: 0.1,
            g: 0.9,
            b: 0.2,
            a: 1.0,
        };

        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(DebugNode::new("leaf"))
                    .semantic_foreground(SemanticColorRef::on_surface())
                    .theme(custom),
            ),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root node");
        let leaf = descendant(&root);
        assert_eq!(
            leaf.foreground_color(),
            Some(Color {
                r: 0.1,
                g: 0.9,
                b: 0.2,
                a: 1.0
            }),
            "the semantic foreground resolved on first install"
        );
    });
}

#[test]
fn semantic_font_writes_font_and_line_height() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(DebugNode::new("leaf")).semantic_font(SemanticFontRef::title()),
            ),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root node");
        let leaf = descendant(&root);

        let expected = Theme::default_dark().typography.title;
        let font = leaf.attachment::<cove_core::Font>(FONT_KEY);
        let line_height = leaf.attachment::<f32>(LINE_HEIGHT_KEY);
        assert_eq!(
            font.as_ref().map(cove_core::Font::size),
            Some(expected.font.size()),
            "the font slot is written"
        );
        assert_eq!(
            font.as_ref().map(cove_core::Font::weight),
            Some(expected.font.weight()),
            "the weight comes from the same token"
        );
        assert_eq!(
            line_height,
            Some(expected.line_height),
            "the line height travels with the font"
        );
    });
}

#[test]
fn a_nearer_theme_overrides_an_outer_one() {
    GlobalTestLock::locked(|| {
        let mut outer = Theme::default_dark();
        outer.colors.accent = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let mut inner = Theme::default_dark();
        inner.colors.accent = Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };

        // The modifier applied last is the innermost provider.
        let theme = deepest_theme(AnyView::new(
            AnyView::new(AnyView::new(DebugNode::new("leaf")).theme(inner)).theme(outer),
        ));
        assert_eq!(
            theme.colors.accent,
            Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0
            },
            "the innermost provider wins"
        );
    });
}

#[test]
fn every_semantic_color_slot_tracks_its_theme_slot() {
    GlobalTestLock::locked(|| {
        let mut custom = Theme::default_dark();
        custom.colors.error = Color {
            r: 0.2,
            g: 0.4,
            b: 0.6,
            a: 1.0,
        };

        let graph = support::graph();
        let expected = custom.colors.error;
        graph.install_view(
            AnyView::new(
                AnyView::new(DebugNode::new("leaf"))
                    .semantic_background(SemanticColorRef::error())
                    .theme(custom),
            ),
            Default::default(),
        );
        let root = graph.tree.borrow().root().expect("root node");
        let leaf = descendant(&root);
        assert_eq!(
            leaf.background_color(),
            Some(expected),
            "an overridden theme slot flows through the semantic reference"
        );
    });
}
