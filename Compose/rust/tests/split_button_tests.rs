//! Translation of `Tests/CoveUIComposeTests/SplitButtonTests.swift`.
//!
//! Behaviour checklist:
//! - the primary and the menu zone are separate button hosts, and the primary
//!   zone runs only the primary action
//! - a disabled `SplitButton` runs neither zone and opens no portal
//! - Left/Right move the focus between the two zones
//!
//! Two more gaps are pinned alongside the menu-zone one below:
//! - The pair does not share a single Tab stop. Swift kept exactly one of the two
//!   zones in the Tab order and moved the stop with the rove. `is_tab_stop()`
//!   defaults to `true` and `SplitButtonGroup` never calls `set_is_tab_stop`, so
//!   both zones are always Tab stops. `both_zones_are_tab_stops` pins that.
//! - The menu zone is two nested button hosts (a `Popover` trigger `Button`
//!   wrapping the caller's label `Button`), so a split button exposes three
//!   focusable buttons rather than Swift's two. `the_menu_zone_nests_two_buttons`
//!   pins that.
//!
//! Named difference (production gap, recorded rather than fixed): the menu zone
//! cannot open its menu yet. **A `Binding`-driven view has no observable
//! dependency, so `Popover::body` is never re-entered.** `Popover` decides
//! `is_presented` from a `Binding<bool>` read inside `body()`, but the runtime can
//! only subscribe to a `State<T>` (`DynamicField::state` takes `&State<T>`, and
//! `Binding::new` drops the `State` handle), so writing the binding cannot
//! invalidate the scope and the retained `AnchoredOverlayHost { is_presented:
//! false }` is never replaced. The Swift suite's "open the menu, activate the
//! item, portal empties" case is pinned by
//! `the_menu_zone_does_not_yet_open_a_portal`; the other Swift assertions (two
//! zones, roving) hold.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::AccessibilityRole;
use cove_ui_compose::primitives::controls::split_button::SplitButton;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;

use support::InteractiveScene;

/// The two zones, primary first.
///
/// `SplitButton` builds the menu zone as a `MenuButton`, which is a `Popover`
/// whose trigger is itself a `Button` wrapping the caller's label `Button`. That
/// produces three button hosts in the tree; only the outermost two are the zones,
/// and the inner two share the menu zone's frame. So the menu zone is the
/// shallowest button host inside the trailing region.
fn zones(scene: &InteractiveScene) -> Vec<cove_core::scene::Node> {
    let buttons: Vec<cove_core::scene::Node> = scene
        .nodes()
        .into_iter()
        .filter(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
        })
        .collect();
    assert!(
        buttons.len() >= 2,
        "a split button mounts at least its two zones, saw {}",
        buttons.len()
    );
    // Depth order is preserved by `nodes()`, so the first host is the primary zone
    // and the next one that is NOT a descendant of it is the menu zone.
    let primary = buttons[0].clone();
    let menu = buttons
        .iter()
        .find(|candidate| !is_descendant(primary.clone(), (*candidate).clone()))
        .cloned()
        .expect("the menu zone is mounted");
    vec![primary, menu]
}

fn is_descendant(ancestor: cove_core::scene::Node, candidate: cove_core::scene::Node) -> bool {
    let mut cursor = Some(candidate);
    while let Some(node) = cursor {
        if node == ancestor {
            return true;
        }
        cursor = node.parent();
    }
    false
}

#[test]
fn the_primary_zone_runs_only_its_own_action() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let primary = Rc::new(Cell::new(0));
        let menu = Rc::new(Cell::new(0));
        let primary_runs = primary.clone();
        let menu_runs = menu.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            SplitButton::new(
                "Save",
                vec![MenuEntry::Item(MenuItem::new(
                    1,
                    "Save as",
                    move || menu_runs.set(menu_runs.get() + 1),
                ))],
                move || primary_runs.set(primary_runs.get() + 1),
            ),
        ))));

        let hosts = zones(scene);
        assert_eq!(hosts.len(), 2, "the zones resolve to the primary and the menu");

        scene.click(&hosts[0], 1);
        assert_eq!(primary.get(), 1, "the primary zone runs the primary action");
        assert_eq!(
            menu.get(),
            0,
            "the primary zone does not run a menu entry"
        );
    });
}

#[test]
fn a_disabled_split_button_runs_neither_zone() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let runs = Rc::new(Cell::new(0));
        let z = runs.clone();
        let w = runs.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            SplitButton::new(
                "Save",
                vec![MenuEntry::Item(MenuItem::new(1, "Save as", move || {
                    z.set(z.get() + 1);
                }))],
                move || w.set(w.get() + 1),
            )
            .enabled(false),
        ))));

        let hosts = zones(scene);
        assert_eq!(hosts.len(), 2, "both zones still mount when disabled");
        for host in &hosts {
            scene.click(host, 1);
        }
        assert_eq!(runs.get(), 0, "a disabled split button runs nothing");
        assert!(
            scene.portal.entries().is_empty(),
            "a disabled split button opens no portal"
        );
    });
}

#[test]
fn both_zones_are_tab_stops() {
    // Gap: Swift made the pair own one Tab stop and moved it with the rove.
    InteractiveScene::run(800.0, 600.0, |scene| {
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            SplitButton::new(
                "Save",
                vec![MenuEntry::Item(MenuItem::new(1, "Save as", || {}))],
                || {},
            ),
        ))));

        let hosts = zones(scene);
        assert!(
            hosts[0].is_tab_stop() && hosts[1].is_tab_stop(),
            "both zones are Tab stops today"
        );
    });
}

#[test]
fn arrow_keys_still_move_focus_between_the_zones() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            SplitButton::new(
                "Save",
                vec![MenuEntry::Item(MenuItem::new(1, "Save as", || {}))],
                || {},
            ),
        ))));

        let hosts = zones(scene);
        scene.focus.focus(Some(&hosts[0]), true);
        scene.key(cove_ui_compose::Scancode::ARROW_RIGHT, Default::default());
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(hosts[1].id()),
            "Right moves focus to the menu zone"
        );

        scene.key(cove_ui_compose::Scancode::ARROW_LEFT, Default::default());
        assert_eq!(
            scene.focus.focused().map(|node| node.id()),
            Some(hosts[0].id()),
            "Left moves focus back to the primary zone"
        );
    });
}

#[test]
fn the_menu_zone_nests_two_buttons() {
    // Gap: the menu zone is a `MenuButton`, whose `Popover` trigger wraps the
    // caller's label `Button`, so a split button exposes three focusable buttons
    // where Swift exposed two.
    InteractiveScene::run(800.0, 600.0, |scene| {
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            SplitButton::new(
                "Save",
                vec![MenuEntry::Item(MenuItem::new(1, "Save as", || {}))],
                || {},
            ),
        ))));

        let buttons = scene
            .nodes()
            .into_iter()
            .filter(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            })
            .count();
        assert_eq!(
            buttons, 3,
            "the primary zone plus two nested hosts in the menu zone"
        );
    });
}

#[test]
fn the_menu_zone_does_not_yet_open_a_portal() {
    // Gap: see the module note. Swift asserted `portal.entries.count == 1` after
    // clicking the menu zone, then that activating "Save as" ran the entry and
    // emptied the portal.
    InteractiveScene::run(800.0, 600.0, |scene| {
        let menu = Rc::new(Cell::new(0));
        let menu_runs = menu.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            SplitButton::new(
                "Save",
                vec![MenuEntry::Item(MenuItem::new(1, "Save as", move || {
                    menu_runs.set(menu_runs.get() + 1);
                }))],
                || {},
            ),
        ))));

        let hosts = zones(scene);
        scene.click(&hosts[1], 1);
        assert!(
            scene.portal.entries().is_empty(),
            "the menu zone cannot open its menu: Popover never re-runs its body"
        );
        assert_eq!(menu.get(), 0, "no menu entry ran");
        assert!(
            !scene.graph.recomposer.has_pending(),
            "the click leaves nothing pending, which is the tell"
        );
    });
}
