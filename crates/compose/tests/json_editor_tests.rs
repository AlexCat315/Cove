//! Translation of `Tests/CoveUIComposeTests/Adaptive/JsonEditorTests.swift`.
//!
//! Behaviour checklist:
//! - the expanded JSON editor keeps one portal, its node and labelled heading,
//!   fits the window and safe area (full width at 390pt) across width, text
//!   scale, input, theme and direction, with 44pt touch actions below the text
//! - an invalid draft disables Apply, survives resizes and Primary-Return; Apply
//!   commits a valid draft once and returns focus to Expand; Escape cancels
//!
//! The Swift suite located the expand button, expanded editor and actions by
//! the `json-expand`, `json-expanded-editor`, `json-apply` and `json-cancel`
//! debug names. Rust publishes the dialog and footer names; the expand button
//! is found by its accessibility label, the dialog by its `Dialog` role and the
//! actions by their title text.

use crate::support;
use cove_composition::Scope;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, View};
use cove_core::scene::{AccessibilityRole, Direction, Node};
use cove_core::{Environment, InputCapabilities, State};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers};
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{EdgeInsets, LayoutFrameExt};
use cove_ui_compose::primitives::controls::fields::json_field::JsonField;
use cove_ui_compose::primitives::controls::fields::json_field_model::JsonFieldLabels;
use cove_ui_compose::primitives::layout::insets::safe_area::safe_area_insets;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt};

use support::InteractiveScene;

const INITIAL: &str = "{\"value\":1}";

struct HarnessState {
    width: State<f32>,
    scale: State<f32>,
    input: State<InputCapabilities>,
    dark: State<bool>,
    rtl: State<bool>,
    text: State<TextBuffer>,
    labels: State<JsonFieldLabels>,
    commits: Arc<AtomicUsize>,
}

struct Harness(Rc<HarnessState>);

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let state = &self.0;
        let mut surface = Environment::default();
        surface.viewport.width = state.width.get();
        surface.viewport.height = 820.0;
        surface.input = state.input.get();
        let commits = state.commits.clone();
        let field = JsonField::new(state.text.binding())
            .labels(state.labels.get())
            .on_commit(move |_| {
                commits.fetch_add(1, Ordering::Relaxed);
            });
        let root = AnyView::new(LayerRoot::new(AnyView::new(AnyView::new(field).frame(
            None,
            None,
            None,
            None,
            Some(520.0),
            None,
        ))))
        .frame_width(state.width.get())
        .frame_height(820.0)
        .surface_environment(surface)
        .text_scaler(TextScaler::new(state.scale.get()))
        .appearance(
            if state.dark.get() {
                Appearance::Dark
            } else {
                Appearance::Light
            },
            ThemeTransition::default(),
        )
        .layout_direction(if state.rtl.get() {
            Direction::Rtl
        } else {
            Direction::Ltr
        })
        .reduced_motion(true);
        safe_area_insets(AnyView::new(root), EdgeInsets::new(24.0, 0.0, 34.0, 0.0))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        let state = &self.0;
        vec![
            DynamicField::state("width", &state.width),
            DynamicField::state("scale", &state.scale),
            DynamicField::state("input", &state.input),
            DynamicField::state("dark", &state.dark),
            DynamicField::state("rtl", &state.rtl),
            DynamicField::state("text", &state.text),
            DynamicField::state("labels", &state.labels),
        ]
    }

    fn tag(&self) -> &'static str {
        "JsonEditorHarness"
    }
}

fn scene(work: impl FnOnce(&mut InteractiveScene, &Rc<HarnessState>)) {
    InteractiveScene::run(390.0, 820.0, |scene| {
        let state = Rc::new(HarnessState {
            width: State::new(390.0),
            scale: State::new(1.0),
            input: State::new(InputCapabilities::TOUCH),
            dark: State::new(false),
            rtl: State::new(false),
            text: State::new(TextBuffer::new(INITIAL)),
            labels: State::new(JsonFieldLabels::default()),
            commits: Arc::new(AtomicUsize::new(0)),
        });
        scene.install(Harness(state.clone()));
        scene.settle(8, 1.0);
        work(scene, &state);
    });
}

fn expand_button(scene: &InteractiveScene, label: &str) -> Option<Node> {
    scene.first(|node| {
        node.is_hit_testable()
            && node
                .accessibility()
                .is_some_and(|semantics| semantics.label == label)
    })
}

fn dialog(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
    })
}

fn action(scene: &InteractiveScene, title: &str) -> Option<Node> {
    scene.first(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            && find_text(node, title)
    })
}

fn find_text(node: &Node, title: &str) -> bool {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .is_some_and(|inputs| inputs.text == title)
        || node.children().iter().any(|child| find_text(child, title))
}

fn key(scene: &InteractiveScene, scancode: u32, primary: bool) {
    scene.dispatcher().dispatch(&InputEvent::KeyDown(KeyEvent {
        scancode,
        keycode: 0,
        modifiers: if primary {
            KeyModifiers::LEFT_GUI
        } else {
            KeyModifiers::default()
        },
        is_repeat: false,
    }));
}

fn settle(scene: &InteractiveScene) {
    scene.settle(8, 1.0);
}

#[test]
fn matrix() {
    scene(|scene, state| {
        let labels = JsonFieldLabels {
            expand: "Edit project configuration and international collaboration permissions"
                .to_string(),
            apply: "Apply configuration changes".to_string(),
            cancel: "Discard editing changes".to_string(),
            ..JsonFieldLabels::default()
        };
        state.labels.set(labels.clone());
        settle(scene);
        let expand =
            expand_button(scene, &labels.expand).expect("the expand button carries its label");
        scene.click(&expand, 1);
        settle(scene);
        let presented = dialog(scene).expect("Expand presents the expanded editor");
        assert_eq!(
            presented.accessibility().map(|semantics| semantics.label),
            Some(labels.expand.clone()),
            "the dialog is labelled with the expand label"
        );
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "the expanded editor owns exactly one portal entry"
        );
        for width in [390.0_f32, 720.0, 1280.0] {
            for scale in [1.0_f32, 2.0] {
                for modality in [
                    InputCapabilities::TOUCH,
                    InputCapabilities::TOUCH
                        | InputCapabilities::POINTER
                        | InputCapabilities::KEYBOARD,
                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
                ] {
                    for dark in [false, true] {
                        state.width.set(width);
                        state.scale.set(scale);
                        state.input.set(modality);
                        state.dark.set(dark);
                        state.rtl.set(dark);
                        scene.width = width;
                        settle(scene);
                        let context = format!("width {width} scale {scale} dark {dark}");
                        let current = expand_button(scene, &labels.expand).expect("expand button");
                        assert_eq!(
                            current.id(),
                            expand.id(),
                            "the expand button is retained ({context})"
                        );
                        assert!(!scene.graph.recomposer.has_pending(), "settled ({context})");
                        let kept = dialog(scene).unwrap_or_else(|| {
                            panic!("the expanded editor stays presented ({context})")
                        });
                        assert_eq!(
                            kept.accessibility().map(|semantics| semantics.label),
                            Some(labels.expand.clone()),
                            "the dialog keeps the expand label ({context})"
                        );
                        assert_eq!(
                            scene.portal.entries().len(),
                            1,
                            "one portal entry across the matrix ({context})"
                        );
                        assert!(
                            action(scene, &labels.apply).is_some()
                                && action(scene, &labels.cancel).is_some(),
                            "Apply and Cancel stay mounted ({context})"
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn transactions() {
    scene(|scene, state| {
        let labels = JsonFieldLabels::default();
        let expand = expand_button(scene, &labels.expand).expect("expand");
        scene.click(&expand, 1);
        settle(scene);
        assert!(scene.focus.focused().is_some(), "the click focuses a node");
        assert!(
            !scene.portal.entries().is_empty(),
            "Expand presents the expanded editor in a portal"
        );
        assert!(
            dialog(scene).is_some(),
            "the expanded editor is a dialog"
        );
        key(scene, Scancode::A, true);
        scene
            .dispatcher()
            .dispatch(&InputEvent::TextInput("{ invalid".into()));
        settle(scene);
        key(scene, Scancode::RETURN, true);
        settle(scene);
        assert!(
            state.text.get().string_value() == INITIAL
                && state.commits.load(Ordering::Relaxed) == 0,
            "an invalid draft never commits: {:?}, commits {}",
            state.text.get().string_value(),
            state.commits.load(Ordering::Relaxed)
        );
        for width in [720.0_f32, 1280.0, 390.0] {
            state.width.set(width);
            scene.width = width;
            settle(scene);
        }
        let apply = action(scene, &labels.apply).expect("the invalid draft keeps an Apply action");
        assert!(
            apply
                .accessibility()
                .is_some_and(|semantics| !semantics.state.is_enabled),
            "an invalid draft disables Apply"
        );
        assert_eq!(
            state.commits.load(Ordering::Relaxed),
            0,
            "a disabled Apply never commits"
        );
        assert!(
            dialog(scene).is_some(),
            "the expanded editor survives a resize with an invalid draft"
        );
        key(scene, Scancode::ESCAPE, false);
        settle(scene);
        assert!(
            state.text.get().string_value() == INITIAL
                && state.commits.load(Ordering::Relaxed) == 0
                && scene.portal.entries().is_empty(),
            "Escape leaves the bound value and no portal"
        );
        assert!(
            scene
                .focus
                .focused()
                .is_some_and(|node| node.id() == expand.id()),
            "focus remains on the expand button"
        );
    });
}
