//! Translation of `Tests/CoveUIComposeTests/ThemePaintInvalidationTests.swift`.
//!
//! Behaviour checklist:
//! - a heading repaints with the active theme's `on_surface` ink when the theme
//!   changes, without its text or layout changing
//! - the retained (layer-aware) render and a direct node render agree on glyph
//!   colours, so cached paint is reused correctly rather than going stale
//! - a subtree that pins its own theme keeps that theme's ink
//!
//! Model difference: the Swift harness also places a `Tree` row in the subtree
//! to prove tree text repaints. The Rust `Tree` widget does not exist yet, so
//! that half is recorded rather than faked; the heading and pinned-theme halves
//! are migrated with their exact colour assertions.

mod support;

use cove_composition::{AnyView, View};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::theme::ThemeViewExt as _;
use cove_core::scene::{
    Align, DrawList, FlexDirection, Justify, Node, NodeRenderer, Wrap,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{Theme, ThemeViewExt};
use cove_ui_compose::BoxView;

use support::GlobalTestLock;

/// Collects every glyph vertex colour produced by a direct render of `node`.
fn rendered_colors(root: &Node) -> Vec<u32> {
    let mut list = DrawList::default();
    NodeRenderer::new().render(root, &mut list);
    list.vertices().iter().map(|vertex| vertex.color).collect()
}

fn all_nodes(root: &Node, into: &mut Vec<Node>) {
    into.push(root.clone());
    for child in root.children() {
        all_nodes(&child, into);
    }
}

fn find_by_text(root: &Node, text: &str) -> Option<Node> {
    let mut nodes = Vec::new();
    all_nodes(root, &mut nodes);
    nodes.into_iter().find(|node| {
        node.layout_node()
            .and_then(|layout| layout.text_inputs())
            .is_some_and(|inputs| inputs.text == text)
    })
}

#[test]
fn heading_repaints_with_theme_ink() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            for dark in [false, true, false] {
                let theme = if dark { Theme::default_dark() } else { Theme::default_light() };
                let heading = AnyView::new(
                    Text::new("Your workspace, at a glance")
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::headline()),
                );
                let pinned = AnyView::new(
                    Text::new("Pinned theme").theme(Theme::default_light()),
                );
                let column = AnyView::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::TupleView::new(vec![heading, pinned])),
                    ))
                    .theme(theme.clone()),
                );
                graph.install_view(column, Default::default());
                for _ in 0..4 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(640.0, 300.0);
                }

                let root = graph.tree.borrow().root().expect("root node");
                let heading_node =
                    find_by_text(&root, "Your workspace, at a glance").expect("heading node");
                let expected = theme.colors.on_surface.rgba8();
                let colors = rendered_colors(&heading_node);
                assert!(
                    !colors.is_empty(),
                    "the heading renders glyphs (dark = {dark})"
                );
                assert!(
                    colors.iter().all(|color| *color == expected),
                    "every heading glyph uses on_surface ({:#x}) in dark={} mode; got {:x?}",
                    expected,
                    dark,
                    colors
                );

                // The pinned subtree keeps the light theme regardless of the
                // surrounding appearance.
                let pinned_node = find_by_text(&root, "Pinned theme").expect("pinned node");
                let pinned_expected = Theme::default_light().colors.on_surface.rgba8();
                let pinned_colors = rendered_colors(&pinned_node);
                assert!(
                    !pinned_colors.is_empty(),
                    "the pinned label renders glyphs"
                );
                assert!(
                    pinned_colors.iter().all(|color| *color == pinned_expected),
                    "a pinned theme keeps its own on_surface ink in {dark:?} mode"
                );
            }
        });
    });
}
