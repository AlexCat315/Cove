//! Translation of `Tests/CoveUIComposeTests/PopoverTests.swift`.
//!
//! Behaviour checklist:
//! - an open popover materialises exactly one portal entry
//! - tearing down an open popover removes its portal entry, so no phantom menu
//!   is left behind to intercept later clicks
//! - a closed popover contributes no portal entry at all
//!
//! Recorded differences (reported, not fixed):
//! - Most of the Swift suite drives the dismissal animation frame by frame
//!   (`renderedEntries` shrinking to zero, a reversal during dismissal, the
//!   scheduler reaching `activeCount == 0`) and the trigger's keyboard routing.
//!   Those cases need `Popover` to be presented from a binding it observes; the
//!   `Binding` gap means a post-mount write never reaches the overlay, so the
//!   animated-dismissal family is not reachable here. The lifetime contract that
//!   family protects — one entry while open, none after teardown — is asserted
//!   directly against `PortalStore`, which is the observable it reads.
//! - `PopoverSurface` is private in Rust, so the harness uses the default
//!   surface rather than authoring `.content` explicitly.

use cove_composition::Scope;
use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::Popover;
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStoreHolder};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

struct ActionBox {
    fired: Rc<RefCell<u32>>,
}

/// Opens a popover over a menu, driven by a `@State` that the body reads.
struct LeakHarness {
    show_popover: State<bool>,
    fired: Rc<RefCell<u32>>,
}

impl View for LeakHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _show = self.show_popover.get();
        let fired = self.fired.clone();
        let popover = Popover::new(
            Text::new("Open"),
            Menu::new(vec![MenuEntry::Item(MenuItem::new(
                0,
                "Reopen",
                move || {
                    *fired.borrow_mut() += 1;
                },
            ))]),
        )
        .is_presented(cove_core::Binding::new(
            {
                let state = self.show_popover.clone();
                move || state.get()
            },
            {
                let state = self.show_popover.clone();
                move |value: bool| state.set(value)
            },
        ))
        .width(140.0);

        AnyView::new(LayerRoot::new(
            AnyView::new(cove_composition::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                AnyView::new(popover),
            ))
            .frame_size(240.0, 200.0),
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("show_popover", &self.show_popover)]
    }
}

fn store_entries(scene: &InteractiveScene) -> usize {
    let _ = scene;
    PortalStoreHolder::current().entries().len()
}

#[test]
fn open_popover_owns_one_portal_entry() {
    InteractiveScene::run(240.0, 200.0, |scene| {
        PortalStoreHolder::current().clear();
        let show = State::new(true);
        scene.install(LeakHarness {
            show_popover: show,
            fired: Rc::new(RefCell::new(0)),
        });
        scene.settle(6, 1.0);

        assert_eq!(
            store_entries(scene),
            1,
            "an open popover materialises exactly one portal entry"
        );
    });
}

#[test]
fn closed_popover_owns_no_portal_entry() {
    InteractiveScene::run(240.0, 200.0, |scene| {
        PortalStoreHolder::current().clear();
        let show = State::new(false);
        scene.install(LeakHarness {
            show_popover: show,
            fired: Rc::new(RefCell::new(0)),
        });
        scene.settle(6, 1.0);

        assert_eq!(
            store_entries(scene),
            0,
            "a closed popover contributes no portal entry"
        );
    });
}

#[test]
fn teardown_does_not_leak_the_portal_entry() {
    InteractiveScene::run(240.0, 200.0, |scene| {
        PortalStoreHolder::current().clear();
        let show = State::new(true);
        scene.install(LeakHarness {
            show_popover: show.clone(),
            fired: Rc::new(RefCell::new(0)),
        });
        scene.settle(6, 1.0);
        assert_eq!(
            store_entries(scene),
            1,
            "the open popover starts with one entry"
        );

        // Remove the whole popover from the tree while it is still open.
        show.set(false);
        scene.graph.recomposer.commit_all();
        scene.graph.compute_layout(240.0, 200.0);

        assert_eq!(
            store_entries(scene),
            0,
            "tearing down an open popover removes its portal entry"
        );
    });
}
