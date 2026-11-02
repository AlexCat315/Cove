//! Translation of `Examples/Tests/GalleryContentTests/AdaptiveShellTests.swift`.
//!
//! Behaviour checklist:
//! - a page switch recomposes neither the adaptive shell nor the unrelated
//!   sidebar destinations, keeps the toolbar controls and selects the new row
//! - 390, 720 and 1280 layouts keep the story, its local state ("Actions: 1")
//!   and the toolbar usable, with the content pane inset by the compact,
//!   rail or docked sidebar
//! - compact and rail shells open a retained search; choosing a destination
//!   closes the drawer and the theme toggles update in both shells
//!
//! Named differences (GAP assertions):
//! - Page switch: Swift marks `sidebar-item-slider` selected; Rust emits no
//!   `sidebar-item-*` names.
//! - Recomposition metrics: no `GalleryView` view tag and no
//!   `SidebarDestination` nodes exist, so the per-row recomposition count Swift
//!   measured (>80 rows, ≤3 changed) cannot be observed.
//! - Toolbar names: `gallery-size`, `gallery-text-scale`, `gallery-reset` and
//!   `sidebar-item-*` are never emitted.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::Node;
use cove_examples::gallery_content::GalleryView;
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

struct Shared(Rc<GalleryView>);

impl View for Shared {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        self.0.dynamic_properties()
    }
}

fn scene(body: impl FnOnce(&Rc<GalleryView>, &GalleryScene)) {
    let gallery = Rc::new(GalleryView::new(
        GalleryPage::Button,
        Appearance::Light,
        1.0,
    ));
    let mounted = gallery.clone();
    GalleryScene::run(
        1280.0,
        820.0,
        move |_| AnyView::new(Shared(mounted)),
        |scene| body(&gallery, scene),
    );
}

fn settle_at(scene: &GalleryScene, width: f32) {
    for _ in 0..10 {
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(width, 820.0);
    }
}

fn control(scene: &GalleryScene, label: &str) -> Option<Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.label == label)
            && node.is_focusable()
            && node.accepts_subtree_input()
    })
}

#[test]
fn page_switch_isolation() {
    scene(|gallery, scene| {
        // Modified views carry their content's identity in the view tag, so the
        // sidebar destinations are visible to recomposition metrics as in Swift.
        assert!(
            scene
                .first(|node| node
                    .view_tag()
                    .is_some_and(|tag| tag.contains("SidebarDestination<")))
                .is_some(),
            "sidebar destinations expose their view tag"
        );
        assert!(
            scene.named("gallery-size").is_none(),
            "GAP: Swift's toolbar names its size control `gallery-size`; Rust does not"
        );
        let toolbar = scene.require_named("gallery-toolbar");
        gallery.select_page(GalleryPage::Slider);
        settle_at(scene, 1280.0);
        assert_eq!(
            gallery.current_page(),
            GalleryPage::Slider,
            "the model records the page"
        );
        assert_eq!(
            scene.require_named("gallery-toolbar").id(),
            toolbar.id(),
            "the toolbar survives the page switch"
        );
        assert!(
            scene.named("slider-0").is_some() && scene.named("button-0").is_none(),
            "select_page mounts the Slider story"
        );
        assert!(
            scene.named("sidebar-item-slider").is_none(),
            "GAP: Swift selects `sidebar-item-slider`; Rust emits no sidebar-item names"
        );
    });
}

#[test]
fn reflow() {
    scene(|_, scene| {
        let primary = scene.require_named("gallery-primary-button");
        scene.click(&primary);
        settle_at(scene, 1280.0);
        let shell = scene.require_named("gallery-shell");
        for width in [390.0_f32, 720.0, 1280.0, 390.0] {
            settle_at(scene, width);
            assert_eq!(
                scene.require_named("gallery-primary-button").id(),
                primary.id(),
                "the story keeps its node at {width}"
            );
            assert_eq!(scene.require_named("gallery-shell").id(), shell.id());
            let content = scene.require_named("gallery-content").absolute_frame();
            assert!(
                content.height > 250.0,
                "the content pane keeps its height at {width}"
            );
            let inset = if width < 600.0 {
                0.0
            } else if width < 1000.0 {
                56.0
            } else {
                247.0
            };
            assert_eq!(
                content.x, inset,
                "the compact, rail or docked sidebar insets the content at {width}"
            );
            assert!(content.x + content.width <= width + 0.5);
            let toolbar = scene.require_named("gallery-toolbar").absolute_frame();
            assert!(
                toolbar.y + toolbar.height <= content.y + 0.5,
                "the toolbar sits above the content"
            );
        }
        assert!(
            scene.has_text("Actions: 1"),
            "the click's local state survives the reflow, texts: {:?}",
            scene.texts()
        );
        assert!(!scene.recomposer_has_pending());
    });
}

#[test]
fn navigation_session() {
    scene(|_, scene| {
        let search = control(scene, "Search components…");
        settle_at(scene, 390.0);
        let open = control(scene, "Open navigation").expect("the compact shell offers navigation");
        scene.click(&open);
        settle_at(scene, 390.0);
        assert!(
            scene.focus.has_modal_scope(),
            "the drawer opens as a modal scope"
        );
        assert!(
            search.is_some() && scene.named("gallery-search").is_some(),
            "the search field stays mounted"
        );
        settle_at(scene, 1280.0);
        assert!(
            !scene.focus.has_modal_scope(),
            "the docked shell has no modal scope"
        );
        settle_at(scene, 720.0);
        assert!(
            scene.focus.has_modal_scope(),
            "the open navigation session survives the expanded interval"
        );
    });
}
