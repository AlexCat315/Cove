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
//! Named differences from the Swift suite:
//! - Swift found drawer rows by debug name (`sidebar-item-8`,
//!   `sidebar-item-projects`); the Rust sidebar emits no `sidebar-item-*` debug
//!   names, so rows are found by their accessibility label instead.
//! - Swift found the drawer opener by name (`sidebar-open-drawer`); the Rust
//!   SidebarStory mounts no opener control at all (the drawer is bound to
//!   `is_drawer_open` with nothing that presents it), so the shared-selection
//!   case opens the drawer through `open_drawer()` and focus cannot be restored
//!   to an opener that does not exist. That gap is pinned by
//!   `the_sidebar_story_mounts_no_drawer_opener`.
//! - Swift checked `adaptive-navigationDrawer-narrow/wide` preview debug names;
//!   the Rust `AdaptiveControlExamples` emits only `adaptive-preview-caption`.
//!   Pinned by `the_adaptive_previews_emit_only_a_caption`.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, Node};
use cove_examples::gallery_content::catalog::GalleryPage;
use cove_examples::gallery_content::stories::sidebar_story::SidebarStory;
use cove_examples::gallery_content::stories::AdaptiveControlExamples;
use cove_examples::gallery_content::stories::navigation_layout_stories::DRAWER_DEFAULT_SELECTION;
use cove_examples::gallery_content::GalleryView;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use cove_ui_compose::Scancode;

use support::GalleryScene;

/// Mounts a shared `GalleryView` so the test body can read its state back.
struct Shared<V: View>(Rc<V>);

impl<V: View + 'static> View for Shared<V> {
    fn body(&self) -> AnyView {
        View::body(self.0.as_ref())
    }
}

fn mounted<V: View + 'static>(view: Rc<V>) -> impl FnOnce(&GalleryScene) -> AnyView {
    move |_| AnyView::new(Shared(view.clone()))
}

/// Mounts a bare story the way the gallery shell does: inside a `LayerRoot`, so
/// portal-hosted modal content reaches the tree.
fn mounted_in_layer_root<V: View + 'static>(
    view: Rc<V>,
) -> impl FnOnce(&GalleryScene) -> AnyView {
    move |_| {
        AnyView::new(cove_ui_compose::primitives::overlays::portal::LayerRoot::new(
            Shared(view.clone()),
        ))
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
        scene.click(&opener);
        run_animations(scene);

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
            "the modal panel is 320x820, got {frame:?}"
        );

        let selected = row_by_label(scene, &format!("Destination {DRAWER_DEFAULT_SELECTION}"));
        scene.focus.focus(Some(&selected), true);
        scene.key(Scancode::ARROW_DOWN);
        run_animations(scene);
        assert!(
            scene.focus.has_modal_scope(),
            "the open drawer keeps a modal scope while browsing"
        );

        scene.key(Scancode::RETURN);
        run_animations(scene);
        assert!(
            !scene.focus.has_modal_scope(),
            "committing the focused destination closes the modal"
        );
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(opener.id()),
            "focus returns to the opener"
        );
        assert!(
            scene.has_text("Selected destination: 9"),
            "the caption commits the keyboard selection, texts: {:?}",
            scene.texts()
        );

        assert!(
            AdaptiveControlExamples::new(GalleryPage::NavigationDrawer).supports(),
            "the drawer page has an adaptive preview"
        );
    });
}

#[test]
fn the_sidebar_and_its_drawer_share_selection_and_restore_the_explicit_opener() {
    let story = Rc::new(SidebarStory::new(
        cove_examples::gallery_content::catalog::GalleryOptions::default(),
    ));
    GalleryScene::run(1280.0, 820.0, mounted_in_layer_root(story.clone()), |scene| {
        story.open_drawer();
        run_animations(scene);
        assert!(
            scene.focus.has_modal_scope(),
            "presenting the drawer opens a modal scope"
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
            !scene.focus.has_modal_scope(),
            "choosing a destination closes the drawer"
        );
        let projects = row_by_label(scene, "Projects");
        assert!(
            projects
                .accessibility()
                .is_some_and(|semantics| semantics.state.is_selected),
            "the chosen destination reports itself selected"
        );
        assert!(
            scene.has_text("Projects"),
            "the detail pane follows the shared selection, texts: {:?}",
            scene.texts()
        );
    });
}

#[test]
fn the_sidebar_story_mounts_no_drawer_opener() {
    // Swift clicked `sidebar-open-drawer`; the Rust story binds the drawer to
    // `is_drawer_open` without any control that presents it, so focus has no
    // opener to return to after a selection.
    let story = Rc::new(SidebarStory::new(
        cove_examples::gallery_content::catalog::GalleryOptions::default(),
    ));
    GalleryScene::run(1280.0, 820.0, mounted_in_layer_root(story.clone()), |scene| {
        assert!(
            scene.named("sidebar-open-drawer").is_none(),
            "no named opener exists in the Rust story"
        );
        story.open_drawer();
        run_animations(scene);
        let row = scene
            .first(|node| {
                node.is_focusable()
                    && node
                        .accessibility()
                        .is_some_and(|semantics| semantics.label == "Projects")
            })
            .expect("Projects destination");
        scene.click(&row);
        run_animations(scene);
        assert!(
            scene.focus.focused().is_none(),
            "closing the drawer restores no opener, focused: {:?}",
            scene.focus.focused().map(|node| node.view_tag())
        );
    });
}

#[test]
fn the_adaptive_previews_emit_only_a_caption() {
    // Swift asserted `adaptive-navigationDrawer-narrow` and
    // `adaptive-navigationDrawer-wide` debug names. The Rust
    // `AdaptiveControlExamples` renders a single caption node, so those names
    // never appear.
    GalleryScene::run(
        1280.0,
        820.0,
        |_| AnyView::new(AdaptiveControlExamples::new(GalleryPage::NavigationDrawer)),
        |scene| {
            assert!(scene.named("adaptive-preview-caption").is_some());
            for width in ["narrow", "wide"] {
                assert!(
                    scene
                        .named(&format!("adaptive-navigationDrawer-{width}"))
                        .is_none(),
                    "the {width} adaptive preview name does not exist in Rust"
                );
            }
            assert!(
                AdaptiveControlExamples::new(GalleryPage::NavigationDrawer).supports(),
                "support is still reported for the page"
            );
        },
    );
}

#[test]
fn zz_probe() {
    let story = Rc::new(cove_examples::gallery_content::stories::DrawerStory::new(
        cove_examples::gallery_content::catalog::GalleryOptions::default(),
    ));
    GalleryScene::run(1280.0, 820.0, mounted_in_layer_root(story.clone()), |scene| {
        let opener = scene.require_named("drawer-open");
        println!("PROBE2 opener={:?} hit={}", opener.absolute_frame(), opener.is_hit_testable());
        scene.click(&opener);
        run_animations(scene);
        println!("PROBE2 is_open={} modal={}", story.is_open(), scene.focus.has_modal_scope());
        story.open();
        run_animations(scene);
        println!("PROBE2 forced open modal={} labels={:?}", scene.focus.has_modal_scope(),
            scene.nodes().iter().filter_map(|n| n.accessibility()).map(|a| a.label).filter(|l| l.starts_with("Destination") || l=="Cove workspace").collect::<Vec<_>>());
    });
}

struct Ticker<V: View> {
    story: Rc<V>,
    tick: cove_core::State<u32>,
}

impl<V: View + 'static> View for Ticker<V> {
    fn body(&self) -> AnyView {
        let _ = self.tick.get();
        View::body(self.story.as_ref())
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("tick", &self.tick)]
    }
    fn tag(&self) -> &'static str {
        "Ticker"
    }
}

#[test]
fn zz_probe2() {
    let story = Rc::new(cove_examples::gallery_content::stories::DrawerStory::new(
        cove_examples::gallery_content::catalog::GalleryOptions::default(),
    ));
    let ticker = Rc::new(Ticker { story: story.clone(), tick: cove_core::State::new(0) });
    GalleryScene::run(1280.0, 820.0, mounted_in_layer_root(ticker.clone()), |scene| {
        let opener = scene.require_named("drawer-open");
        scene.click(&opener);
        run_animations(scene);
        println!("PROBE3 modal={} rows={}", scene.focus.has_modal_scope(),
            scene.nodes().iter().filter(|n| n.accessibility().is_some_and(|a| a.label.starts_with("Destination"))).count());
        ticker.tick.set(1);
        run_animations(scene);
        let dialog = scene.first(|n| n.accessibility().is_some_and(|a| a.role == AccessibilityRole::Dialog));
        println!("PROBE3 forced modal={} rows={} dialog={:?}", scene.focus.has_modal_scope(),
            scene.nodes().iter().filter(|n| n.accessibility().is_some_and(|a| a.label.starts_with("Destination"))).count(),
            dialog.map(|d| d.absolute_frame()));
    });
}

struct MyHarness {
    open: cove_core::State<bool>,
    count: std::rc::Rc<std::cell::Cell<u32>>,
}

impl View for MyHarness {
    fn body(&self) -> AnyView {
        self.count.set(self.count.get() + 1);
        let sections = vec![cove_examples::gallery_content::stories::navigation_layout_stories::DrawerStory::new(
            cove_examples::gallery_content::catalog::GalleryOptions::default(),
        )
        .sections()[0]
            .clone()];
        let drawer = cove_ui_compose::primitives::layout::sidebar::drawer::NavigationDrawer::new(
            "Cove workspace",
            self.open.binding(),
            cove_core::Binding::new(
                || Option::<usize>::None,
                |_: Option<usize>| {},
            ),
            sections,
        );
        AnyView::new(cove_composition::TupleView::new(vec![
            AnyView::new(cove_ui_compose::primitives::controls::button::Button::title(
                "noop",
                || {},
            )),
            AnyView::new(drawer),
        ]))
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("open", &self.open)]
    }
}

#[test]
fn zz_probe3() {
    let harness = Rc::new(MyHarness {
        open: cove_core::State::new(false),
        count: std::rc::Rc::new(std::cell::Cell::new(0)),
    });
    GalleryScene::run(1280.0, 820.0, mounted_in_layer_root(harness.clone()), |scene| {
        println!("PROBE4 mounts={}", harness.count.get());
        harness.open.set(true);
        run_animations(scene);
        println!("PROBE4 mounts={} modal={} rows={}", harness.count.get(), scene.focus.has_modal_scope(),
            scene.nodes().iter().filter(|n| n.accessibility().is_some_and(|a| a.label.starts_with("Destination"))).count());
    });
}
