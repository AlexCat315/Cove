//! Translation of `Tests/CoveUIComposeTests/Adaptive/DisclosureTests.swift`.
//!
//! Behaviour checklist:
//! - the long header keeps exactly one measured line and stays inside the
//!   authored 220pt column at every width, text scale and theme
//! - the header text publishes its accessibility label
//! - the accordion and the disclosure group each expose one button header
//! - headers keep a 44pt touch height outside the pointer-only width
//! - keyboard activation of both headers flips the accordion expansion and the
//!   disclosure state
//!
//! Named differences from the Swift suite:
//! - **The leading column is expressed as `Align::Stretch`.** A Rust flex column
//!   with `Align::FlexStart` hands children their intrinsic cross width, so an
//!   in-trinsic-wider header never truncates; Swift's `Column(alignment: .leading)`
//!   caps children at the authored frame width. Stretch reproduces the observed
//!   geometry (title capped at 220pt, one line).
//! - **No `ButtonHost.markerKey` attachment.** The Rust button host publishes no
//!   marker attachment; the headers are located by their published accessibility
//!   role (`Button` with `is_expanded`), the same contract the sample suites use.

use crate::support;

use std::collections::BTreeSet;

use cove_composition::{AnyView, TupleView};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Wrap};
use cove_core::{Environment, InputCapabilities, State};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers};
use cove_ui_compose::BoxView;
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::layout::{Accordion, AccordionItem, DisclosureGroup};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::ThemeViewExt;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;
use cove_ui_compose::theme::{TextScaler, Theme};

use support::InteractiveScene;

const TITLE: &str = "Workspace permissions, collaboration and notification preferences";

fn text_of(node: &cove_core::scene::Node) -> Option<String> {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text)
}

#[test]
fn long_headers_keep_one_line_and_keyboard_activation_at_both_text_scales() {
    InteractiveScene::run(720.0, 600.0, |scene| {
        for width in [390.0_f32, 720.0, 1280.0] {
            for scale in [1.0_f32, 2.0] {
                for (theme_label, theme) in [
                    ("light", Theme::default_light()),
                    ("dark", Theme::default_dark()),
                ] {
                    let context_label = format!("width {width} scale {scale} theme {theme_label}");
                    scene.width = width;
                    scene.height = 600.0;

                    let expanded = State::new(BTreeSet::<i32>::new());
                    let disclosure = State::new(false);

                    let mut surface = Environment::default();
                    surface.viewport.width = width;
                    surface.viewport.height = 600.0;
                    surface.input = if width == 1280.0 {
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD
                    } else {
                        InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
                    };

                    let column = AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        20.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(
                                Accordion::try_new(
                                    vec![AccordionItem::new(
                                        1i32,
                                        TITLE,
                                        AnyView::new(Text::new("Accordion content")),
                                    )],
                                    expanded.binding(),
                                )
                                .expect("unique accordion IDs"),
                            ),
                            AnyView::new(DisclosureGroup::title(
                                disclosure.binding(),
                                TITLE,
                                AnyView::new(Text::new("Disclosure content")),
                            )),
                        ])),
                    ))
                    .frame_width(220.0)
                    .surface_environment(surface)
                    .text_scaler(TextScaler::new(scale))
                    .theme(theme)
                    .reduced_motion(true);

                    scene.install(column);

                    let titles = scene.all(|node| text_of(node).as_deref() == Some(TITLE));
                    assert_eq!(
                        titles.len(),
                        2,
                        "both headers mount their title ({context_label})"
                    );
                    for label in &titles {
                        assert!(
                            label.frame().width > 0.0 && label.frame().width <= 220.0,
                            "the title stays inside the 220pt column, got {} ({context_label})",
                            label.frame().width
                        );
                        let lines = label
                            .layout_node()
                            .and_then(|layout| layout.text_measure())
                            .map(|measured| measured.result.lines.len())
                            .expect("the title records a text measure");
                        assert_eq!(lines, 1, "the title keeps one line ({context_label})");
                        assert_eq!(
                            label.accessibility().map(|semantics| semantics.label),
                            Some(TITLE.to_string()),
                            "the title keeps its accessibility label ({context_label})"
                        );
                    }

                    let headers = scene.all(|node| {
                        node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::Button
                                && semantics.state.is_expanded.is_some()
                        })
                    });
                    assert_eq!(
                        headers.len(),
                        2,
                        "the accordion and the disclosure each publish one expandable header ({context_label})"
                    );

                    for header in &headers {
                        if width != 1280.0 {
                            assert!(
                                header.frame().height >= 44.0,
                                "headers keep the 44pt touch height, got {} ({context_label})",
                                header.frame().height
                            );
                        }
                        let expanded_before = expanded.get();
                        let disclosure_before = disclosure.get();
                        scene.focus.focus(Some(header), false);
                        scene.key(Scancode::SPACE, KeyModifiers::default());
                        assert_ne!(
                            (expanded.get(), disclosure.get()),
                            (expanded_before, disclosure_before),
                            "Space key-down activates the focused header ({context_label})"
                        );
                        let after_key_down = (expanded.get(), disclosure.get());
                        scene.dispatcher().dispatch(&InputEvent::KeyUp(KeyEvent {
                            scancode: Scancode::SPACE,
                            keycode: 0,
                            modifiers: KeyModifiers::default(),
                            is_repeat: false,
                        }));
                        scene.settle(5, 1.0);
                        assert_eq!(
                            (expanded.get(), disclosure.get()),
                            after_key_down,
                            "Space key-up does not activate again ({context_label})"
                        );
                    }

                    assert_eq!(
                        (expanded.get(), disclosure.get()),
                        (BTreeSet::from([1i32]), true),
                        "space activates both headers ({context_label})"
                    );
                }
            }
        }
    });
}
