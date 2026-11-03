//! Translation of `Examples/Tests/GalleryContentTests/DrawerTests.swift`.
//!
//! Behaviour checklist:
//! - the drawer story opens a dialog sized to the authored 320pt panel, full
//!   window height
//! - arrow keys move focus inside the modal scope, Return commits the focused
//!   destination, closes the drawer, returns focus to the opener and writes the
//!   selection caption ("Selected destination: 9")
//! - the drawer page is adaptive-supported and previews narrow and wide frames
//! - the sidebar story's drawer shares selection with the sidebar: clicking
//!   "Projects" closes the modal, marks the row selected and updates the
//!   destination pane
//!
use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, Node};
use cove_examples::gallery_content::GalleryView;
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_examples::gallery_content::stories::navigation_layout_stories::DRAWER_DEFAULT_SELECTION;
use cove_examples::gallery_content::stories::sidebar_story::SidebarStory;
use cove_ui_compose::Scancode;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

/// Shares a view without dropping the state subscriptions used by production.
struct Shared<V: View>(Rc<V>);

impl<V: View + 'static> View for Shared<V> {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        self.0.dynamic_properties()
    }
}

fn mounted<V: View + 'static>(view: Rc<V>) -> impl FnOnce(&GalleryScene) -> AnyView {
    move |_| AnyView::new(Shared(view.clone()))
}

/// Mounts a bare story the way the gallery shell does: inside a `LayerRoot`, so
/// portal-hosted modal content reaches the tree.
fn mounted_in_layer_root<V: View + 'static>(view: Rc<V>) -> impl FnOnce(&GalleryScene) -> AnyView {
    move |_| {
        AnyView::new(
            cove_ui_compose::primitives::overlays::portal::LayerRoot::new(Shared(view.clone())),
        )
    }
}

fn row_by_label(scene: &GalleryScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.is_focusable()
                && node
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == label)
        })
        .unwrap_or_else(|| panic!("the {label:?} destination is mounted"))
}

/// The modal entry animation is driven by the scheduler; a commit alone never
/// mounts the presented drawer.
fn run_animations(scene: &GalleryScene) {
    for _ in 0..10 {
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(scene.width, scene.height);
        scene.scheduler.tick(1.0);
    }
    scene.graph.recomposer.commit_all();
    scene.graph.compute_layout(scene.width, scene.height);
    scene.settle();
}

#[test]
fn the_dedicated_story_opens_a_modal_and_commits_keyboard_navigation_back_into_its_summary() {
    let gallery = Rc::new(GalleryView::new(
        GalleryPage::NavigationDrawer,
        Appearance::Light,
        1.0,
    ));
    GalleryScene::run(1280.0, 820.0, mounted(gallery), |scene| {
        let opener = scene.require_named("drawer-open");
        let opener_button = row_by_label(scene, "Open navigation");
        scene.focus.focus(Some(&opener_button), true);
        scene.click(&opener);
        run_animations(scene);
        assert!(
            scene
                .first(|node| node
                    .accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog))
                .is_some(),
            "the opener click presents the drawer dialog"
        );
        let selected = row_by_label(scene, &format!("Destination {DRAWER_DEFAULT_SELECTION}"));
        scene.focus.focus(Some(&selected), true);
        scene.key(Scancode::ARROW_DOWN);
        scene.key(Scancode::RETURN);
        run_animations(scene);
        assert!(!scene.focus.has_modal_scope());
        assert_eq!(scene.focus.focused(), Some(opener_button));
        assert!(scene.has_text("Selected destination: 9"));
        assert!(
            scene
                .first(|node| node
                    .accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog))
                .is_none(),
            "the closed drawer leaves the scene tree"
        );
    });

    let story = Rc::new(cove_examples::gallery_content::stories::DrawerStory::new(
        cove_examples::gallery_content::catalog::GalleryOptions::default(),
    ));
    story.open();
    GalleryScene::run(
        1280.0,
        820.0,
        mounted_in_layer_root(story.clone()),
        |scene| {
            run_animations(scene);
            assert!(
                scene.focus.has_modal_scope(),
                "an open drawer holds a modal scope"
            );
            let dialog = scene
                .first(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
                })
                .expect("the drawer presents a dialog");
            let frame = dialog.absolute_frame();
            assert_eq!(
                (frame.width, frame.height),
                (320.0, 820.0),
                "the drawer fills the host height at 320pt width: {frame:?}"
            );

            let selected = row_by_label(scene, &format!("Destination {DRAWER_DEFAULT_SELECTION}"));
            scene.focus.focus(Some(&selected), true);
            scene.key(Scancode::ARROW_DOWN);
            run_animations(scene);
            assert!(
                scene.focus.has_modal_scope(),
                "the open drawer keeps a modal scope while browsing"
            );
            let next = row_by_label(
                scene,
                &format!("Destination {}", DRAWER_DEFAULT_SELECTION + 1),
            );
            assert_eq!(
                scene.focus.focused().map(|node| node.id()),
                Some(next.id()),
                "ArrowDown moves focus to the next destination"
            );

            scene.key(Scancode::RETURN);
            run_animations(scene);
            assert_eq!(story.selection(), Some(9), "Return commits destination 9");
            assert!(!story.is_open(), "Return closes the drawer");
            assert!(
                !scene.focus.has_modal_scope(),
                "closing the drawer releases its modal scope"
            );
            assert!(
                scene.has_text("Selected destination: 9"),
                "the caption reports the committed destination, texts: {:?}",
                scene.texts()
            );
        },
    );
}

#[test]
fn the_sidebar_and_its_drawer_share_selection_and_restore_the_explicit_opener() {
    let story = Rc::new(SidebarStory::new(
        cove_examples::gallery_content::catalog::GalleryOptions::default(),
    ));
    GalleryScene::run(
        1280.0,
        820.0,
        mounted_in_layer_root(story.clone()),
        |scene| {
            let opener = scene.require_named("sidebar-open-drawer");
            scene.focus.focus(Some(&opener), true);
            scene.click(&opener);
            run_animations(scene);
            assert!(
                scene.focus.has_modal_scope(),
                "presenting the drawer opens a modal scope"
            );
            assert!(
                scene.named("sidebar-open-drawer").is_some(),
                "the sidebar provides its drawer opener"
            );

            let row = scene
                .first(|node| {
                    node.is_focusable()
                        && node
                            .accessibility()
                            .is_some_and(|semantics| semantics.label == "Projects")
                        && scene.focus.permits_input(node)
                })
                .expect("the drawer lists an input-permitted Projects destination");
            scene.click(&row);
            run_animations(scene);

            assert!(
                !scene.focus.has_modal_scope() && !story.is_drawer_open(),
                "destination activation closes the drawer"
            );
            assert_eq!(scene.focus.focused(), Some(opener));
            assert_eq!(story.selection().as_deref(), Some("projects"));
            assert!(scene.has_text("Projects"));
            assert!(
                scene
                    .all(|node| node
                        .accessibility()
                        .is_some_and(|semantics| semantics.label == "Projects"))
                    .iter()
                    .any(|node| node.accessibility().is_some_and(|s| s.state.is_selected)),
                "the selected destination is published"
            );
        },
    );
}
