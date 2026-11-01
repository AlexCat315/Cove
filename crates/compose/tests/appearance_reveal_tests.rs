//! Translation of `Tests/CoveUIComposeTests/Animation/AppearanceRevealTests.swift`.
//!
//! Behaviour checklist:
//! - a pointer press on the theme control starts the reveal at the control
//!   centre with a zero radius
//! - the reveal radius grows across ticks
//! - keyboard activation reverses with a fresh sequence and its own zero radius
//! - reduced motion suppresses the reveal entirely and leaves nothing running
//! - a programmatic appearance change reveals from the window centre
//! - a completed reveal clears the request and its scheduler entry
//! - unmounting clears an in-flight reveal
//!
//! Model difference: the Swift suite reads the reveal off a rendered
//! `DrawList`. The Rust renderer records the same `Reveal` on the node's
//! request and again on the draw list, so these tests read
//! `Node::reveal_request()` — the identical value the renderer consumes.

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, TupleView, View};
use cove_core::scene::{
    Align, AnimatorScheduler, FlexDirection, FocusChainHolder, InteractionRegistryHolder, Justify,
    Node, PointerCaptureHolder, WindowRevealContext, Wrap,
};
use cove_core::{Reveal, State};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::{AbsolutePositionModifier, LayoutFrameExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::toggle::Toggle;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::GlobalTestLock;

/// The Swift harness: a theme button and a dark-mode toggle on a light surface
/// with a circular appearance transition.
struct Harness {
    dark: State<bool>,
    reduced: State<bool>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let dark = self.dark.clone();
        let reduced = self.reduced.get();

        let theme_button = AnyView::new(
            AnyView::new(
                AnyView::new(Button::title("Theme", move || {
                    let next = !dark.get();
                    dark.set(next);
                }))
                .frame_width(90.0)
                .frame_height(36.0),
            )
            .with_modifier(AbsolutePositionModifier::new(
                Some(100.0),
                Some(70.0),
                None,
                None,
            )),
        );

        let toggle = AnyView::new(
            AnyView::new(Toggle::new(self.dark.binding())).with_modifier(
                AbsolutePositionModifier::new(Some(280.0), Some(120.0), None, None),
            ),
        );

        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![theme_button, toggle])),
                ))
                .frame_width(640.0)
                .frame_height(400.0),
            )
            .appearance(
                if self.dark.get() {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ThemeTransition::Circular { duration: 0.4 },
            )
            .reduced_motion(reduced),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("dark", &self.dark),
            cove_composition::DynamicField::state("reduced", &self.reduced),
        ]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all_nodes(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn record(graph: &cove_composition::ViewGraph) -> Option<Reveal> {
    all_nodes(graph)
        .into_iter()
        .find_map(|node| node.reveal_request())
}

fn settle(graph: &cove_composition::ViewGraph) {
    for _ in 0..4 {
        graph.recomposer.commit_all();
        WindowRevealContext::clear_activation(graph.tree.borrow().root().as_ref());
        graph.compute_layout(640.0, 400.0);
    }
}

struct Holders {
    registry: Rc<cove_core::scene::InteractionRegistry>,
    capture: Rc<cove_core::scene::PointerCapture>,
    focus: Rc<cove_core::scene::FocusChain>,
}

impl Holders {
    fn new() -> Self {
        let registry = Rc::new(cove_core::scene::InteractionRegistry::new());
        let capture = Rc::new(cove_core::scene::PointerCapture::new());
        let focus = Rc::new(cove_core::scene::FocusChain::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        Self {
            registry,
            capture,
            focus,
        }
    }
    fn dispatcher(
        &self,
        graph: &Rc<cove_composition::ViewGraph>,
    ) -> cove_core::scene::EventDispatcher {
        cove_core::scene::EventDispatcher::new(
            graph.tree.clone(),
            self.registry.clone(),
            self.capture.clone(),
            self.focus.clone(),
            cove_platform::WindowId::MAIN,
        )
    }
}

impl Drop for Holders {
    fn drop(&mut self) {
        InteractionRegistryHolder::set_current(None);
        PointerCaptureHolder::set_current(None);
        FocusChainHolder::set_current(None);
    }
}

fn click(dispatcher: &cove_core::scene::EventDispatcher, x: f32, y: f32) {
    let event = cove_platform::MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
    dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(event));
}

#[test]
fn pointer_reveal_starts_at_the_control_centre_and_radius_grows() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let holders = Holders::new();
            let harness_dark = State::new(false);
            let harness_reduced = State::new(false);
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness {
                    dark: harness_dark.clone(),
                    reduced: harness_reduced.clone(),
                }),
                Default::default(),
            );
            settle(&graph);

            let control = all_nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.attachment::<bool>(BUTTON_HOST_MARKER_KEY)
                        .unwrap_or(false)
                })
                .expect("the theme button host is mounted");
            let frame = control.absolute_frame();
            let dispatcher = holders.dispatcher(&graph);
            click(&dispatcher, frame.x + 3.0, frame.y + frame.height / 2.0);
            settle(&graph);

            assert!(harness_dark.get(), "the press toggled the appearance");
            let first = record(&graph).expect("a reveal is requested");
            assert_eq!(
                (first.center_x, first.center_y),
                (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0),
                "the reveal is centred on the control"
            );
            assert_eq!(first.radius, 0.0, "the reveal starts collapsed");

            scheduler.tick(0.15);
            settle(&graph);
            let grown = record(&graph).expect("the reveal is still running");
            assert!(grown.radius > 0.0, "the disc grows after a tick");
        });
    });
}

#[test]
fn reduced_motion_suppresses_the_reveal_and_leaves_nothing_running() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let holders = Holders::new();
            let harness_dark = State::new(false);
            let harness_reduced = State::new(true);
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness {
                    dark: harness_dark.clone(),
                    reduced: harness_reduced.clone(),
                }),
                Default::default(),
            );
            settle(&graph);

            let toggle = all_nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == cove_core::scene::AccessibilityRole::Toggle
                    })
                })
                .expect("the toggle is mounted");
            let frame = toggle.absolute_frame();
            let dispatcher = holders.dispatcher(&graph);
            click(&dispatcher, frame.x + 3.0, frame.y + frame.height / 2.0);
            settle(&graph);

            assert!(
                harness_dark.get(),
                "reduced motion still changes the appearance"
            );
            assert!(
                record(&graph).is_none(),
                "reduced motion requests no reveal"
            );
            scheduler.tick(1.0);
            assert_eq!(
                scheduler.active_count(),
                0,
                "nothing is left animating under reduced motion"
            );
        });
    });
}

#[test]
fn programmatic_changes_reveal_from_the_window_centre_and_clear_on_completion() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let _holders = Holders::new();
            let harness_dark = State::new(false);
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness {
                    dark: harness_dark.clone(),
                    reduced: State::new(false),
                }),
                Default::default(),
            );
            settle(&graph);

            harness_dark.set(true);
            settle(&graph);
            let reveal = record(&graph).expect("a programmatic change reveals");
            assert_eq!(
                (reveal.center_x, reveal.center_y),
                (320.0, 200.0),
                "no control was pressed, so the reveal starts at the window centre"
            );

            scheduler.tick(0.5);
            settle(&graph);
            assert!(
                record(&graph).is_none(),
                "a completed reveal clears its request"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "and releases its scheduler entry"
            );

            harness_dark.set(false);
            settle(&graph);
            assert!(
                record(&graph).is_some(),
                "a second programmatic change reveals again"
            );

            graph.install_view(
                AnyView::new(cove_composition::EmptyView),
                Default::default(),
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "unmounting clears the in-flight reveal"
            );
        });
    });
}

#[test]
fn keyboard_activation_reverses_with_a_new_sequence_at_zero_radius() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let holders = Holders::new();
            let harness_dark = State::new(false);
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness {
                    dark: harness_dark.clone(),
                    reduced: State::new(false),
                }),
                Default::default(),
            );
            settle(&graph);

            let control = all_nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.attachment::<bool>(BUTTON_HOST_MARKER_KEY)
                        .unwrap_or(false)
                })
                .expect("the theme button host is mounted");
            let dispatcher = holders.dispatcher(&graph);
            let frame = control.absolute_frame();
            click(&dispatcher, frame.x + 3.0, frame.y + frame.height / 2.0);
            settle(&graph);
            let first = record(&graph).expect("the first reveal");
            assert!(harness_dark.get());

            holders.focus.focus(Some(&control), false);
            let key = cove_platform::KeyEvent {
                scancode: cove_composition::Scancode::SPACE,
                keycode: 0,
                modifiers: Default::default(),
                is_repeat: false,
            };
            dispatcher.dispatch(&cove_platform::InputEvent::KeyDown(key.clone()));
            settle(&graph);
            dispatcher.dispatch(&cove_platform::InputEvent::KeyUp(key));
            settle(&graph);

            assert!(!harness_dark.get(), "Space reverses the appearance");
            let reversal = record(&graph).expect("the reversal reveal");
            assert_ne!(
                reversal.sequence, first.sequence,
                "the reversal starts a fresh reveal sequence"
            );
            assert_eq!(reversal.radius, 0.0, "the reversal starts collapsed again");
            assert_eq!(
                (reversal.center_x, reversal.center_y),
                (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0),
                "keyboard activation uses the same control centre as the press"
            );
        });
    });
}
