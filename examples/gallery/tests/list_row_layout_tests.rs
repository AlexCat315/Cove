//! Translation of `Examples/Tests/GalleryContentTests/ListRowLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - the `sectionedList` page mounts at every width / scale / appearance entry in
//!   the matrix without collapsing
//! - the sectioned list reports its three section headers in order
//! - no node in the mounted story has a non-finite or negative frame
//!
//! ## Named differences — neither list story is ported
//!
//! `GAP(routing.rs GalleryPage::List)`: Swift's `ListRowLayoutTests` ran the same
//! matrix over **two** pages, `GalleryPage.list` and `.sectionedList`, and both
//! branches relied on `ListStory` / `SectionedListStory` rendering real rows.
//!
//! - `GalleryPage::List` has no story in Rust at all. `routing.rs` has no
//!   `Self::Story(GalleryPage::List)` arm — the page falls through to
//!   `Self::Story(_) | Self::Pending { .. } => None` — so the Swift `ListStory`
//!   (30 selectable rows plus a `Selected` badge and two state lines, embedding
//!   `MultipleSelectionListStory`) has never been migrated. That half of the
//!   matrix therefore has no target.
//! - `SectionedListStory` exists but renders only a title, a detail line and a
//!   caption (`"{n} sections"`). It materialises **no** list rows, so the Swift
//!   assertions cannot hold even for the page that does exist:
//!     - `rows.count >= 2` filtered on `role == .listItem` — no list item is
//!       rendered, so no row carries that role;
//!     - every row's first text-bearing child has `width > 0 && height > 0` and
//!       `clipsToBounds` — there is no such child;
//!     - the `Selected` badge (`role == .staticText`, label "Selected") never
//!       exists, so the badge-vs-title overlap contract
//!       (`title.maxX <= badge.minX`), the badge height cap (`<= 20 * scale`) and
//!       the row-contains-badge containment check have no target;
//!     - the tap-a-row-and-assert-`isSelected` sequence cannot run: there is no
//!       row to hit-test and no `isSelected` state to read.
//!
//! Per the Lead's decision this file asserts the reachable surface for the one
//! page that mounts, and records the rest here rather than weakening an
//! assertion to produce a green row.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::AccessibilityRole;
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_examples::gallery_content::stories::data_stories_file::SectionedListStory;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use cove_ui_compose::theme::tokens::TextScaler;
use cove_ui_compose::theme::{TextScalerViewExt, ThemeViewExt};

use support::GalleryScene;

struct Mounted {
    story: Rc<SectionedListStory>,
    scale: f32,
    appearance: Appearance,
}

impl cove_composition::View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(StoryView(self.story.clone()))
                .text_scaler(TextScaler::new(self.scale))
                .theme(self.appearance.theme()),
        )
    }
}

struct StoryView(Rc<SectionedListStory>);

impl cove_composition::View for StoryView {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::View::dynamic_properties(self.0.as_ref())
    }
}

#[test]
fn the_sectioned_list_mounts_at_every_matrix_entry() {
    for width in [330.0f32, 600.0, 970.0] {
        for scale in [1.0f32, 2.0] {
            for appearance in [Appearance::Light, Appearance::Dark] {
                let story = Rc::new(SectionedListStory::new());
                let mounted = story.clone();
                GalleryScene::run(
                    width,
                    2000.0,
                    move |_| {
                        AnyView::new(Mounted {
                            story: mounted,
                            scale,
                            appearance,
                        })
                    },
                    |scene| {
                        scene.settle_n(8);
                        let context = format!("width={width} scale={scale} {appearance:?}");

                        for node in scene.nodes() {
                            let frame = node.frame();
                            assert!(
                                frame.width.is_finite() && frame.height.is_finite(),
                                "a node has a non-finite frame {frame:?} ({context})"
                            );
                            assert!(
                                frame.width >= 0.0 && frame.height >= 0.0,
                                "a node has a negative box {frame:?} ({context})"
                            );
                        }

                        let section = scene
                            .graph
                            .layout_snapshot()
                            .into_iter()
                            .find(|entry| {
                                entry
                                    .debug_name
                                    .as_deref()
                                    .is_some_and(|name| name.starts_with("story-section-"))
                            })
                            .unwrap_or_else(|| panic!("no story section ({context})"));
                        assert!(
                            section.frame.width > 0.0 && section.frame.height > 0.0,
                            "the section has a real box ({context})"
                        );
                        assert!(
                            scene.has_text("Sectioned list"),
                            "the story renders its title ({context})"
                        );
                    },
                );
            }
        }
    }
}

#[test]
fn the_sectioned_list_reports_its_three_headers() {
    let story = SectionedListStory::new();
    assert_eq!(
        story.sections(),
        ["Today", "Yesterday", "Earlier"],
        "the story keeps its three authored section headers in order"
    );
    assert_eq!(story.selection(), Some(0), "the first row starts selected");
}

#[test]
fn the_sectioned_list_renders_no_rows_yet() {
    // Pins the GAP above as a checked fact rather than prose: the story's whole
    // observable surface is its caption, with no list items and no badge.
    let story = Rc::new(SectionedListStory::new());
    let mounted = story.clone();
    GalleryScene::run(
        600.0,
        2000.0,
        move |_| AnyView::new(StoryView(mounted)),
        |scene| {
            scene.settle_n(8);
            assert!(
                scene.has_text("3 sections"),
                "the story renders the section count caption, texts: {:?}",
                scene.texts()
            );
            assert!(
                !scene.nodes().iter().any(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
                }),
                "GAP: the story materialises no list rows"
            );
            assert!(
                !scene.nodes().iter().any(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Selected")
                }),
                "GAP: the story renders no Selected badge"
            );
        },
    );
}

#[test]
fn the_list_page_has_no_story() {
    use cove_examples::gallery_content::StoryDispatch;
    // GAP: `GalleryPage::List` has no `view()` arm, so it resolves to the pending
    // notice. The Swift `ListStory` has never been migrated.
    let dispatch = StoryDispatch::new(GalleryPage::List);
    assert!(
        !dispatch.is_implemented(),
        "GAP: the list page still has no story"
    );
    assert!(
        dispatch.pending_message().is_some(),
        "the pending notice is produced for the list page"
    );
    assert!(dispatch.view(Default::default()).is_none());
}
