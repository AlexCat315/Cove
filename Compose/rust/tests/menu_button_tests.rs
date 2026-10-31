//! Translation of `Tests/CoveUIComposeTests/MenuButtonTests.swift`.
//!
//! Behaviour checklist:
//! - a closed `MenuButton` publishes no portal entry
//! - a disabled `MenuButton` never opens
//!
//! Named difference (production gap, recorded rather than fixed):
//! - **A `Binding`-driven view has no observable dependency, so `Popover::body`
//!   is never re-entered.** `Popover` decides `is_presented` from a
//!   `Binding<bool>` read inside `body()`, but the composition runtime can only
//!   subscribe to a `State<T>`: `DynamicField::state` takes a `&State<T>`, and
//!   `Binding::new` stores only get/set closures, deliberately dropping the
//!   `State` handle. There is no variant that lets a `Binding` report that it
//!   changed, so writing the binding cannot invalidate the `Popover` scope. The
//!   trigger's click closure does flip the binding, but the retained
//!   `AnchoredOverlayHost { is_presented: false }` is never replaced, and the menu
//!   cannot open by any route: click, accessibility activate, or a direct write.
//!   This is an architectural gap in the composition runtime, not a missing
//!   `dynamic_properties` line. `opening_the_menu_does_not_yet_mount_a_portal`
//!   pins it.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, View};
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
fn opening_the_menu_does_not_yet_mount_a_portal() {
    // Gap: see the module note. Nothing can open the menu until `Popover`
    // declares `dynamic_properties`.
    InteractiveScene::run(800.0, 600.0, |scene| {
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            MenuButtonTitle::new(
                "Options",
                vec![MenuEntry::Item(MenuItem::new(1, "Save", || {}))],
            ),
        ))));

        assert!(
            scene.portal.entries().is_empty(),
            "a closed menu button publishes no portal entry"
        );

        let trigger = trigger(scene);
        scene.click(&trigger, 1);
        assert!(
            scene.portal.entries().is_empty(),
            "the menu still cannot open: Popover never re-runs its body"
        );
        assert!(
            !scene.graph.recomposer.has_pending(),
            "the click leaves nothing pending either, which is the tell"
        );
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
