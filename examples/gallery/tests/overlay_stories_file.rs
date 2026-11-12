//! Behaviour tests for the overlay story file.
//!
//! Translation of `GalleryContent/Stories/Layout/OverlayStories.swift`, which holds
//! six stories behind six gallery pages. The menu tree and the tab-close
//! fallback are the parts with real logic.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, Scope, View};
use cove_core::DrawList;
use cove_core::scene::{
    AccessibilityRole, LayerAwareNodeRenderer, NativeMenuCompletion, NativeMenuPresenter, Node,
};
use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::overlay_stories_file::{
    DisclosureStory, GROUP_STRIDE, INITIAL_DOCUMENTS, MANY_COMMANDS, MenuStory, ModalStory,
    PopoverStory, TabsStory, leaf_count, menu_depth, tab_after_close,
};
use cove_platform::{NativeMenuItem, NativeMenuKind, NativeMenuSnapshot};
use cove_ui_compose::primitives::controls::KeyboardShortcut;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, NativeMenuBuilder};
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStoreHolder};
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::{GalleryScene, GlobalTestLock, collect};

struct MountedMenu(Rc<MenuStory>);

impl View for MountedMenu {
    fn body(&self, cx: &Scope) -> AnyView {
        self.0.body(cx)
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        self.0.dynamic_properties()
    }
}

/// Captures the data sent by the mounted native trigger, not a helper-built menu.
/// This tests the platform boundary, not an OS popup's visual/keyboard behavior.
#[derive(Default)]
struct NativeProbe {
    snapshot: RefCell<Option<NativeMenuSnapshot>>,
    completion: RefCell<Option<NativeMenuCompletion>>,
}

struct ResetNativePresenter;

impl Drop for ResetNativePresenter {
    fn drop(&mut self) {
        NativeMenuPresenter::set_present(None);
    }
}

impl NativeProbe {
    fn open(&self, scene: &GalleryScene, name: &str) -> NativeMenuSnapshot {
        self.snapshot.borrow_mut().take();
        click_trigger(scene, name);
        assert!(PortalStoreHolder::current().entries().is_empty());
        self.snapshot
            .borrow()
            .clone()
            .expect("the native presenter ran")
    }

    fn complete(&self, scene: &GalleryScene, chosen: Option<i64>) {
        let done = self
            .completion
            .borrow_mut()
            .take()
            .expect("an open native menu");
        done(chosen);
        scene.settle();
    }

    fn choose(&self, scene: &GalleryScene, name: &str, path: &[&str]) {
        let snapshot = self.open(scene, name);
        let item = native_item(&snapshot, path);
        assert!(item.enabled);
        assert!(matches!(item.kind, NativeMenuKind::Regular));
        self.complete(scene, Some(item.id));
    }
}

fn mount_menu(options: GalleryOptions, body: impl FnOnce(&GalleryScene, &MenuStory, &NativeProbe)) {
    GlobalTestLock::locked(|| {
        let probe = Rc::new(NativeProbe::default());
        let captured = probe.clone();
        NativeMenuPresenter::set_present(Some(Rc::new(move |snapshot, _, done| {
            *captured.snapshot.borrow_mut() = Some(snapshot.clone());
            *captured.completion.borrow_mut() = Some(done);
        })));
        let _reset = ResetNativePresenter;
        let story = Rc::new(MenuStory::new(options));
        let mounted = story.clone();
        GalleryScene::run(
            1000.0,
            1100.0,
            move |_| {
                AnyView::new(LayerRoot::new(MountedMenu(mounted)).theme(Theme::default_light()))
            },
            |scene| body(scene, &story, &probe),
        );
    });
}

fn trigger(scene: &GalleryScene, name: &str) -> Node {
    collect(&scene.require_named(name))
        .into_iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|a| a.role == AccessibilityRole::Button)
        })
        .unwrap_or_else(|| panic!("{name} has a mounted trigger"))
}

fn click_trigger(scene: &GalleryScene, name: &str) {
    scene.click(&trigger(scene, name));
    scene.settle();
}

fn row(scene: &GalleryScene, id: u64) -> Node {
    scene
        .first(|node| node.attachment::<u64>("__menu_item_id") == Some(id))
        .unwrap_or_else(|| panic!("menu row {id} is mounted"))
}

fn native_item<'a>(snapshot: &'a NativeMenuSnapshot, path: &[&str]) -> &'a NativeMenuItem {
    let item = snapshot
        .items
        .iter()
        .find(|item| item.title == path[0])
        .unwrap_or_else(|| panic!("missing native menu path {path:?}"));
    if path.len() == 1 {
        item
    } else if let NativeMenuKind::Submenu(children) = &item.kind {
        native_item(children, &path[1..])
    } else {
        panic!("{} is not a submenu", item.title);
    }
}

fn assert_status(scene: &GalleryScene, story: &MenuStory, expected: &str) {
    assert_eq!(story.status(), expected);
    assert!(
        collect(&scene.require_named("menu-status"))
            .iter()
            .any(|node| {
                scene
                    .graph
                    .layout_node(node)
                    .and_then(|layout| layout.text_inputs())
                    .is_some_and(|inputs| inputs.text == expected)
            }),
        "the mounted caption must show {expected:?}"
    );
}

#[test]
fn mounted_native_workspace_uses_the_complete_tree_and_callbacks() {
    mount_menu(GalleryOptions::default(), |scene, story, native| {
        let snapshot = native.open(scene, "menu-native");
        // Comparing the actual trigger payload pins the helper/body wiring too.
        assert_eq!(
            snapshot,
            NativeMenuBuilder::build(&story.entries()).snapshot
        );
        assert_eq!(menu_depth(&story.entries()), 3);
        assert_eq!(leaf_count(&story.entries()), 9);
        assert_eq!(snapshot.items.len(), 7);
        assert_eq!(snapshot.items[0].title, "Workspace");
        assert!(!snapshot.items[0].enabled);
        assert!(matches!(snapshot.items[3].kind, NativeMenuKind::Separator));
        assert!(native_item(&snapshot, &["Pin item"]).checked);
        assert!(native_item(&snapshot, &["Settings", "Appearance", "System theme"]).checked);
        assert!(!native_item(&snapshot, &["Unavailable"]).enabled);
        assert!(!native_item(&snapshot, &["Settings", "Restricted"]).enabled);
        assert_eq!(
            native_item(&snapshot, &["Copy"]).shortcut,
            NativeMenuBuilder::parse_shortcut(Some(
                &KeyboardShortcut::primary("C").display_string()
            ))
        );
        native.complete(scene, None);
        assert_status(scene, story, "Choose a command");

        for (path, status) in [
            (vec!["Copy"], "Copied"),
            (vec!["Settings", "General"], "General settings"),
            (
                vec!["Settings", "Appearance", "System theme"],
                "System theme",
            ),
            (
                vec!["Settings", "Appearance", "Accent color", "Blue"],
                "Blue accent",
            ),
            (
                vec!["Settings", "Appearance", "Accent color", "Green"],
                "Green accent",
            ),
            (vec!["Delete"], "Deleted"),
            (vec!["Pin item"], "Unpinned"),
            (vec!["Pin item"], "Pinned"),
        ] {
            native.choose(scene, "menu-native", &path);
            assert_status(scene, story, status);
            let snapshot = native.open(scene, "menu-native");
            assert_eq!(
                native_item(&snapshot, &["Pin item"]).checked,
                story.is_checked()
            );
            native.complete(scene, None);
        }
    });
}

#[test]
fn explicit_custom_menu_renders_roles_and_runs_the_same_nested_actions() {
    mount_menu(GalleryOptions::default(), |scene, story, native| {
        click_trigger(scene, "menu-custom");
        assert!(
            native.snapshot.borrow().is_none(),
            "custom must bypass an installed native presenter"
        );
        assert_eq!(PortalStoreHolder::current().entries().len(), 1);
        assert_eq!(
            row(scene, 101).accessibility().unwrap().help,
            KeyboardShortcut::primary("C").display_string()
        );
        assert!(row(scene, 102).accessibility().unwrap().state.is_selected);
        assert!(!row(scene, 112).accessibility().unwrap().state.is_enabled);

        // A destructive role must reach the mounted Rust renderer, not just exist
        // on a helper's unused MenuItem. Native snapshots currently omit roles.
        let delete_text = collect(&row(scene, 113))
            .into_iter()
            .find(|node| {
                scene
                    .graph
                    .layout_node(node)
                    .and_then(|layout| layout.text_inputs())
                    .is_some_and(|inputs| inputs.text == "Delete")
            })
            .expect("the Delete label is rendered");
        let mut renderer = LayerAwareNodeRenderer::new();
        renderer.render(scene.graph.render_tree(), &mut DrawList::default());
        let object = scene
            .graph
            .render_tree()
            .render_object(&delete_text)
            .unwrap();
        let glyphs = object
            .cached_layer_list()
            .expect("the Delete glyphs are cached");
        assert!(!glyphs.vertices().is_empty());
        assert!(
            glyphs
                .vertices()
                .iter()
                .all(|v| v.color == Theme::default_light().colors.error.rgba8())
        );

        scene.click(&row(scene, 112));
        scene.settle();
        assert_status(scene, story, "Choose a command");
        assert_eq!(PortalStoreHolder::current().entries().len(), 1);
        scene.click(&row(scene, 104));
        scene.settle();
        assert!(!row(scene, 111).accessibility().unwrap().state.is_enabled);
        scene.click(&row(scene, 111));
        scene.settle();
        assert_status(scene, story, "Choose a command");
        scene.key(cove_ui_compose::Scancode::ESCAPE);
        scene.settle();
        scene.key(cove_ui_compose::Scancode::ESCAPE);
        scene.settle();
        assert!(PortalStoreHolder::current().entries().is_empty());

        for (path, status) in [
            (vec![101], "Copied"),
            (vec![104, 105], "General settings"),
            (vec![104, 106, 107], "System theme"),
            (vec![104, 106, 108, 109], "Blue accent"),
            (vec![104, 106, 108, 110], "Green accent"),
            (vec![113], "Deleted"),
            (vec![102], "Unpinned"),
        ] {
            click_trigger(scene, "menu-custom");
            for id in path {
                scene.click(&row(scene, id));
                scene.settle();
            }
            assert_status(scene, story, status);
            assert!(
                PortalStoreHolder::current().entries().is_empty(),
                "a leaf closes all levels"
            );
        }
        assert!(!story.is_checked());
        let snapshot = native.open(scene, "menu-native");
        assert!(
            !native_item(&snapshot, &["Pin item"]).checked,
            "both examples share the pin state"
        );
        native.complete(scene, None);
        native.choose(scene, "menu-native", &["Pin item"]);
        assert_status(scene, story, "Pinned");
        click_trigger(scene, "menu-custom");
        assert!(row(scene, 102).accessibility().unwrap().state.is_selected);
    });
}

#[test]
fn disabled_story_triggers_never_present_native_or_custom_menus() {
    mount_menu(
        GalleryOptions {
            is_enabled: false,
            ..GalleryOptions::default()
        },
        |scene, story, native| {
            for name in [
                "menu-native",
                "menu-custom",
                "menu-scrollable",
                "menu-columns",
                "menu-selection",
            ] {
                assert!(
                    !trigger(scene, name)
                        .accessibility()
                        .unwrap()
                        .state
                        .is_enabled
                );
                click_trigger(scene, name);
                assert!(native.snapshot.borrow().is_none());
                assert!(PortalStoreHolder::current().entries().is_empty());
            }
            assert_status(scene, story, "Choose a command");
            assert!(story.is_checked());
        },
    );
}

#[test]
fn mounted_hundred_row_menu_runs_every_command_and_group_action() {
    mount_menu(GalleryOptions::default(), |scene, story, native| {
        let snapshot = native.open(scene, "menu-scrollable");
        assert_eq!(snapshot, NativeMenuBuilder::build(&story.many()).snapshot);
        assert_eq!(snapshot.items.len(), MANY_COMMANDS);
        assert_eq!(
            snapshot
                .items
                .iter()
                .filter(|item| matches!(item.kind, NativeMenuKind::Submenu(_)))
                .count(),
            MANY_COMMANDS / GROUP_STRIDE
        );
        assert_eq!(menu_depth(&story.many()), 1);
        assert_eq!(leaf_count(&story.many()), 110);
        native.complete(scene, None);
        for index in 0..MANY_COMMANDS {
            if index % GROUP_STRIDE == 0 {
                let group = format!("Group {index}");
                for action in ["Inspect group", "Export group"] {
                    native.choose(scene, "menu-scrollable", &[&group, action]);
                    assert_status(scene, story, &format!("{action} {index}"));
                }
            } else {
                let title = format!("Command {index}");
                native.choose(scene, "menu-scrollable", &[&title]);
                assert_status(scene, story, &title);
            }
        }
    });
}

#[test]
fn authored_ids_in_the_mounted_menus_are_unique() {
    fn ids(entries: &[MenuEntry], into: &mut Vec<u64>) {
        for entry in entries {
            into.push(entry.id());
            if let MenuEntry::Submenu(submenu) = entry {
                ids(&submenu.entries, into);
            }
        }
    }
    mount_menu(GalleryOptions::default(), |scene, story, native| {
        for (name, entries) in [
            ("menu-native", story.entries()),
            ("menu-scrollable", story.many()),
        ] {
            assert_eq!(
                native.open(scene, name),
                NativeMenuBuilder::build(&entries).snapshot
            );
            native.complete(scene, None);
            let mut all = Vec::new();
            ids(&entries, &mut all);
            let unique: std::collections::HashSet<_> = all.iter().collect();
            assert_eq!(all.len(), unique.len());
        }
    });
}

#[test]
fn the_popover_starts_closed_with_its_draft() {
    let story = PopoverStory::new(GalleryOptions::default());
    assert!(!story.is_open());
    assert_eq!(story.name(), "New project");
    story.open();
    assert!(story.is_open());
    story.set_name("Aurora");
    story.done();
    assert!(!story.is_open());
    assert_eq!(story.name(), "Aurora", "the draft is kept");
}

#[test]
fn the_modal_cancel_keeps_the_status_and_create_records_it() {
    let story = ModalStory::new(GalleryOptions::default());
    assert!(!story.is_open());
    assert_eq!(story.status(), "No changes");

    story.open();
    story.set_name("Aurora");
    story.cancel();
    assert!(!story.is_open());
    assert_eq!(story.status(), "No changes", "cancelling records nothing");

    story.open();
    story.create();
    assert!(!story.is_open());
    assert_eq!(story.status(), "Created Aurora");
}

#[test]
fn collapsing_a_disclosure_keeps_the_draft() {
    let story = DisclosureStory::new(GalleryOptions::default());
    assert!(story.is_expanded());
    assert_eq!(story.draft(), "The draft survives collapse");
    story.set_draft("edited");
    story.toggle();
    assert!(!story.is_expanded());
    assert_eq!(
        story.draft(),
        "edited",
        "caller-owned expansion preserves the draft"
    );
}

#[test]
fn the_content_tabs_include_a_disabled_and_a_badged_page() {
    let story = TabsStory::new(GalleryOptions::default());
    let tabs = story.content_tabs();
    assert_eq!(tabs.len(), 5);
    assert!(!tabs[1].is_enabled, "the Unavailable tab cannot be reached");
    assert_eq!(tabs[2].badge.as_deref(), Some("3"));
    assert_eq!(story.selection(), "overview");
    assert_eq!(story.segment(), "day");
}

#[test]
fn closing_a_document_falls_back_to_the_nearest_tab() {
    let story = TabsStory::new(GalleryOptions::default());
    assert_eq!(story.documents(), INITIAL_DOCUMENTS.to_vec());
    assert_eq!(story.document_selection(), "Scene.cove");

    // Closing the selected first document selects the one that took its slot.
    let fallback = story.close_document(0);
    assert_eq!(fallback.as_deref(), Some("Material.cove"));
    assert_eq!(story.document_selection(), "Material.cove");
    assert_eq!(story.documents(), vec!["Material.cove", "Notes.md"]);
}

#[test]
fn closing_the_last_document_falls_back_to_the_previous_tab() {
    let story = TabsStory::new(GalleryOptions::default());
    // Select the last document, then close it.
    story.close_document(1); // removes Material.cove; selection was Scene.cove
    assert_eq!(
        story.document_selection(),
        "Scene.cove",
        "an unselected close keeps the selection"
    );
    let fallback = story.close_document(1); // removes Notes.md
    assert_eq!(
        fallback.as_deref(),
        Some("Scene.cove"),
        "the nearest remaining tab"
    );
}

#[test]
fn closing_every_document_clears_the_selection() {
    let story = TabsStory::new(GalleryOptions::default());
    story.close_document(0);
    story.close_document(0);
    story.close_document(0);
    assert!(story.documents().is_empty());
    assert_eq!(story.document_selection(), "", "no documents, no selection");
    // The story offers a reset back to the authored set.
    story.reset_documents();
    assert_eq!(story.documents(), INITIAL_DOCUMENTS.to_vec());
}

#[test]
fn tab_after_close_never_indexes_past_the_end() {
    assert_eq!(tab_after_close(&[], 0), None);
    let docs = vec!["a".to_owned(), "b".to_owned()];
    // The requested index is clamped, so a stale index cannot panic.
    assert_eq!(tab_after_close(&docs, 99).as_deref(), Some("b"));
}
