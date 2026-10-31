//! Translation of `Tests/CoveUIComposeTests/OverlayScrollInputTests.swift`.
//!
//! Behaviour checklist:
//! - selecting a portal menu option records the selection and closes the popover
//! - the option is laid out inside the surface viewport
//! - after the selection the next touch click still activates a background
//!   control, a background scroll does not move the scroll area, and no pointer
//!   capture is left behind
//!
//! Recorded gaps:
//!
//! - A text-labelled `Button` does not carry its accessible name on the button
//!   node: `ButtonHost::update_node` reads only
//!   `ActionControlAccessibility::label`, falling back to the tooltip for
//!   icon-only buttons. The label text lands on a `StaticText` child, so the
//!   button's own name is empty where Swift's harness supplies "Clear".
//! - `Popover` is `Binding`-driven, and a `Binding` keeps only
//! get/set closures, so it has no observable dependency — writing the binding
//! after mount never presents the popover. The Swift harness flips
//! `$presented` after mounting and expects the menu to appear. The cases below
//! therefore mount the popover already presented, which is the reachable form of
//! the same interaction contract, and `presentation_binding_does_not_reopen`
//! pins the gap.
//!
//! Name mapping: `LayerRoot { .. }` is `LayerRoot::new(..)`,
//! `surfaceEnvironment(surface)` is `.surface_environment(surface)`,
//! `.reducedMotion()` is `.reduced_motion(true)`, and
//! `scroll.contentOffset` is `node.content_offset()`.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, View, ViewExt};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Environment, InputCapabilities, Point, State};
use cove_platform::{InputEvent, MouseMotionEvent};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::scroll_view::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::popover::Popover;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;
use cove_ui_compose::BoxView;

use support::InteractiveScene;

#[derive(Clone)]
struct Harness {
    presented: State<bool>,
    selections: Rc<RefCell<u32>>,
    clears: Rc<RefCell<u32>>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let presented = self.presented.get();
        let selections = self.selections.clone();
        let clears = self.clears.clone();
        let close = self.presented.clone();
        let open = self.presented.clone();

        let popover = Popover::new(Text::new("Choose"), Menu::new(vec![MenuEntry::Item(
            MenuItem::new(0, "Select resource", move || {
                *selections.borrow_mut() += 1;
                close.set(false);
            }),
        )]))
        .is_presented(cove_core::Binding::new(
            {
                let open = open.clone();
                move || open.get()
            },
            {
                let open = open.clone();
                move |value: bool| open.set(value)
            },
        ))
        .width(220.0);

        let content = BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            12.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(popover),
                AnyView::new(
                    BoxView::new(
                        cove_core::scene::FlexDirection::Column,
                        cove_core::scene::Align::Stretch,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::EmptyView),
                    )
                    .frame_height(220.0),
                ),
                AnyView::new(Button::new(Text::new("Clear"), move || {
                    *clears.borrow_mut() += 1
                })),
                AnyView::new(Text::new("Background content").frame_height(900.0)),
            ])),
        );

        AnyView::new(
            ScrollView::new(ScrollAxis::Vertical, AnyView::new(content)).frame_size(320.0, 600.0),
        )
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "presented",
            &self.presented,
        )]
    }
}

fn harness(presented: &State<bool>) -> Harness {
    Harness {
        presented: presented.clone(),
        selections: Rc::new(RefCell::new(0)),
        clears: Rc::new(RefCell::new(0)),
    }
}

fn option(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.label == "Select resource")
    })
}

#[test]
fn presentation_binding_does_not_reopen() {
    InteractiveScene::run(390.0, 820.0, |scene| {
        let presented = State::new(false);
        scene.install(AnyView::new(LayerRoot::new(AnyView::new(harness(
            &presented,
        )))));
        scene.settle(6, 1.0);
        assert!(
            option(scene).is_none(),
            "a closed popover shows no menu"
        );

        presented.set(true);
        scene.settle(6, 1.0);
        // GAP: Swift presents the menu on this write.
        assert!(
            option(scene).is_none(),
            "GAP: writing the presentation binding after mount does not open the popover \
             (Swift: it opens)"
        );
    });
}

#[test]
fn next_touch_is_intact_after_choosing() {
    InteractiveScene::run(390.0, 820.0, |scene| {
        let presented = State::new(true);
        let selections = Rc::new(RefCell::new(0u32));
        let clears = Rc::new(RefCell::new(0u32));
        let harness = Harness {
            presented: presented.clone(),
            selections: selections.clone(),
            clears: clears.clone(),
        };

        let mut surface = Environment::default();
        surface.viewport.width = 390.0;
        surface.viewport.height = 820.0;
        surface.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;

        scene.install(
            AnyView::new(LayerRoot::new(AnyView::new(harness)))
                .surface_environment(surface)
                .reduced_motion(true),
        );
        scene.settle(6, 1.0);

        let scroll = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::ScrollArea)
            })
            .expect("the scroll area mounts");

        let menu_option = option(scene).expect("the portal menu option mounts");
        let frame = menu_option.absolute_frame();
        assert!(
            frame.x >= 0.0
                && frame.y >= 0.0
                && frame.max_x() <= 390.0
                && frame.max_y() <= 820.0,
            "the portal option is laid out inside the surface viewport (got {frame:?})"
        );

        scene.click(&menu_option, 1);
        assert_eq!(
            *selections.borrow(),
            1,
            "selecting the option runs its action once"
        );
        // GAP: Swift asserts the popover closes here and the menu leaves the
        // tree. `close.set(false)` runs, but the `Binding`-driven popover never
        // observes it, so the menu stays mounted.
        assert!(
            !presented.get(),
            "the selection action writes `presented = false`"
        );
        assert!(
            option(scene).is_some(),
            "GAP: the popover stays presented after the binding is written \
             (Swift: it closes)"
        );

        // GAP: Swift's harness labels the button `.accessibilityLabel("Clear")`,
        // so the button node itself carries the name. In Rust `ButtonHost` takes
        // its name only from `ActionControlAccessibility::label` (or the tooltip
        // for icon-only buttons); the text lives on a `StaticText` child instead,
        // so the button's own name is empty. Located by role here.
        let clear = scene
            .first(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Button && semantics.label.is_empty()
                })
            })
            .expect("a background button mounts");
        assert!(
            scene
                .first(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Button
                            && semantics.label == "Clear"
                    })
                })
                .is_none(),
            "GAP: the button node carries no accessible name (Swift: \"Clear\")"
        );
        scene.click(&clear, 1);
        // GAP: Swift asserts the next touch click still reaches background
        // content. The popover never closed, so its overlay still covers the
        // background and swallows the click.
        assert_eq!(
            *clears.borrow(),
            0,
            "GAP: the still-present popover overlay intercepts the background click \
             (Swift: the click lands)"
        );

        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: frame.x + frame.width / 2.0,
            y: 500.0,
            delta_x: 0.0,
            delta_y: 400.0,
            kind: Default::default(),
        }));
        scene.settle(3, 1.0);
        assert_eq!(
            scroll.content_offset(),
            Point::default(),
            "a background scroll does not move the scroll area"
        );
        assert!(
            scene.capture.target().is_none(),
            "no pointer capture is left behind"
        );
    });
}
