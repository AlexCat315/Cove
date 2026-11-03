//! Translation of `Tests/CoveUIComposeTests/NativeMenuTests.swift`.
//!
//! Behaviour checklist:
//! - a menu button's entries build a two-item native snapshot whose first
//!   regular item runs its action once, and the native path never opens a
//!   self-drawn portal
//! - the snapshot encodes a label as a disabled regular item, a checked item, a
//!   separator, a submenu with one disabled child, and a disabled item
//! - the shortcut parser decodes macOS glyphs (`⌘⇧K`) and word modifiers
//!   (`Ctrl+Alt+F`), and a missing shortcut parses to `None`
//! - an entry with a template icon yields a rasterized template PNG, while a
//!   plain item carries no icon
//!

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, NativeMenuPresenter};
use cove_platform::NativeMenuKind;
use cove_ui_compose::primitives::controls::MenuButtonTitle;
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::primitives::overlays::menu::native_menu_bridge::NativeMenuBuilder;
use cove_ui_compose::primitives::overlays::menu::{
    MenuEntry, MenuItem, MenuItemContent, MenuSubmenu,
};
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStoreHolder};

use support::InteractiveScene;

#[test]
fn menu_button_uses_the_native_presenter_and_runs_the_chosen_item() {
    let runs = Rc::new(Cell::new(0));
    let save_runs = runs.clone();
    let export_runs = runs.clone();
    let entries = vec![
        MenuEntry::Item(MenuItem::new(0, "Save", move || {
            save_runs.set(save_runs.get() + 1)
        })),
        MenuEntry::Item(MenuItem::new(1, "Export", move || {
            export_runs.set(export_runs.get() + 10)
        })),
    ];

    let built = NativeMenuBuilder::build(&entries);
    assert_eq!(
        built.snapshot.items.len(),
        2,
        "the native snapshot carries both entries"
    );
    let chosen = built
        .snapshot
        .items
        .iter()
        .find(|item| item.kind == NativeMenuKind::Regular)
        .map(|item| item.id)
        .expect("the snapshot has a choosable regular item");
    built.run(chosen);
    assert_eq!(
        runs.get(),
        1,
        "running the item the native menu reported invokes its action once"
    );

    runs.set(0);
    InteractiveScene::run(800.0, 600.0, |scene| {
        let entries = entries.clone();
        PortalStoreHolder::current().clear();
        let presented = Rc::new(Cell::new(None::<usize>));
        let seen = presented.clone();
        NativeMenuPresenter::set_present(Some(Rc::new(move |snapshot, _, done| {
            seen.set(Some(snapshot.items.len()));
            done(
                snapshot
                    .items
                    .iter()
                    .find(|item| item.kind == NativeMenuKind::Regular)
                    .map(|item| item.id),
            );
        })));
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            MenuButtonTitle::new("Options", entries),
        ))));
        let trigger = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            })
            .expect("the menu button trigger is mounted");
        scene.click(&trigger, 1);
        NativeMenuPresenter::set_present(None);

        assert_eq!(
            presented.get(),
            Some(2),
            "the presenter received both entries"
        );
        assert_eq!(runs.get(), 1, "the chosen item ran once");
        assert!(
            scene.portal.entries().is_empty(),
            "the native path never opens a self-drawn portal"
        );
    });
}

#[test]
fn snapshot_encodes_labels_separators_submenus_checks_and_disabled_items() {
    let entries = vec![
        MenuEntry::label(0, "Workspace"),
        MenuEntry::Item(MenuItem::new(1, "Pin", || {}).selected(true)),
        MenuEntry::separator(2),
        MenuEntry::Submenu(MenuSubmenu::new(
            3,
            "Settings",
            vec![MenuEntry::Item(
                MenuItem::new(4, "General", || {}).enabled(false),
            )],
        )),
        MenuEntry::Item(MenuItem::new(5, "Delete", || {}).enabled(false)),
    ];

    let items = NativeMenuBuilder::build(&entries).snapshot.items;
    assert_eq!(items.len(), 5, "every entry yields one snapshot item");

    assert_eq!(
        items[0].title, "Workspace",
        "a label keeps its title in the snapshot"
    );
    assert!(!items[0].enabled, "a label becomes a disabled regular item");
    assert!(items[1].checked, "a selected item is checked");
    assert!(
        matches!(items[2].kind, NativeMenuKind::Separator),
        "a separator keeps its kind, saw {:?}",
        items[2].kind
    );
    match &items[3].kind {
        NativeMenuKind::Submenu(branch) => {
            assert_eq!(branch.items.len(), 1, "the submenu carries one child");
            assert!(
                !branch.items[0].enabled,
                "the submenu's disabled child stays disabled"
            );
        }
        other => panic!("expected a submenu, saw {other:?}"),
    }
    assert!(!items[4].enabled, "a disabled regular item stays disabled");
}

#[test]
fn shortcut_parser_decodes_glyphs_and_word_modifiers() {
    let mac = NativeMenuBuilder::parse_shortcut(Some("⌘⇧K")).expect("the glyph shortcut parses");
    assert_eq!(mac.key, "K", "the glyph form keeps the letter key");
    assert!(mac.meta, "⌘ sets meta");
    assert!(mac.shift, "⇧ sets shift");
    assert!(!mac.ctrl, "⌘ does not set ctrl");

    let win =
        NativeMenuBuilder::parse_shortcut(Some("Ctrl+Alt+F")).expect("the word shortcut parses");
    assert_eq!(win.key, "F", "the word form keeps the letter key");
    assert!(win.ctrl, "Ctrl sets ctrl");
    assert!(win.alt, "Alt sets alt");

    assert_eq!(
        NativeMenuBuilder::parse_shortcut(None),
        None,
        "a missing shortcut parses to nothing"
    );
}

#[test]
fn native_menu_icons_rasterize_as_templates() {
    let entries = vec![
        MenuEntry::Item(
            MenuItem::new(0, "Save", || {}).content(MenuItemContent::new(
                Some(ui_icon_catalog::file::save()),
                Vec::new(),
                None,
                None,
            )),
        ),
        MenuEntry::Item(MenuItem::new(1, "Plain", || {})),
    ];

    let items = NativeMenuBuilder::build(&entries).snapshot.items;
    let icon = items[0]
        .icon
        .as_ref()
        .expect("the template icon rasterizes");
    assert!(
        icon.is_template,
        "the icon is a template so the menu can tint it"
    );
    assert!(
        !icon.png.is_empty(),
        "the template raster carries encoded PNG bytes"
    );
    assert_eq!(
        &icon.png[..8],
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
        "the raster is a PNG stream"
    );
    assert!(items[1].icon.is_none(), "a plain item carries no icon");
}
