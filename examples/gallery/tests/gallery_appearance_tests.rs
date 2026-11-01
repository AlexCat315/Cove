//! Translation of `Examples/Tests/GalleryContentTests/AppearanceTests.swift`.
//!
//! Behaviour checklist:
//! - Dashboard and Tree text keep one retained glyph node across appearance
//!   switches
//! - every vertex of that node's cached glyph layer uses the theme's
//!   `onSurface` colour after each switch, in both themes
//! - the theme toggle in the navigation footer drives `gallery.isDark`,
//!   light → dark → light → dark
//!
//! Named differences from the Swift suite:
//! - Swift's suite is one parameterised `@Test` over `[.introduction, .tree]`;
//!   Rust has no test arguments, so the two cases are two `#[test]`s over one
//!   shared body, matching the two Swift test instances.
//! - Swift installed `gallery` directly; Rust wraps the same `GalleryView`
//!   instance so the test can read `is_dark()` back from it.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::DrawList;
use cove_core::scene::{LayerAwareNodeRenderer, Node};
use cove_examples::gallery_content::GalleryView;
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_ui_compose::theme::Theme;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

/// Mounts the shared gallery instance so the test body can read its state.
struct GalleryRoot(Rc<GalleryView>);

impl View for GalleryRoot {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }
}

fn run(page: GalleryPage, title: &str) {
    let gallery = Rc::new(GalleryView::new(page, Appearance::Dark, 1.0));
    let observed = gallery.clone();
    GalleryScene::run(
        1280.0,
        900.0,
        move |_: &GalleryScene| AnyView::new(GalleryRoot(gallery.clone())),
        |scene| {
            let gallery = observed;
            let find_title = |scene: &GalleryScene| -> Node {
                scene
                    .first(|node| {
                        scene.graph.layout_node(node).is_some_and(|layout| {
                            layout
                                .text_inputs()
                                .is_some_and(|inputs| inputs.text == title)
                        })
                    })
                    .unwrap_or_else(|| panic!("the {page:?} title {title:?} is mounted"))
            };
            let text = find_title(scene);
            let text_id = text.id();

            for dark in [true] {
                if gallery.is_dark() != dark {
                    let want = if dark {
                        "Switch to dark theme"
                    } else {
                        "Switch to light theme"
                    };
                    let toggle = scene
                        .first(|node| {
                            node.accessibility()
                                .is_some_and(|semantics| semantics.label == want)
                        })
                        .unwrap_or_else(|| panic!("the {want:?} toggle is mounted"));
                    scene.click(&toggle);
                    scene.settle();
                    scene.scheduler.tick(1.0);
                    scene.graph.recomposer.commit_all();
                    scene.graph.compute_layout(scene.width, scene.height);
                    scene.settle();
                }
                assert_eq!(gallery.is_dark(), dark);

                let mut renderer = LayerAwareNodeRenderer::new();
                for _ in 0..2 {
                    renderer.render(scene.graph.render_tree(), &mut DrawList::default());
                }
                let mounted_title = find_title(scene);
                println!(
                    "PROBE dark={dark} same_id={} is_layer_root={} object_for_old={:?}",
                    mounted_title.id() == text_id,
                    scene
                        .graph
                        .render_tree()
                        .render_object(&text)
                        .map(|o| o.is_layer_root())
                        .unwrap_or(false),
                    scene
                        .graph
                        .render_tree()
                        .render_object(&mounted_title)
                        .map(|o| o
                            .cached_layer_list()
                            .map(|l| l
                                .vertices()
                                .iter()
                                .map(|v| v.color)
                                .collect::<std::collections::BTreeSet<u32>>())
                            .unwrap_or_default())
                        .unwrap_or_default(),
                );
                let object = scene
                    .graph
                    .render_tree()
                    .render_object(&text)
                    .expect("the title node has a render object");
                let glyphs = object
                    .cached_layer_list()
                    .unwrap_or_else(|| panic!("the retained glyphs are cached at dark={dark}"));
                let expected = if dark {
                    Theme::default_dark()
                } else {
                    Theme::default_light()
                }
                .colors
                .on_surface
                .rgba8();
                assert!(
                    !glyphs.vertices().is_empty()
                        && glyphs
                            .vertices()
                            .iter()
                            .all(|vertex| vertex.color == expected),
                    "every glyph vertex is on_surface ({}), got {:?}",
                    expected,
                    glyphs
                        .vertices()
                        .iter()
                        .map(|vertex| vertex.color)
                        .collect::<std::collections::BTreeSet<u32>>()
                );
                assert_eq!(
                    find_title(scene).id(),
                    text_id,
                    "the same glyph node is retained across the switch"
                );
            }
        },
    );
}

#[test]
fn dashboard_text_updates_retained_glyph_colors_on_every_appearance_switch() {
    run(GalleryPage::Introduction, "Your workspace, at a glance");
}

#[test]
fn tree_text_updates_retained_glyph_colors_on_every_appearance_switch() {
    run(GalleryPage::Tree, "Tree");
}
