//! Translation of `Tests/CoveUIComposeTests/Adaptive/DialogTests.swift`.
//!
//! Behaviour checklist:
//! - resizing a presented dialog preserves the focused text field, the edited
//!   draft and the identity of the footer action nodes
//! - the alert panel fits compact windows (374pt) and keeps a 420pt width
//!   otherwise, with finite frames for every node
//! - Cancel/Confirm render as complete measured labels; they stack vertically
//!   at compact width with 44pt touch lanes and share a row otherwise
//! - the message sits above the actions inside the panel
//! - activating Confirm runs the confirmation exactly once and dismisses;
//!   a second activation on the closed dialog is ignored
//!
//! Named differences from the Swift suite:
//! - **No `ButtonHost.markerKey` attachment.** Rust button hosts publish no
//!   marker attachment; action nodes are located by their accessibility role
//!   (`Button`), and the close button of the `Dialog` is included in the
//!   retained-id set like the Swift suite included it via the marker.
//! - **Dialog actions carry text labels, not accessibility labels.** Rust builds
//!   the alert actions with `Button::title`, which measures the title text into
//!   a child node; the suite finds the buttons by a descendant text node and
//!   asserts the measurement, matching the Swift `layoutNode.textInputs` checks.

use crate::support;
use cove_composition::Scope;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cove_composition::{AnyView, DynamicField, TupleView, View};
use cove_core::scene::AccessibilityRole;
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::overlays::modal::dialog::{AlertDialog, Dialog};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::theme::TextScaler;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;
use cove_ui_compose::theme::tokens::text_scaler::TextScalerViewExt;

use support::InteractiveScene;

/// The Swift `Harness`: a presented dialog with a bound draft and a width that
/// the test resizes. `State` fields replay the Swift `@State` semantics.
struct DialogScene {
    width: State<f32>,
    text: State<TextBuffer>,
}

struct DialogSceneView {
    scene: Rc<DialogScene>,
}

impl View for DialogSceneView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut surface = Environment::default();
        surface.viewport.width = self.scene.width.get();
        surface.viewport.height = 820.0;
        surface.input =
            InputCapabilities::TOUCH | InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(
                Dialog::new(
                    Binding::constant(true),
                    "Retained dialog",
                    TextField::new("Draft", self.scene.text.binding()),
                )
                .footer(AnyView::new(TupleView::new(vec![
                    AnyView::new(Button::title("Cancel", || {})),
                    AnyView::new(Button::title("Save", || {})),
                ]))),
            )))
            .frame_width(self.scene.width.get())
            .frame_height(820.0)
            .surface_environment(surface)
            .reduced_motion(true),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.scene.width),
            DynamicField::state("text", &self.scene.text),
        ]
    }
    fn tag(&self) -> &'static str {
        "DialogSceneView"
    }
}

#[test]
fn resizing_preserves_the_action_nodes_editing_draft_and_focus() {
    InteractiveScene::run(720.0, 820.0, |scene| {
        let dialog_scene = Rc::new(DialogScene {
            width: State::new(720.0),
            text: State::new(TextBuffer::from("Draft")),
        });
        scene.install(DialogSceneView {
            scene: dialog_scene.clone(),
        });
        scene.settle(7, 1.0);

        let field = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
                    && node.is_focusable()
            })
            .expect("the dialog mounts a focusable text field");
        scene.focus.focus(Some(&field), false);
        dialog_scene.text.set(TextBuffer::from("Edited draft"));
        scene.settle(3, 1.0);

        let action_ids: Vec<cove_core::scene::ElementId> = scene
            .all(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
            })
            .into_iter()
            .map(|node| node.id())
            .collect();
        assert_eq!(
            action_ids.len(),
            3,
            "Cancel, Save and the close button mount"
        );

        for width in [390.0_f32, 1280.0, 720.0] {
            dialog_scene.width.set(width);
            scene.width = width;
            scene.settle(12, 1.0);

            assert_eq!(
                scene.focus.focused().map(|node| node.id()),
                Some(field.id()),
                "the resize keeps the focused field at width {width}"
            );
            assert_eq!(
                dialog_scene.text.get().string_value(),
                "Edited draft",
                "the resize preserves the edited draft at width {width}"
            );
            let after: Vec<cove_core::scene::ElementId> = scene
                .all(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                })
                .into_iter()
                .map(|node| node.id())
                .collect();
            assert_eq!(
                after, action_ids,
                "the action nodes are retained at width {width}"
            );
        }
    });
}

fn button_with_text(
    nodes: &[cove_core::scene::Node],
    title: &str,
) -> Option<cove_core::scene::Node> {
    nodes
        .iter()
        .find(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Button)
                && scene_of(node).into_iter().any(|descendant| {
                    descendant
                        .layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == title)
                })
        })
        .cloned()
}

fn collect(node: &cove_core::scene::Node, into: &mut Vec<cove_core::scene::Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn scene_of(node: &cove_core::scene::Node) -> Vec<cove_core::scene::Node> {
    let mut nodes = Vec::new();
    collect(node, &mut nodes);
    nodes
}

#[test]
fn dialogs_fit_compact_windows_and_stack_the_same_accessible_actions() {
    for width in [390.0_f32, 720.0, 1280.0] {
        for scale in [1.0_f32, 2.0] {
            for (appearance_label, appearance) in
                [("light", Appearance::Light), ("dark", Appearance::Dark)]
            {
                let context_label = format!("width {width} scale {scale} {appearance_label}");
                let confirmed = Arc::new(AtomicU32::new(0));
                let confirmed_for_dialog = confirmed.clone();
                let open = Arc::new(AtomicBool::new(true));
                let open_for_get = open.clone();
                let open_for_set = open.clone();
                InteractiveScene::run(width, 820.0, |scene| {
                    let mut surface = Environment::default();
                    surface.viewport.width = width;
                    surface.viewport.height = 820.0;
                    surface.input = if width == 390.0 {
                        InputCapabilities::TOUCH
                    } else if width == 720.0 {
                        InputCapabilities::TOUCH
                            | InputCapabilities::POINTER
                            | InputCapabilities::KEYBOARD
                    } else {
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD
                    };
                    let is_presented = Binding::new(
                        move || open_for_get.load(Ordering::Relaxed),
                        move |value: bool| open_for_set.store(value, Ordering::Relaxed),
                    );
                    let root = AnyView::new(
                        AnyView::new(LayerRoot::new(AnyView::new(AlertDialog::new(
                            is_presented,
                            "Delete project?",
                            "This removes the selected project from the workspace.",
                            move || {
                                confirmed_for_dialog.fetch_add(1, Ordering::Relaxed);
                            },
                        ))))
                        .frame_width(width)
                        .frame_height(820.0)
                        .surface_environment(surface)
                        .text_scaler(TextScaler::new(scale))
                        .appearance(appearance, ThemeTransition::default())
                        .reduced_motion(true),
                    );
                    scene.install(root);
                    scene.settle(5, 0.1);

                    let nodes = scene.nodes();
                    let panel = nodes
                        .iter()
                        .find(|node| {
                            node.accessibility().is_some_and(|semantics| {
                                semantics.role == AccessibilityRole::Dialog
                                    && semantics.label == "Delete project?"
                            })
                        })
                        .cloned()
                        .expect("the alert presents a labelled dialog panel");
                    let expected_width = (width - 16.0).min(420.0);
                    assert!(
                        (panel.frame().width - expected_width).abs() < 1.0,
                        "the panel fits the window ({} vs {expected_width}, {context_label})",
                        panel.frame().width
                    );

                    let cancel = button_with_text(&nodes, "Cancel")
                        .expect("the alert mounts a Cancel button");
                    let confirm = button_with_text(&nodes, "Confirm")
                        .expect("the alert mounts a Confirm button");
                    for (button, title) in [(&cancel, "Cancel"), (&confirm, "Confirm")] {
                        let label = scene_of(button)
                            .into_iter()
                            .find(|node| {
                                node.layout_node()
                                    .and_then(|layout| layout.text_inputs())
                                    .is_some_and(|inputs| inputs.text == title)
                            })
                            .unwrap_or_else(|| panic!("the {title} action measures its title"));
                        assert_eq!(
                            label
                                .layout_node()
                                .and_then(|layout| layout.text_inputs())
                                .map(|inputs| inputs.text),
                            Some(title.to_string()),
                            "the {title} label is measured ({context_label})"
                        );
                    }

                    if width == 390.0 {
                        assert!(
                            confirm.absolute_frame().y >= cancel.absolute_frame().max_y(),
                            "the actions stack vertically at compact width ({context_label})"
                        );
                        assert!(
                            cancel.frame().height >= 44.0 && confirm.frame().height >= 44.0,
                            "both actions keep the 44pt touch lane ({context_label}): {} / {}",
                            cancel.frame().height,
                            confirm.frame().height
                        );
                    } else {
                        assert!(
                            (cancel.absolute_frame().y - confirm.absolute_frame().y).abs() < 1.0,
                            "the actions share one row ({context_label})"
                        );
                    }
                    assert!(
                        confirm.absolute_frame().max_y() <= panel.absolute_frame().max_y(),
                        "the confirm action stays inside the panel ({context_label})"
                    );

                    let message = nodes
                        .iter()
                        .find(|node| {
                            node.layout_node()
                                .and_then(|layout| layout.text_inputs())
                                .is_some_and(|inputs| inputs.text.starts_with("This removes"))
                        })
                        .cloned()
                        .expect("the alert mounts the message text");
                    assert!(
                        message.frame().height > 0.0,
                        "the message occupies vertical space ({context_label})"
                    );
                    assert!(
                        message.absolute_frame().max_y() <= cancel.absolute_frame().y,
                        "the actions follow the message in window coordinates ({context_label})"
                    );
                    for node in &nodes {
                        assert!(
                            node.frame().width.is_finite() && node.frame().height.is_finite(),
                            "every frame is finite ({context_label})"
                        );
                    }

                    let activate = confirm
                        .accessibility_actions()
                        .activate
                        .expect("the confirm action exposes activate");
                    activate();
                    activate();
                    assert_eq!(
                        (
                            confirmed.load(Ordering::Relaxed),
                            open.load(Ordering::Relaxed)
                        ),
                        (1, false),
                        "the confirmation runs once and dismisses ({context_label})"
                    );
                });
            }
        }
    }
}
