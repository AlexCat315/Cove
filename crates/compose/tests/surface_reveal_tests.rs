//! Translation of `Tests/CoveUIComposeTests/Transitions/SurfaceRevealTests.swift`.
//!
//! Behaviour checklist:
//! - a menu popover opens exactly one portal entry, for both trigger corners and
//!   with and without reduced motion
//! - reduced motion presents the menu with nothing animating
//! - the command row accepts a click while the surface is entering and runs its
//!   action exactly once, which closes the presentation
//! - an activated menu leaves no portal entry and, after the exit finishes, no
//!   running animation
//! - reduced motion tears the activated row down instead of keeping it in an
//!   exit state
//! - a context menu press leaves no portal entry and nothing animating
//!
//! Named differences from the Swift suite:
//! - `ContextMenuHost` installs its pointer route at
//!   `InputRoutingPriority::CHROME`; the secondary press consumes at target
//!   delivery, so the menu opens before any chrome sweep can swallow it.
//! - After activation Rust swaps the portal content synchronously, so the
//!   presenting row is detached. A detached subtree accepts no input, matching
//!   Swift's exiting host.
//! - Swift's `MenuItem` identity is a string; Rust menu rows are keyed by
//!   `u64`, so the command row is looked up as row id 0.

use crate::support;
use cove_composition::Scope;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View, ViewExt};
use cove_core::scene::{AnimatorScheduler, Node};
use cove_core::{Binding, Point, State};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent};
use cove_ui_compose::modifiers::layout::{AbsolutePositionModifier, LayoutFrameExt};
use cove_ui_compose::primitives::overlays::ContextMenu;
use cove_ui_compose::primitives::overlays::Popover;
use cove_ui_compose::primitives::overlays::anchored::{
    PopoverOverflow, PopoverPlacement, PopoverSurface,
};
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::InteractiveScene;

const MENU_ITEM_ID: &str = "__menu_item_id";

#[derive(Clone)]
struct MenuHarness {
    shown: State<bool>,
    actions: Rc<Cell<u32>>,
    trailing: bool,
    reduced: bool,
}

impl View for MenuHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let binding = Binding::new(
            {
                let shown = self.shown.clone();
                move || shown.get()
            },
            {
                let shown = self.shown.clone();
                move |value: bool| shown.set(value)
            },
        );
        let close = {
            let shown = self.shown.clone();
            move || shown.set(false)
        };
        let actions = self.actions.clone();
        let command = MenuItem::new(0, "Command", move || {
            actions.set(actions.get() + 1);
        });
        let menu = Menu::new(vec![MenuEntry::Item(command)])
            .width(180.0)
            .on_item_activated(close);
        let popover = Popover::new(Text::new("Open"), menu)
            .is_presented(binding)
            .width(180.0)
            .placement(if self.trailing {
                PopoverPlacement::BottomTrailing
            } else {
                PopoverPlacement::BottomLeading
            })
            .surface(PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .frame_size(80.0, 40.0)
            .modifier(AbsolutePositionModifier::new(
                Some(200.0),
                Some(80.0),
                None,
                None,
            ));
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(popover))).reduced_motion(self.reduced),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("shown", &self.shown)]
    }
}

fn settle(scene: &InteractiveScene) {
    for _ in 0..4 {
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(640.0, 400.0);
    }
}

/// The node the Swift suite identified as the reveal host: the one whose
/// transform is not the identity.
fn reveal_host(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| !node.transform().is_identity())
}

fn command_row(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| node.attachment::<u64>(MENU_ITEM_ID) == Some(0))
}

fn press(scene: &InteractiveScene, button: MouseButton, x: f32, y: f32) {
    let dispatcher = scene.dispatcher();
    let event = MouseButtonEvent {
        button,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
    settle(scene);
    dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
    settle(scene);
}

#[test]
fn menu_reveal() {
    for trailing in [false, true] {
        for reduced in [false, true] {
            InteractiveScene::run(640.0, 400.0, |scene| {
                let actions = Rc::new(Cell::new(0));
                let harness = MenuHarness {
                    shown: State::new(false),
                    actions: actions.clone(),
                    trailing,
                    reduced,
                };
                scene.install(harness.clone());
                settle(scene);
                harness.shown.set(true);
                settle(scene);

                if reduced {
                    assert!(
                        reveal_host(scene).is_none(),
                        "reduced motion presents the surface without a scale host (trailing={trailing})"
                    );
                } else {
                    let host = reveal_host(scene).expect("the surface grows from a transform host");
                    assert!(
                        (host.transform().scale - 0.94).abs() < 0.0001,
                        "the surface starts at a 0.94 scale, saw {} (trailing={trailing})",
                        host.transform().scale
                    );
                    assert_eq!(
                        host.transform().anchor,
                        Point::new(if trailing { 1.0 } else { 0.0 }, 0.0),
                        "the reveal is anchored at the trigger corner (trailing={trailing})"
                    );
                    assert!(
                        !host.transform().scales_shadow,
                        "the reveal does not scale elevation (trailing={trailing})"
                    );
                }
                let scheduler = AnimatorScheduler::current();
                if reduced {
                    assert_eq!(
                        scheduler.active_count(),
                        0,
                        "reduced motion leaves nothing animating (trailing={trailing})"
                    );
                } else {
                    assert_eq!(
                        scheduler.active_count(),
                        1,
                        "the entry transition is running (trailing={trailing})"
                    );
                }
                assert_eq!(
                    scene.portal.entries().len(),
                    1,
                    "the open menu owns one portal entry (trailing={trailing}, reduced={reduced})"
                );

                let command = command_row(scene).expect("the command row is mounted");
                let frame = command.absolute_frame();
                press(
                    scene,
                    MouseButton::Left,
                    frame.x + frame.width / 2.0,
                    frame.y + frame.height / 2.0,
                );

                assert_eq!(
                    actions.get(),
                    1,
                    "the command runs exactly once while the surface is entering"
                );
                assert!(
                    !harness.shown.get(),
                    "activating the command closes the menu"
                );
                assert_eq!(
                    scene.portal.entries().len(),
                    0,
                    "the activated menu owns no portal entry"
                );

                let mounted = scene.nodes().iter().any(|node| node.id() == command.id());
                if reduced {
                    assert!(
                        !mounted,
                        "reduced motion tears the activated row down instead of keeping it in an exit state"
                    );
                } else {
                    assert!(
                        !mounted,
                        "the activated row leaves the tree with the swapped portal content"
                    );
                    assert!(
                        !command.accepts_subtree_input(),
                        "the exiting row takes no input after activation"
                    );
                }

                scheduler.tick(1.0);
                settle(scene);
                assert_eq!(
                    scheduler.active_count(),
                    0,
                    "the exit finishes and nothing keeps animating"
                );
                assert!(
                    scene.portal.entries().is_empty(),
                    "no portal entry survives the exit"
                );
            });
        }
    }
}

#[test]
fn context_menu_reveal() {
    InteractiveScene::run(640.0, 400.0, |scene| {
        let actions = Rc::new(Cell::new(0));
        let probe = actions.clone();
        let content = AnyView::new(cove_composition::BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_size(100.0, 60.0);
        let menu = ContextMenu::new(
            move || {
                let probe = probe.clone();
                vec![MenuEntry::Item(MenuItem::new(0, "Command", move || {
                    probe.set(probe.get() + 1);
                }))]
            },
            content,
        )
        .width(180.0)
        .modifier(AbsolutePositionModifier::new(
            Some(80.0),
            Some(40.0),
            None,
            None,
        ));
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(menu))));
        settle(scene);

        press(scene, MouseButton::Right, 100.0, 60.0);
        let scheduler = AnimatorScheduler::current();
        let host = reveal_host(scene).expect("the menu grows from a transform host");
        assert!(
            (host.transform().scale - 0.94).abs() < 0.0001,
            "the menu starts at a 0.94 scale"
        );
        assert!(
            !host.transform().scales_shadow,
            "the reveal does not scale elevation"
        );
        let command = command_row(scene).expect("the command row is mounted");
        scheduler.tick(0.08);
        settle(scene);
        let frame = command.absolute_frame();
        press(
            scene,
            MouseButton::Left,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        );
        assert_eq!(actions.get(), 1, "the command runs");
        assert!(
            !command.accepts_subtree_input(),
            "the exiting row takes no input"
        );

        scheduler.tick(1.0);
        settle(scene);
        assert!(
            scene.portal.entries().is_empty(),
            "the context menu leaves no portal behind"
        );
        assert_eq!(
            scheduler.active_count(),
            0,
            "nothing keeps animating after the trigger is released"
        );
    });
}
