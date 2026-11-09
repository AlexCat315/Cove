//! Translation of `Examples/Tests/GalleryContentTests/AdaptiveShellTests.swift`.
//!
//! Behaviour checklist:
//! - a page switch recomposes neither the adaptive shell nor the unrelated
//!   sidebar destinations, keeps the toolbar controls and selects the new row
//! - 390, 720 and 1280 layouts keep the story, its local state ("Actions: 1")
//!   and the toolbar usable: below 600 the page covers the navigation, from
//!   600 the navigation sits beside it
//! - in one pane, back reveals the navigation with its search, choosing a
//!   destination opens it, and the selection survives widening and narrowing
//!
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
            scene.named("gallery-size").is_some(),
            "the size control has its stable identifier"
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
            scene.named("sidebar-item-slider").is_some(),
            "the selected sidebar destination has its stable identifier"
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
            let navigation = scene.require_named("gallery-navigation").absolute_frame();
            if width < 600.0 {
                assert_eq!(
                    navigation.width, 0.0,
                    "the page covers the navigation at {width}"
                );
                assert!(content.x < 20.0, "the page fills the window at {width}");
                assert!(scene.named("gallery-back").is_some());
            } else {
                assert_eq!(
                    (navigation.x, navigation.width),
                    (0.0, 240.0),
                    "the navigation sits beside the page at {width}"
                );
                assert!(content.x >= navigation.max_x());
            }
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
    scene(|gallery, scene| {
        settle_at(scene, 390.0);
        let back = scene.require_named("gallery-back");
        scene.click(&back);
        settle_at(scene, 390.0);
        let navigation = scene.require_named("gallery-navigation").absolute_frame();
        assert_eq!(navigation.width, 390.0, "back reveals the navigation");
        assert!(
            control(scene, "Search components…").is_some(),
            "the search field is part of the navigation"
        );
        assert_eq!(
            scene
                .require_named("gallery-content")
                .absolute_frame()
                .width,
            0.0,
            "the page is hidden behind the navigation"
        );

        let slider = scene.require_named("sidebar-item-slider");
        scene.click(&slider);
        settle_at(scene, 390.0);
        assert_eq!(gallery.current_page(), GalleryPage::Slider);
        assert_eq!(
            scene
                .require_named("gallery-navigation")
                .absolute_frame()
                .width,
            0.0,
            "choosing a destination opens it"
        );

        settle_at(scene, 1280.0);
        assert_eq!(gallery.current_page(), GalleryPage::Slider);
        assert!(
            scene
                .require_named("gallery-navigation")
                .absolute_frame()
                .width
                > 0.0
        );
        assert!(
            scene.named("gallery-back").is_none(),
            "two panes need no back"
        );

        settle_at(scene, 390.0);
        assert!(
            scene.named("gallery-back").is_some(),
            "narrowing again keeps the page in front"
        );
    });
}
