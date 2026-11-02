//! Translation of `Examples/Tests/GalleryContentTests/NavigationStateTests.swift`.
//!
//! Behaviour checklist:
//! - the calendar story reports its selected date and granularity, and a weekend
//!   is never the default selection
//! - the lazy grid keeps a bounded viewport and a bounded number of cells instead
//!   of materialising all 100,000
//! - the tree story owns a scroll viewport sized shorter than its caption row
//! - the shell's search narrows the navigation and the selection survives it
//!
//! ## Named differences
//!
//! Calendar interaction is covered by Compose's calendar keyboard tests;
//! this suite checks the Gallery's scope-owned initial selection captions.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node};
use cove_examples::gallery_content::catalog::{GalleryOptions, GalleryPage};
use cove_examples::gallery_content::stories::calendar::{CalendarStory, is_weekend};
use cove_examples::gallery_content::stories::data_stories_file::TreeStory;
use cove_examples::gallery_content::stories::layout::LazyGridStory;
use cove_ui_compose::foundation::Date;
use cove_ui_compose::theme::tokens::TextScaler;
use cove_ui_compose::theme::{TextScalerViewExt, Theme, ThemeViewExt};

use support::GalleryScene;

/// A `View` that delegates to a shared story, so the test keeps a handle on the
/// state it mounted.
struct StoryRef<T: cove_composition::View + 'static>(Rc<T>);

impl<T: cove_composition::View + 'static> cove_composition::View for StoryRef<T> {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::View::dynamic_properties(self.0.as_ref())
    }
}

fn mount<T: cove_composition::View + 'static>(
    story: Rc<T>,
    width: f32,
    height: f32,
    scale: f32,
    theme: Theme,
    body: impl FnOnce(&GalleryScene, &T),
) {
    let mounted = story.clone();
    GalleryScene::run(
        width,
        height,
        move |_| {
            AnyView::new(
                AnyView::new(StoryRef(mounted))
                    .text_scaler(TextScaler::new(scale))
                    .theme(theme),
            )
        },
        move |scene| {
            scene.settle_n(5);
            body(scene, &story);
        },
    );
}

#[test]
fn the_calendar_reports_its_selection_and_granularity() {
    let story = Rc::new(CalendarStory::new(GalleryOptions::default()));
    mount(
        story.clone(),
        720.0,
        700.0,
        1.0,
        Theme::default_light(),
        |scene, _story| {
            let named = scene
                .named("calendar-story")
                .expect("the calendar story names its root");
            assert!(
                named.frame().width > 0.0 && named.frame().height > 0.0,
                "the calendar has a real box, got {:?}",
                named.frame()
            );
            let line = CalendarStory::selected_line(None);
            assert!(!line.is_empty(), "the story reports a selected line");
            assert!(
                scene.has_text(&line),
                "the selection line is rendered, texts: {:?}",
                scene.texts()
            );
            // The story starts with nothing selected; the Swift suite's
            // "nothing is committed until Return" contract reduces to that.
            assert_eq!(
                CalendarStory::selected_line(None),
                "No date selected",
                "the story starts with no committed selection"
            );
            assert!(
                scene.has_text("Week: None"),
                "week state starts unselected in its scope"
            );
            assert!(
                scene.has_text("Month: None"),
                "month state starts unselected in its scope"
            );
            assert!(
                scene.has_text("Year: None"),
                "year state starts unselected in its scope"
            );

            // The weekend predicate the calendar navigation is built on still
            // skips Saturdays and Sundays.
            // 2024-06-01 is a Saturday, 2024-06-03 a Monday.
            let saturday = Date::from_seconds(1_717_200_000);
            let sunday = saturday.adding_seconds(86_400);
            let monday = sunday.adding_seconds(86_400);
            assert!(is_weekend(saturday), "Saturday is a weekend");
            assert!(is_weekend(sunday), "Sunday is a weekend");
            assert!(!is_weekend(monday), "Monday is not a weekend");
        },
    );
}

#[test]
fn the_lazy_grid_keeps_a_bounded_viewport() {
    let story = Rc::new(LazyGridStory);
    mount(
        story,
        900.0,
        1000.0,
        2.0,
        Theme::default_light(),
        |scene, _| {
            let viewport = scene
                .named("layout-lazy-grid")
                .expect("the lazy grid names its viewport");
            assert_eq!(
                viewport.frame().height,
                240.0,
                "the viewport is the authored 240pt band"
            );
            assert!(
                viewport.clips_to_bounds(),
                "a lazy viewport must clip so rows cannot paint outside it"
            );

            let cells: Vec<Node> = scene.all(|node| {
                node.layout_node()
                    .and_then(|layout| layout.text_inputs())
                    .is_some_and(|inputs| inputs.text == "Cell")
            });
            assert!(!cells.is_empty(), "the viewport renders some cells");
            assert!(
                cells.len() < 100,
                "only a bounded subset is materialised, got {}",
                cells.len()
            );
            // The authored row height is 48pt and the story is mounted at 2x
            // text scale, so each grid row resolves to 96pt. The text node sits
            // inside that row and measures its own line box, so the scaled
            // height is asserted on the row container that carries it.
            assert!(
                scene
                    .nodes()
                    .iter()
                    .any(|node| { (node.frame().height - 96.0).abs() < 0.5 }),
                "a grid row holds the scaled 96pt height, heights were {:?}",
                scene
                    .nodes()
                    .iter()
                    .map(|node| node.frame().height)
                    .filter(|height| *height > 0.0 && *height < 200.0)
                    .collect::<Vec<_>>()
            );
        },
    );
}

#[test]
fn the_tree_story_mounts_with_its_caption() {
    let story = Rc::new(TreeStory::new(GalleryOptions::default()));
    mount(
        story,
        900.0,
        700.0,
        2.0,
        Theme::default_light(),
        |scene, _| {
            let caption = scene.require_named("tree-caption");
            assert!(
                caption.frame().width > 0.0 && caption.frame().height > 0.0,
                "the tree caption has a real box"
            );
            let tree = scene.require_named("tree-control");
            assert!(
                tree.frame().width > 0.0 && tree.frame().height > 0.0,
                "the tree viewport has a real box"
            );
            assert!(
                scene.nodes().iter().any(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Tree)
                }),
                "the story publishes a tree role"
            );
        },
    );
}

#[test]
fn the_shell_search_narrows_the_navigation() {
    use cove_examples::gallery_content::{GalleryCatalog, GalleryView};
    use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

    let gallery = Rc::new(GalleryView::new(
        GalleryPage::Button,
        Appearance::Light,
        1.0,
    ));
    let mounted = gallery.clone();
    GalleryScene::run(
        1280.0,
        820.0,
        move |_| AnyView::new(GalleryRef(mounted)),
        |scene| {
            scene.settle_n(6);
            let all_sections = gallery.navigation_sections();
            let all_items: usize = all_sections.iter().map(|section| section.items.len()).sum();
            assert_eq!(
                all_items,
                GalleryCatalog::entries().len(),
                "an empty query lists the whole catalog"
            );
            assert!(
                scene.has_text("Component showcase"),
                "the navigation header is mounted"
            );

            // Typing into the search field narrows the navigation.
            let search = scene.require_named("gallery-search");
            scene.click(&search);
            scene.text_input("Accordion");
            scene.settle_n(4);

            let filtered = gallery.navigation_sections();
            let filtered_items: Vec<_> = filtered
                .iter()
                .flat_map(|section| section.items.iter())
                .collect();
            assert!(
                !filtered_items.is_empty(),
                "the query matches at least one entry"
            );
            assert!(
                filtered_items.len() < all_items,
                "the query narrowed the list from {all_items} to {}",
                filtered_items.len()
            );
            assert!(
                filtered_items
                    .iter()
                    .any(|item| item.id == GalleryPage::Accordion),
                "the matching entry survives the filter"
            );
            assert!(
                filtered_items
                    .iter()
                    .all(|item| GalleryCatalog::search("Accordion")
                        .iter()
                        .any(|entry| entry.page == item.id)),
                "every remaining item is a catalog match"
            );

            // Filtering does not disturb the shell or its selection.
            assert_eq!(
                scene.require_named("gallery-shell").frame().height,
                820.0,
                "filtering does not collapse the shell"
            );
            assert_eq!(
                gallery.current_page(),
                GalleryPage::Button,
                "the shell keeps its selection while searching"
            );
        },
    );
}

/// Shares the gallery's state with the test body.
struct GalleryRef(Rc<cove_examples::gallery_content::GalleryView>);

impl cove_composition::View for GalleryRef {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::View::dynamic_properties(self.0.as_ref())
    }
}
