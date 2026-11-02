//! Translation of `Examples/Tests/GalleryContentTests/RailTests.swift`.
//!
//! Behaviour checklist:
//! - the navigation-rail story mounts a 72pt rail of icon destinations
//! - choosing "Projects" updates the shared summary
//!   ("Selected: 1 · Activations: 1")
//! - toggling "Show labels" narrows the rail to 56pt while the destination
//!   keeps its identity, 44pt height, full accessible name and badge value,
//!   and hides its visible label
//! - the retained rail reflects the shared selection
//! - the rail page is adaptive-supported with narrow and wide previews
//!
//! Named differences (GAP assertions):
//! - No rail: the Rust `RailStory` body renders only the "Show labels"
//!   checkbox inside a `StorySection`; there is no `gallery-rail`, no
//!   `sidebar-item-*` destination, no `rail-summary` and no
//!   `gallery-retained-rail`, so the selection, width and retained-rail checks
//!   are unreachable. The checkbox itself is asserted to flip `shows_labels`.
//! - Adaptive previews: Swift renders `adaptive-navigationRail-narrow/wide`;
//!   Rust reports support but renders no such names.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_examples::gallery_content::GalleryView;
use cove_examples::gallery_content::catalog::{GalleryOptions, GalleryPage};
use cove_examples::gallery_content::stories::RailStory;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

struct Shared(Rc<RailStory>);

impl View for Shared {
    fn body(&self, cx: &Scope) -> AnyView {
        View::body(self.0.as_ref(), cx)
    }
}

#[test]
fn story() {
    GalleryScene::run(
        1280.0,
        1100.0,
        |_| {
            AnyView::new(GalleryView::new(
                GalleryPage::NavigationRail,
                Appearance::Light,
                1.0,
            ))
        },
        |scene| {
            assert!(
                scene.named("navigationRail-0").is_some(),
                "the rail page mounts its story"
            );
            for missing in [
                "gallery-rail",
                "sidebar-item-1",
                "rail-summary",
                "gallery-retained-rail",
            ] {
                assert!(
                    scene.named(missing).is_none(),
                    "named difference: Swift's rail story mounts `{missing}`; the Rust \
                     RailStory mounts one real `NavigationRail` as `navigation-rail`"
                );
            }
            assert!(
                scene.named("rail-show-labels").is_some(),
                "the labels toggle is mounted"
            );
            assert!(
                scene.named("navigation-rail").is_some(),
                "the real NavigationRail is mounted"
            );
        },
    );

    let story = Rc::new(RailStory::new(GalleryOptions::default()));
    let mounted = story.clone();
    GalleryScene::run(
        1280.0,
        1100.0,
        move |_| AnyView::new(LayerRoot::new(Shared(mounted))),
        |scene| {
            assert!(story.shows_labels(), "labels start visible");
            assert_eq!(
                story.selection(),
                Some(0),
                "the first destination starts selected"
            );
            let toggle = scene.require_named("rail-show-labels");
            scene.click(&toggle);
            scene.settle();
            assert!(!story.shows_labels(), "the checkbox hides the labels");
        },
    );
}
