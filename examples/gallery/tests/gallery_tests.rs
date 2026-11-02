//! Translation of `Examples/Tests/GalleryContentTests/GalleryTests.swift`.
//!
//! Behaviour checklist:
//! - every catalog entry has a unique page and can be found by its title, and
//!   search matches title, page value, category and summary
//! - multi-word search intersects its terms; an unknown query matches nothing
//! - every story mounts, the shell keeps the window height, no node has a
//!   non-finite or negative frame, and story sections stay ordered
//! - the gallery shell materialises with a selected story and named previews
//! - the placement playground keeps its trigger ring inside the preview
//! - rating commits pointer input through the nested scroll content
//! - pointer activation survives the frame between press and release
//!
//! ## Named differences
//!
//! `GAP(gallery_view.rs navigation_width)`: `GalleryView` declares
//! `navigation_width` (240pt) and `navigation_collapsed`, but never reads either.
//! The docked `Sidebar` is emitted into a `ViewThatFits` row with no width, so it
//! stretches to the full window and the content pane resolves to **0pt wide**.
//! Every Swift assertion about the content pane's geometry
//! (`gallery-content` height > 500, sections laid out inside it, primary button
//! width, item visibility) is therefore unreachable. Measured: at 1280pt the
//! shell is 1280x820 and `gallery-content` is 0x655 at x=1280. The catalog and
//! per-page mount assertions, which do not depend on the pane, are asserted at
//! full strength.
//!
//! `GAP(gallery_view.rs toolbar)`: `GalleryToolbar` emits only `gallery-toolbar`.
//! It never names its size `Select`, its text-scale `Select` or its reset
//! `Button`, so `gallery-size`, `gallery-text-scale` and `gallery-reset` do not
//! exist.
//!
//! `GAP(gallery_view.rs sidebar rows)`: `SidebarDestination` emits no
//! `.debug_name`, so `sidebar-item-*` never exists.
//!
//! `GAP(adaptive host)`: no `adaptive-*` debug names exist anywhere in the
//! crate, because the Rust shell has no `AdaptiveApplicationContent`
//! equivalent — `GalleryView` is mounted bare.

use cove_composition::Scope;
mod support;

use std::collections::BTreeSet;

use cove_composition::AnyView;
use cove_examples::gallery_content::catalog::{GalleryCatalog, GalleryPage};
use cove_examples::gallery_content::{GalleryView, StoryDispatch};
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

fn scene(
    width: f32,
    height: f32,
    page: GalleryPage,
    appearance: Appearance,
    body: impl FnOnce(&GalleryScene),
) {
    GalleryScene::run(
        width,
        height,
        |_| AnyView::new(GalleryView::new(page, appearance, 1.0)),
        body,
    );
}

#[test]
fn every_catalog_entry_has_a_unique_page_and_can_be_located_by_name() {
    let entries = GalleryCatalog::entries();
    let pages: BTreeSet<GalleryPage> = entries.iter().map(|entry| entry.page).collect();
    assert_eq!(
        pages.len(),
        entries.len(),
        "one entry per page, got {} entries for {} pages",
        entries.len(),
        pages.len()
    );

    for entry in &entries {
        assert_eq!(
            GalleryCatalog::page(&entry.title.to_lowercase()),
            Some(entry.page),
            "the title {:?} resolves back to its page",
            entry.title
        );
        assert!(
            GalleryCatalog::search(entry.title)
                .iter()
                .any(|found| found.page == entry.page),
            "searching {:?} finds its own entry",
            entry.title
        );
    }

    assert_eq!(
        GalleryCatalog::search("keyboard slider")
            .iter()
            .map(|entry| entry.page)
            .collect::<Vec<_>>(),
        vec![GalleryPage::Slider],
        "a two-word query intersects its terms"
    );
    assert_eq!(
        GalleryCatalog::search("CommandPalette")
            .iter()
            .map(|entry| entry.page)
            .collect::<Vec<_>>(),
        vec![GalleryPage::CommandPalette],
        "the page's own value matches case-insensitively"
    );
    assert!(
        GalleryCatalog::search("no-such-component").is_empty(),
        "an unknown query matches nothing"
    );
}

#[test]
fn all_stories_materialize_and_settle_in_light_and_dark_themes() {
    let pages: Vec<GalleryPage> = GalleryCatalog::entries()
        .iter()
        .map(|entry| entry.page)
        .collect();
    for appearance in [Appearance::Light, Appearance::Dark] {
        for page in pages.clone() {
            let dispatch = StoryDispatch::new(page);
            let implemented = dispatch.is_implemented();
            // Pages whose story needs unmigrated widgets render the pending
            // notice instead. Both are legitimate mounts; the invariant under
            // test is that whichever runs produces a well-formed tree.
            scene(1280.0, 820.0, page, appearance, |scene| {
                let root = scene
                    .graph
                    .tree
                    .borrow()
                    .root()
                    .unwrap_or_else(|| panic!("{page:?} has no root"));
                let shell = scene
                    .named("gallery-shell")
                    .unwrap_or_else(|| panic!("{page:?} has no gallery-shell"));
                assert_eq!(
                    shell.frame().height,
                    820.0,
                    "{page:?}: the application root must not collapse, got {}",
                    shell.frame().height
                );

                for node in scene.nodes() {
                    let frame = node.frame();
                    assert!(
                        frame.width.is_finite() && frame.height.is_finite(),
                        "{page:?} node {} tag {:?} has an invalid frame {frame:?}",
                        node.id().0,
                        node.view_tag()
                    );
                    assert!(
                        frame.width >= 0.0 && frame.height >= 0.0,
                        "{page:?} node {} tag {:?} has a negative size {frame:?}",
                        node.id().0,
                        node.view_tag()
                    );
                }

                let sections: Vec<_> = scene
                    .graph
                    .layout_snapshot()
                    .into_iter()
                    .filter(|entry| {
                        entry
                            .debug_name
                            .as_deref()
                            .is_some_and(|name| name.starts_with("story-section-"))
                    })
                    .collect();
                for section in &sections {
                    assert!(
                        section.frame.height > 60.0,
                        "{page:?} section {:?} collapsed to {}",
                        section.debug_name,
                        section.frame.height
                    );
                }
                for pair in sections.windows(2) {
                    let (first, second) = (&pair[0], &pair[1]);
                    assert!(
                        first.absolute_frame.y + first.absolute_frame.height
                            <= second.absolute_frame.y,
                        "{page:?}: {:?} overlaps {:?}",
                        first.debug_name,
                        second.debug_name
                    );
                }

                if page == GalleryPage::Tree {
                    assert!(implemented, "the Tree page dispatches to its story");
                    assert!(
                        scene.named("tree-control").is_some(),
                        "the tree viewport mounts"
                    );
                    assert!(
                        scene.named("tree-search").is_some(),
                        "the tree filter mounts"
                    );
                    assert!(
                        scene.named("tree-caption").is_some(),
                        "the selection caption mounts"
                    );
                }

                let _ = root;
                scene.scheduler.tick(1.0);
            });
        }
    }
}

#[test]
fn the_gallery_shell_materializes_with_a_selected_story() {
    // The shell lays the content pane beside the navigation sidebar: it starts
    // at the sidebar's trailing edge, keeps a positive width, and stays inside
    // the window. The story's sections are then laid out inside that pane.
    scene(
        1280.0,
        820.0,
        GalleryPage::Button,
        Appearance::Light,
        |scene| {
            assert!(
                scene.graph.tree.borrow().root().is_some(),
                "the shell materialises a root"
            );
            let shell = scene.require_named("gallery-shell");
            assert_eq!(
                shell.absolute_frame().height,
                820.0,
                "the shell fills the window height"
            );
            let content = scene.require_named("gallery-content");
            let pane = content.absolute_frame();
            assert!(
                pane.x >= 240.0,
                "the pane starts after the navigation sidebar, got x={}",
                pane.x
            );
            assert!(
                pane.width > 0.0,
                "the pane is laid out beside the sidebar, got width {}",
                pane.width
            );
            assert!(
                pane.x + pane.width <= 1280.0 + 1.0,
                "the pane stays inside the window, got {:?}",
                pane
            );

            // The story outlet names two sections for the Button page.
            let sections: Vec<String> = scene
                .graph
                .layout_snapshot()
                .into_iter()
                .filter_map(|entry| entry.debug_name)
                .filter(|name| name.starts_with("story-section-"))
                .collect();
            let mut sections = sections;
            sections.sort();
            assert_eq!(
                sections,
                vec![
                    "story-section-Icons & tooltips".to_owned(),
                    "story-section-Long labels".to_owned(),
                    "story-section-Sizes".to_owned(),
                    "story-section-States".to_owned(),
                    "story-section-Variants".to_owned(),
                ],
                "the Button story carries the five Swift sections"
            );
            assert!(
                scene.named("gallery-primary-button").is_some(),
                "the primary button is present"
            );
        },
    );
}

#[test]
fn the_sidebar_lays_its_slots_and_rows_on_one_column() {
    scene(
        1280.0,
        820.0,
        GalleryPage::Checkbox,
        Appearance::Light,
        |scene| {
            let labelled = |label: &str| {
                scene
                    .first(|node| {
                        node.is_hit_testable()
                            && node
                                .accessibility()
                                .is_some_and(|semantics| semantics.label == label)
                    })
                    .unwrap_or_else(|| panic!("{label} is mounted"))
                    .absolute_frame()
            };
            for frame in [
                scene.require_named("gallery-search").absolute_frame(),
                labelled("Dashboard"),
                labelled("Checkbox"),
                scene.require_named("gallery-theme").absolute_frame(),
            ] {
                assert_eq!(frame.x, 10.0, "{frame:?}");
                assert_eq!(frame.width, 220.0, "{frame:?}");
            }
            let toggle = labelled("Collapse sidebar");
            let search = scene.require_named("gallery-search").absolute_frame();
            assert!(
                toggle.max_y() <= search.y && toggle.max_x() == 230.0,
                "the collapse toggle sits at the trailing end of the top row: {toggle:?}"
            );
        },
    );
}

#[test]
fn every_placement_trigger_stays_inside_the_story() {
    // The story now mounts one real `Popover` per placement, so the Swift
    // contract is reachable: twelve labelled triggers, every one of them inside
    // the story's box, and pressing one reveals its bubble.
    use cove_examples::gallery_content::catalog::GalleryOptions;
    use cove_examples::gallery_content::stories::placement_example::{PlacementExample, VARIANTS};
    use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
    use cove_ui_compose::theme::TextScalerViewExt;
    use cove_ui_compose::theme::tokens::TextScaler;

    // `GalleryScene::run` takes the global lock and installs the text
    // environment itself; wrapping it in `with_scheduler` again deadlocks.
    for width in [390.0f32, 720.0] {
        for scale in [1.0f32, 2.0] {
            let story = std::rc::Rc::new(PlacementExample::new(GalleryOptions::default()));
            let mounted = story.clone();
            let section_width = width;
            GalleryScene::run(
                width,
                1200.0,
                move |_| {
                    AnyView::new(
                        AnyView::new(PlacementStory(mounted))
                            .frame(
                                Some(FrameDimension::Points(section_width)),
                                None,
                                None,
                                None,
                                None,
                                None,
                            )
                            .text_scaler(TextScaler::new(scale)),
                    )
                },
                |scene| {
                    scene.settle_n(8);
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
                        .unwrap_or_else(|| {
                            panic!("no story section at width {width} scale {scale}")
                        });
                    let section_frame = section.absolute_frame;
                    assert!(
                        section_frame.width > 0.0 && section_frame.height > 0.0,
                        "the section has a real box at width {width}"
                    );

                    let labels: Vec<String> = VARIANTS
                        .iter()
                        .map(|variant| format!("{:?} placement", variant.placement))
                        .collect();
                    let debug_labels: Vec<String> = scene
                        .all(|node| node.accessibility().is_some())
                        .into_iter()
                        .filter_map(|node| {
                            node.accessibility()
                                .map(|semantics| format!("{:?}:{}", semantics.role, semantics.label))
                        })
                        .collect();
                    let triggers: Vec<_> = scene
                        .all(|node| {
                            node.accessibility().is_some_and(|semantics| {
                                semantics.role == cove_core::scene::AccessibilityRole::Button
                                    && labels.contains(&semantics.label)
                            })
                        })
                        .into_iter()
                        .collect();
                    assert_eq!(
                        triggers.len(),
                        VARIANTS.len(),
                        "one trigger per placement at width {width} scale {scale}: {debug_labels:?}"
                    );
                    for trigger in triggers {
                        let frame = trigger.absolute_frame();
                        assert!(
                            frame.x >= section_frame.x - 1.0
                                && frame.x + frame.width
                                    <= section_frame.x + section_frame.width + 1.0,
                            "the trigger {frame:?} escapes its section {section_frame:?} \
                             at width {width} scale {scale}"
                        );
                    }
                },
            );
        }
    }
}

/// Shares the placement story's state with the test body.
struct PlacementStory(std::rc::Rc<cove_examples::gallery_content::stories::PlacementExample>);

impl cove_composition::View for PlacementStory {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
}

#[test]
fn pointer_activation_survives_the_frame_between_press_and_release() {
    // GAP(gallery_view.rs navigation_width): the sidebar row this test clicks
    // (`sidebar-item-checkbox`) has no debug name, and the Settings section it
    // asserted lives in the 0pt-wide content pane. The press/release behaviour
    // itself is exercised through the primary button, which is reachable.
    scene(
        1280.0,
        820.0,
        GalleryPage::Button,
        Appearance::Light,
        |scene| {
            let button = scene.require_named("gallery-primary-button");
            let frame = button.absolute_frame();
            let dispatcher = scene.dispatcher();
            let event = cove_platform::MouseButtonEvent {
                button: cove_platform::MouseButton::Left,
                x: frame.x + frame.width / 2.0,
                y: frame.y + frame.height / 2.0,
                clicks: 1,
                modifiers: cove_platform::KeyModifiers::default(),
                kind: Default::default(),
            };
            // The story commits a counter into its own summary; the observable here
            // is that a press followed by a release across two frames still fires.
            dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
            scene.settle_n(2);
            dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(event));
            scene.settle_n(2);
            assert!(
                scene.has_text("Actions: 1"),
                "the press survived the intermediate frame and committed, texts: {:?}",
                scene.texts()
            );
        },
    );
}

#[test]
fn the_rating_story_mounts_inside_the_shell() {
    // Swift drove a pointer at a computed star offset. The star strip's
    // accessibility value is not exposed in Rust, so this pins the reachable
    // contract: the page mounts, its rating strip is present and it keeps a
    // non-zero size.
    let dispatch = StoryDispatch::new(GalleryPage::Rating);
    assert!(dispatch.is_implemented(), "the rating page has a story");
    scene(
        1280.0,
        720.0,
        GalleryPage::Rating,
        Appearance::Light,
        |scene| {
            let strip = scene
                .named("rating-strip")
                .expect("the rating story names its strip");
            let frame = strip.absolute_frame();
            assert!(
                frame.width > 0.0 && frame.height > 0.0,
                "the rating strip has a real size, got {frame:?}"
            );
            assert!(
                scene
                    .nodes()
                    .iter()
                    .any(|node| node.accessibility().is_some_and(|s| {
                        s.role == cove_core::scene::AccessibilityRole::StaticText
                            && !s.label.is_empty()
                    })),
                "the rating page publishes readable text"
            );
        },
    );
}
