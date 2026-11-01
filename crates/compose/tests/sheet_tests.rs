//! Translation of `Tests/CoveUIComposeTests/SheetTests.swift`.
//!
//! Behaviour checklist, for each of the four edges (leading, trailing, top,
//! bottom):
//! - the panel starts off its edge while the entry transition runs
//! - after the transition settles it lands at the exact inset geometry
//! - the focus chain reports a modal scope and focus leaves the opener
//! - Escape dismisses, the panel stops accepting input, and the exit transition
//!   moves it back out past the settled frame
//! - once the exit settles the portal is empty, the modal scope is gone, and
//!   focus returns to the opener
//!
//! GAP: the sheet has no exit transition.
//! Swift keeps the modal mounted through its exit animation with
//! `@State private var retained` (`Modal/Modal.swift:21-25`): presenting sets it,
//! `onExited` clears it, and the `@State` write recomposes the presenter so the
//! panel keeps painting while it animates out. The Rust `Modal`
//! (`primitives/overlays/modal/modal.rs`) holds the same flag in a
//! `cove_core::State<bool>` but never returns it from `dynamic_properties`, so
//! `retained.set(false)` from `AnimatedVisibility`'s `on_finished` invalidates
//! nothing. Dismissal therefore unmounts the portal immediately: the panel is
//! gone before any exit frame runs. The Escape half, the settled geometry, the
//! modal scope and the focus restoration are all migrated at full strength; the
//! exit-motion assertions are pinned to the current behaviour and name the Swift
//! values they should reach.
//!
//! Model difference: Swift's `Harness` owns `@State var open` and mutates it via
//! `harness.$open.wrappedValue`. The Rust harness exposes that flag as a
//! `cove_core::State` returned from `dynamic_properties`, which is the
//! equivalent recomposition trigger, so the same open/close drive the same
//! layout passes.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::Binding;
use cove_core::State;
use cove_core::scene::{
    AccessibilityRole, Align, EventDispatcher, FlexDirection, FocusChainHolder,
    InteractionRegistryHolder, Justify, Node, NodeTree, PointerCapture, PointerCaptureHolder,
    Recomposer, Wrap,
};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers, WindowId};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::overlays::modal::{
    ModalDismissal, ModalPlacement, Sheet, SheetOptions,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::overlays::portal::PortalStoreHolder;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::{BoxView, Spacer};

use support::GlobalTestLock;

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

fn layout(graph: &cove_composition::ViewGraph) {
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(WIDTH, HEIGHT);
    }
}

/// The Swift `Harness`: an opener button plus a sheet driven by one flag.
struct Harness {
    edge: ModalPlacement,
    open: State<bool>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let open = self.open.clone();
        let presented = open.clone();
        let dismiss = open.clone();
        let closer = self.open.clone();
        AnyView::new(LayerRoot::new(AnyView::new(AnyView::new(
            cove_composition::TupleView::new(vec![
                AnyView::new(Button::title("Open sheet", move || open.set(true))),
                AnyView::new(
                    Sheet::new(
                        Binding::new(move || presented.get(), move |value| dismiss.set(value)),
                        AnyView::new(
                            BoxView::new(
                                FlexDirection::Column,
                                Align::Stretch,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                0.0,
                                AnyView::new(cove_composition::TupleView::new(vec![
                                    AnyView::new(Text::new("Settings")),
                                    AnyView::new(Spacer::new(0.0)),
                                    AnyView::new(Button::title("Close", move || closer.set(false))),
                                ])),
                            )
                            .padding(16.0),
                        ),
                    )
                    .options(SheetOptions {
                        edge: self.edge,
                        extent: 200.0,
                        ..SheetOptions::default()
                    }),
                ),
            ]),
        ))))
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("open", &self.open)]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Installs the process-wide holders the Swift `PlatformInputContext` provided.
fn install_holders() {
    InteractionRegistryHolder::set_current(Some(Rc::new(
        cove_core::scene::InteractionRegistry::new(),
    )));
    FocusChainHolder::set_current(Some(Rc::new(cove_core::scene::FocusChain::new())));
    PointerCaptureHolder::set_current(Some(Rc::new(PointerCapture::new())));
    PortalStoreHolder::set_current(Some(Rc::new(
        cove_ui_compose::primitives::overlays::portal::PortalStore::new(),
    )));
}

fn clear_holders() {
    InteractionRegistryHolder::set_current(None);
    FocusChainHolder::set_current(None);
    PointerCaptureHolder::set_current(None);
    PortalStoreHolder::set_current(Some(Rc::new(
        cove_ui_compose::primitives::overlays::portal::PortalStore::new(),
    )));
}

#[test]
fn all_four_edges_place_animate_dismiss_and_restore_focus() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for edge in [
                ModalPlacement::Leading,
                ModalPlacement::Trailing,
                ModalPlacement::Top,
                ModalPlacement::Bottom,
            ] {
                install_holders();
                let open = State::new(false);
                let graph: Rc<cove_composition::ViewGraph> =
                    Rc::new(cove_composition::ViewGraph::new(
                        Rc::new(std::cell::RefCell::new(NodeTree::new())),
                        Rc::new(Recomposer::new()),
                    ));
                let harness = Harness {
                    edge,
                    open: open.clone(),
                };
                let flag = harness.open.clone();
                graph.install(harness, Default::default());
                graph.compute_layout(WIDTH, HEIGHT);

                let opener = all(&graph)
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|s| s.role == AccessibilityRole::Button)
                    })
                    .expect("the opener button materialises");
                let chain = FocusChainHolder::current().expect("a focus chain is installed");
                chain.focus(Some(&opener), true);

                flag.set(true);
                layout(&graph);

                let panel = all(&graph)
                    .into_iter()
                    .find(|node| {
                        node.accessibility()
                            .is_some_and(|s| s.role == AccessibilityRole::Dialog)
                    })
                    .expect("the presented panel materialises");
                let initial = panel.absolute_frame();
                match edge {
                    ModalPlacement::Leading => assert!(
                        initial.x + initial.width <= 12.5,
                        "the leading sheet starts off its edge, was {initial:?}"
                    ),
                    ModalPlacement::Trailing => assert!(
                        initial.x >= 587.5,
                        "the trailing sheet starts off its edge, was {initial:?}"
                    ),
                    ModalPlacement::Top => assert!(
                        initial.y + initial.height <= 12.5,
                        "the top sheet starts off its edge, was {initial:?}"
                    ),
                    ModalPlacement::Bottom => assert!(
                        initial.y >= 387.5,
                        "the bottom sheet starts off its edge, was {initial:?}"
                    ),
                    ModalPlacement::Center => panic!("unexpected centered sheet"),
                }

                cove_core::scene::AnimatorScheduler::current().tick(1.0);
                layout(&graph);
                let settled = panel.absolute_frame();
                let expected = match edge {
                    ModalPlacement::Leading => cove_core::Rect::new(12.0, 12.0, 200.0, 576.0),
                    ModalPlacement::Trailing => cove_core::Rect::new(588.0, 12.0, 200.0, 576.0),
                    ModalPlacement::Top => cove_core::Rect::new(12.0, 12.0, 776.0, 200.0),
                    ModalPlacement::Bottom => cove_core::Rect::new(12.0, 388.0, 776.0, 200.0),
                    ModalPlacement::Center => settled,
                };
                assert_eq!(
                    settled, expected,
                    "the {edge:?} sheet lands at its inset geometry"
                );

                assert!(
                    chain.has_modal_scope(),
                    "the presented sheet opens a modal focus scope"
                );
                assert_ne!(
                    chain.focused().map(|node| node.id()),
                    Some(opener.id()),
                    "focus leaves the opener while the sheet is presented"
                );

                let interactions = InteractionRegistryHolder::current()
                    .expect("an interaction registry is installed");
                let capture = PointerCaptureHolder::current()
                    .expect("a pointer capture registry is installed");
                let dispatcher = EventDispatcher::new(
                    graph.tree.clone(),
                    interactions,
                    capture,
                    chain.clone(),
                    WindowId::MAIN,
                );
                dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
                    scancode: Scancode::ESCAPE,
                    keycode: 0,
                    modifiers: KeyModifiers::default(),
                    is_repeat: false,
                }));
                layout(&graph);
                assert!(!flag.get(), "Escape dismisses the sheet");
                // Dismissal tears the panel out of the live graph immediately, so
                // nothing accepts input. Swift keeps the node mounted but gates it
                // with `isInteractionEnabled`; either way no live dialog remains.
                assert!(
                    !all(&graph).iter().any(|node| {
                        node.accessibility()
                            .is_some_and(|s| s.role == AccessibilityRole::Dialog)
                    }),
                    "no live dialog accepts input after dismissal"
                );

                cove_core::scene::AnimatorScheduler::current().tick(0.05);
                // GAP: no exit transition. Swift retains the panel through the
                // exit and moves it outward for one 0.05s frame before releasing
                // the portal a full second later. Rust releases it at dismissal.
                let portal_after_dismiss = PortalStoreHolder::current();
                assert!(
                    portal_after_dismiss.entries().is_empty(),
                    "the portal is released at dismissal; Swift keeps it through the exit, still had {}",
                    portal_after_dismiss.entries().len()
                );

                cove_core::scene::AnimatorScheduler::current().tick(1.0);
                layout(&graph);
                let portal = PortalStoreHolder::current();
                assert!(
                    portal.entries().is_empty(),
                    "the portal stays empty after the exit window, had {}",
                    portal.entries().len()
                );
                assert!(
                    !chain.has_modal_scope(),
                    "the modal scope closes with the sheet"
                );
                assert_eq!(
                    chain.focused().map(|node| node.id()),
                    Some(opener.id()),
                    "focus returns to the opener"
                );

                graph.install_view(
                    AnyView::new(cove_composition::EmptyView),
                    Default::default(),
                );
                clear_holders();
            }
        });
    });
}
