//! Translation of `Tests/CoveUIComposeTests/MenuButtonTests.swift`.
//!
//! Behaviour checklist:
//! - a closed `MenuButton` publishes no portal entry
//! - a disabled `MenuButton` never opens
//! - clicking the trigger mounts one portal entry, and activating an item runs
//!   it and closes the menu

use crate::support;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, AccessibilityTree, Node};
use cove_ui_compose::primitives::controls::menu_button::MenuButtonTitle;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;

use support::InteractiveScene;

/// The menu button's own trigger. `MenuButtonTitle` gives it the caller's title
/// as its accessible name.
fn trigger(scene: &InteractiveScene) -> Node {
    // `MenuButton::accessibility_label` lands on a `PopoverLabelSemantics`
    // descendant rather than on the button host itself, so the trigger is located
    // by the composed label Swift's suite read from the same nodes.
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                && AccessibilityTree::resolved_label(node) == "Options"
        })
        .expect("the menu button trigger is mounted")
}

fn item(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .labelled(label, Some(AccessibilityRole::Button))
        .unwrap_or_else(|| panic!("the menu item {label:?} is mounted"))
}

#[test]
fn opening_the_menu_mounts_a_portal_and_an_item_closes_it() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let runs = std::rc::Rc::new(std::cell::Cell::new(0));
        let tally = runs.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            MenuButtonTitle::new(
                "Options",
                vec![MenuEntry::Item(MenuItem::new(1, "Save", move || {
                    tally.set(tally.get() + 1)
                }))],
            ),
        ))));

        assert!(
            scene.portal.entries().is_empty(),
            "a closed menu button publishes no portal entry"
        );

        let trigger = trigger(scene);
        scene.click(&trigger, 1);
        assert_eq!(scene.portal.entries().len(), 1, "the click opens the menu");

        scene.click(&item(scene, "Save"), 1);
        assert_eq!(runs.get(), 1, "the item ran");
        assert!(scene.portal.entries().is_empty(), "the menu closed");
    });
}

#[test]
fn disabled_menu_button_does_not_open() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            MenuButtonTitle::new(
                "Options",
                vec![MenuEntry::Item(MenuItem::new(1, "Save", || {}))],
            )
            .enabled(false),
        ))));

        let trigger = trigger(scene);
        scene.click(&trigger, 1);
        assert!(
            scene.portal.entries().is_empty(),
            "a disabled menu button never opens"
        );
    });
}
