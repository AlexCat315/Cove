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
//! - `ContextMenuHost` installs its pointer route at
//!   `InputRoutingPriority::CHROME`, but a secondary press only consumes after
//!   target delivery, so the press still reaches the host and `on_open` runs.

use crate::support;
use cove_composition::Scope;

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
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::list::List;
use cove_ui_compose::primitives::data::lists::selection::Selection;
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
            move |item: &Item, _: RowState| {
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
        )
        .options(TreeOptions {
            selection: TreeSelection {
                ids: Selection::single(Binding::new(
                    {
                        let selection = selection.clone();
                        move || selection.get()
                    },
                    {
                        let selection = selection.clone();
                        move |value: Option<u64>| selection.set(value)
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        });

        // The column has to claim the window height, otherwise the tree below the
        // trigger has no space to lay out and its rows are never hit-testable.
        AnyView::new(LayerRoot::new(AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(popover),
                    AnyView::new(AnyView::new(tree).flex_with(1.0, 1.0, Some(0.0))),
                ])),
            )
            .flex_with(1.0, 1.0, None),
        )))
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
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "the secondary press opens the row's context menu"
        );
        assert_eq!(
            selection.get(),
            Some(1),
            "`on_open` moves the tree selection to the right-clicked row"
        );
        assert_eq!(probe.get(), 0, "opening the menu runs no context item yet");

        let action = item(scene, 2);
        scene.click(&action, 1);
        assert_eq!(
            probe.get(),
            1,
            "the context item's action runs once when activated"
        );
        assert!(
            scene.portal.entries().is_empty(),
            "the activated context menu leaves no portal behind"
        );
    });
}

#[test]
fn nested_button_in_a_tree_row_receives_its_own_clicks() {
    InteractiveScene::run(400.0, 300.0, |scene| {
        let probe = Rc::new(Cell::new(0));
        let action_probe = probe.clone();
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            Tree::new(
                vec![Item { id: 1 }],
                |item: &Item| item.id,
                |_item: &Item| Vec::<Item>::new(),
                move |_item: &Item, _: RowState| {
                    AnyView::new(Button::new(Text::new("Action"), {
                        let probe = action_probe.clone();
                        move || probe.set(probe.get() + 1)
                    }))
                },
            )
            .options(TreeOptions::default()),
        ))));

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
            List::new(vec![1], move |_id: &u64, _: RowState| {
                AnyView::new(Button::new(Text::new("Action"), {
                    let probe = action_probe.clone();
                    move || probe.set(probe.get() + 1)
                }))
            })
            .selection(Selection::single(Binding::constant(None))),
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
