//! Translation of `Tests/CoveUIComposeTests/ListMultipleSelectionTests.swift`.
//!
//! Behaviour checklist:
//! - a plain click selects one ID
//! - Shift-click extends the selection into a contiguous range
//! - Cmd-click removes a member and Cmd-clicking it again restores it
//! - Shift-arrow-down extends from the anchor, plain arrow-down collapses to the
//!   new row, and Cmd+A selects every row
//!
//! Model difference: Swift builds the list with `List(ids, id: \\.self,
//! selection:)` and a row closure; the Rust `List` takes a
//! `Selection` and a `rows(ids)` builder, so the harness passes
//! `Selection::multiple(..)` and `rows(vec![..])` and keeps the same
//! selection binding.

use crate::support;

use std::collections::BTreeSet;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, EventPhase, EventResult, FocusChain, Node};
use cove_core::{Binding, State};
use cove_platform::{KeyEvent, KeyModifiers, MouseButtonEvent};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::{List, Selection};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

const ROWS: [i32; 4] = [10, 20, 30, 40];

fn harness(selection: State<BTreeSet<i32>>) -> AnyView {
    AnyView::new(
        List::new(ROWS.to_vec(), |item: &i32, _: RowState| {
            let item = item.clone();
            AnyView::new(Text::new(format!("{item}")))
        })
        .selection(Selection::multiple(Binding::new(
            {
                let selection = selection.clone();
                move || selection.get()
            },
            {
                let selection = selection.clone();
                move |value| selection.set(value)
            },
        )))
        .frame_size(200.0, 500.0),
    )
}

fn rows(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
    })
}

/// Dispatches a click straight at the row's pointer handler, the Swift helper.
fn click(scene: &InteractiveScene, node: &Node, modifiers: KeyModifiers) {
    let frame = node.absolute_frame();
    let event = MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
        clicks: 1,
        modifiers,
        kind: Default::default(),
    };
    let handlers = scene.registry.handlers(node);
    let pointer = handlers.pointer.expect("rows install a pointer handler");
    let _ = pointer(
        &event,
        cove_core::scene::PointerPhase::Down,
        EventPhase::Target,
    );
    let _ = pointer(
        &event,
        cove_core::scene::PointerPhase::Up,
        EventPhase::Target,
    );
    scene.settle(1, 1.0);
}

fn set(values: &[i32]) -> BTreeSet<i32> {
    values.iter().copied().collect()
}

#[test]
fn click_shift_click_and_cmd_click_form_a_stable_id_set() {
    // `InteractiveScene::run` already holds the process-wide lock; nesting
    // `GlobalTestLock::locked` around it would self-deadlock.
    {
        let selection = State::new(BTreeSet::new());
        let observed = selection.clone();
        InteractiveScene::run(240.0, 600.0, |scene| {
            scene.install(harness(selection.clone()));
            let row_nodes = rows(scene);
            assert_eq!(row_nodes.len(), 4, "all four rows mount");

            click(scene, &row_nodes[1], KeyModifiers::default());
            assert_eq!(observed.get(), set(&[20]), "a plain click selects one row");

            click(scene, &row_nodes[3], KeyModifiers::LEFT_SHIFT);
            assert_eq!(
                observed.get(),
                set(&[20, 30, 40]),
                "shift-click extends from the anchor through the clicked row"
            );

            click(scene, &row_nodes[2], KeyModifiers::LEFT_GUI);
            assert_eq!(
                observed.get(),
                set(&[20, 40]),
                "cmd-click removes the clicked row from the selection"
            );

            click(scene, &row_nodes[2], KeyModifiers::LEFT_GUI);
            assert_eq!(
                observed.get(),
                set(&[20, 30, 40]),
                "cmd-clicking it again restores it"
            );
        });
    }
}

#[test]
fn keyboard_extends_from_the_anchor_and_selects_everything() {
    {
        let selection = State::new(BTreeSet::new());
        let observed = selection.clone();
        InteractiveScene::run(240.0, 600.0, |scene| {
            scene.install(harness(selection.clone()));
            let row_nodes = rows(scene);
            assert_eq!(row_nodes.len(), 4, "rows were mounted");

            click(scene, &row_nodes[1], KeyModifiers::default());
            assert_eq!(observed.get(), set(&[20]));

            let host = scene
                .nodes()
                .into_iter()
                .find(|node| scene.registry.handlers(node).key.is_some())
                .expect("the list owns a key host");
            let chain = std::rc::Rc::new(FocusChain::new());
            cove_core::scene::FocusChainHolder::set_current(Some(chain.clone()));
            chain.focus(Some(&host), false);
            let key = scene
                .registry
                .handlers(&host)
                .key
                .expect("the host handles keys");

            let key_event = |scancode: u32, modifiers: KeyModifiers| KeyEvent {
                scancode,
                keycode: 0,
                modifiers,
                is_repeat: false,
            };

            assert_eq!(
                key(
                    &key_event(Scancode::ARROW_DOWN, KeyModifiers::LEFT_SHIFT),
                    EventPhase::Target
                ),
                EventResult::Handled,
                "shift-arrow-down is handled by the list"
            );
            scene.settle(1, 1.0);
            assert_eq!(
                observed.get(),
                set(&[20, 30]),
                "shift-arrow-down extends the range from the anchor"
            );

            assert_eq!(
                key(
                    &key_event(Scancode::ARROW_DOWN, KeyModifiers::default()),
                    EventPhase::Target
                ),
                EventResult::Handled,
                "plain arrow-down is handled by the list"
            );
            scene.settle(1, 1.0);
            assert_eq!(
                observed.get(),
                set(&[40]),
                "plain arrow-down collapses the selection onto the new row"
            );

            assert_eq!(
                key(
                    &key_event(Scancode::A, KeyModifiers::LEFT_GUI),
                    EventPhase::Target
                ),
                EventResult::Handled,
                "cmd+A is handled by the list"
            );
            scene.settle(1, 1.0);
            assert_eq!(
                observed.get(),
                set(&[10, 20, 30, 40]),
                "cmd+A selects every row"
            );
        });
    }
}
