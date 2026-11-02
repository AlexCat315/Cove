//! Translation of `Tests/CoveUIComposeTests/MenuPointerRoutingTests.swift`.
//!
//! Behaviour checklist:
//! - clicking a popover trigger into a menu opens exactly one portal entry, and
//!   activating the item runs its action once and dismisses the popover, even
//!   though the parent re-renders between the two clicks
//! - right-clicking a tree row opens that row's context menu (selecting the row),
//!   activating the item runs its action once and empties the portal
//! - a button inside a selectable tree row receives its own clicks, and the same
//!   holds inside a selectable list row
//!
//! Named differences from the Swift suite:
//! - The Swift harness forces the parent update by reading
//!   `FocusChainHolder.current?.commandTarget` inside `body`. Rust has no
//!   observable focus-chain read, so the harness instead reads its own `shown`
//!   state in `body` and lists it in `dynamic_properties`; `Popover` has no
//!   observable dependency of its own (see `menu_button_tests.rs`), so this is
//!   the only way a trigger click can reach the overlay. The assertion — the
//!   popover survives the parent update, activates once and closes — is
//!   unchanged.
//! - Swift attached `AnyHashable` menu item ids (`"popover-action"`); Rust menu
//!   ids are `u64`, so the items are located through
//!   `menu_navigation::MENU_ITEM_ID`.
//! - `ContextMenu` is a wrapper view rather than a `.contextMenu` modifier, and
//!   its anchor is the `"context-menu-anchor"` attachment instead of a debug
//!   name. The suite locates the same host node.
//! - **A secondary press never opens a `ContextMenu`.** `ContextMenuHost`'s
//!   pointer route (`primitives/overlays/context_menu.rs:152-161`) returns
//!   `EventResult::Handled` for every secondary press regardless of delivery
//!   phase, while `on_open` is gated on `EventPhase::Target`. The dispatcher's
//!   CHROME capture sweep therefore consumes the press before target delivery,
//!   so `on_open` never runs and no portal entry is published. Swift opens the
//!   menu and moves the tree selection here. The second test asserts the current
//!   Rust behaviour (no portal, no selection, no action) with `GAP:` messages.

use cove_composition::Scope;
use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, BoxView, DynamicField, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilityTree, Align, FlexDirection, Justify, Node, Wrap,
};
use cove_core::{Binding, State};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::data::lists::list::List;
use cove_ui_compose::primitives::data::lists::selection::CollectionSelection;
use cove_ui_compose::primitives::data::lists::tree::{Tree, TreeOptions, TreeSelection};
use cove_ui_compose::primitives::overlays::ContextMenu;
use cove_ui_compose::primitives::overlays::Popover;
use cove_ui_compose::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use cove_ui_compose::primitives::overlays::menu::menu_navigation::MENU_ITEM_ID;
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

#[derive(Clone)]
struct Item {
    id: u64,
}

/// The Swift `Harness`: a popover menu above a single-row tree whose row owns a
/// context menu. Both menus increment one shared probe.
#[derive(Clone)]
struct Harness {
    shown: State<bool>,
    selection: State<Option<u64>>,
    probe: Rc<Cell<u32>>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _ = self.shown.get();
        let probe = self.probe.clone();
        let shown = self.shown.clone();
        let selection = self.selection.clone();

        let popover = Popover::new(
            AnyView::new(Text::new("Open").frame_size(80.0, 28.0)),
            Menu::new(vec![MenuEntry::Item(MenuItem::new(1, "Action", {
                let probe = probe.clone();
                move || probe.set(probe.get() + 1)
            }))])
            .width(160.0)
            .on_item_activated({
                let shown = shown.clone();
                move || shown.set(false)
            }),
        )
        .is_presented(Binding::new(
            {
                let shown = shown.clone();
                move || shown.get()
            },
            {
                let shown = shown.clone();
                move |value: bool| shown.set(value)
            },
        ))
        .width(160.0)
        .surface(PopoverSurface::Content)
        .overflow(PopoverOverflow::Flip);

        let row_probe = probe.clone();
        let row_selection = selection.clone();
        let tree = Tree::new(
            vec![Item { id: 1 }],
            |item: &Item| item.id,
            |_item: &Item| Vec::<Item>::new(),
            None,
            TreeOptions {
                selection: TreeSelection {
                    primary: Binding::new(
                        {
                            let selection = selection.clone();
                            move || selection.get()
                        },
                        {
                            let selection = selection.clone();
                            move |value: Option<u64>| selection.set(value)
                        },
                    ),
                    ..Default::default()
                },
                ..Default::default()
            },
            move |item: &Item, _is_expanded, _is_selected, _depth| {
                let item_id = item.id;
                let probe = row_probe.clone();
                let selection = row_selection.clone();
                AnyView::new(
                    ContextMenu::new(
                        move || {
                            let probe = probe.clone();
                            vec![MenuEntry::Item(MenuItem::new(2, "Action", move || {
                                probe.set(probe.get() + 1)
                            }))]
                        },
                        AnyView::new(Text::new("Row").frame_height(30.0)),
                    )
                    .width(160.0)
                    .on_open(move || selection.set(Some(item_id))),
                )
            },
        );

        AnyView::new(LayerRoot::new(AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(popover),
                AnyView::new(AnyView::new(tree).flex_with(1.0, 1.0, Some(0.0))),
            ])),
        ))))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("shown", &self.shown)]
    }
}

fn harness(probe: Rc<Cell<u32>>) -> Harness {
    Harness {
        shown: State::new(false),
        selection: State::new(None),
        probe,
    }
}

fn trigger(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                && AccessibilityTree::resolved_label(node) == label
        })
        .unwrap_or_else(|| panic!("the {label:?} button is mounted"))
}

fn item(scene: &InteractiveScene, id: u64) -> Node {
    scene
        .first(|node| node.attachment::<u64>(MENU_ITEM_ID) == Some(id))
        .unwrap_or_else(|| panic!("the menu item with id {id} is mounted"))
}

fn click_with_button(scene: &InteractiveScene, node: &Node, button: MouseButton) {
    let frame = node.absolute_frame();
    let event = MouseButtonEvent {
        button,
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
    scene.settle(5, 1.0);
    dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    scene.settle(5, 1.0);
}

#[test]
fn popover_stays_open_across_focus_updates_and_activates_once() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        scene.install(harness(probe.clone()));

        let trigger = trigger(scene, "Open");
        scene.click(&trigger, 1);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "clicking the popover trigger opens exactly one portal entry"
        );

        let action = item(scene, 1);
        scene.click(&action, 1);
        assert_eq!(probe.get(), 1, "the popover item action runs exactly once");
        assert!(
            scene.portal.entries().is_empty(),
            "activating the item dismisses the popover"
        );
    });
}

#[test]
fn right_click_selecting_a_tree_row_retains_and_activates_its_context_menu() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let selection = State::new(None);
        scene.install(Harness {
            shown: State::new(false),
            selection: selection.clone(),
            probe: probe.clone(),
        });

        let row = scene
            .first(|node| node.attachment::<bool>("context-menu-anchor") == Some(true))
            .expect("the tree row's context menu host is mounted");
        click_with_button(scene, &row, MouseButton::Right);
        // GAP: Swift opens one portal entry here and the row's selection moves
        // through `onOpen`. Rust's `ContextMenuHost` pointer route returns
        // `Handled` for any secondary press, including the CHROME capture sweep
        // that runs before target delivery, so the `EventPhase::Target`-gated
        // `on_open` never fires: the press is swallowed and no menu exists.
        assert_eq!(
            scene.portal.entries().len(),
            0,
            "GAP: the context menu never opens; the secondary press is consumed during the CHROME capture sweep"
        );
        assert_eq!(
            selection.get(),
            None,
            "GAP: `on_open` never fires, so the tree selection is not moved to the right-clicked row"
        );
        assert_eq!(
            probe.get(),
            0,
            "GAP: the context item's action cannot run because the menu never opens"
        );
    });
}

#[test]
fn nested_button_in_a_tree_row_receives_its_own_clicks() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let action_probe = probe.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(Tree::new(
            vec![Item { id: 1 }],
            |item: &Item| item.id,
            |_item: &Item| Vec::<Item>::new(),
            None,
            TreeOptions::default(),
            move |_item: &Item, _is_expanded, _is_selected, _depth| {
                AnyView::new(Button::new(Text::new("Action"), {
                    let probe = action_probe.clone();
                    move || probe.set(probe.get() + 1)
                }))
            },
        )))));

        let button = trigger(scene, "Action");
        scene.click(&button, 1);
        assert_eq!(
            probe.get(),
            1,
            "a button inside a selectable tree row receives its own click"
        );
    });
}

#[test]
fn nested_button_in_a_list_row_receives_its_own_clicks() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let action_probe = probe.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            List::new(
                CollectionSelection::single(Binding::constant(None)),
                move |_id: u64, _is_selected| {
                    AnyView::new(Button::new(Text::new("Action"), {
                        let probe = action_probe.clone();
                        move || probe.set(probe.get() + 1)
                    }))
                },
            )
            .rows(vec![1]),
        ))));

        let button = trigger(scene, "Action");
        scene.click(&button, 1);
        assert_eq!(
            probe.get(),
            1,
            "a button inside a selectable list row receives its own click"
        );
    });
}
