//! Translation of `Tests/CoveUIComposeTests/BoolControlLabelTests.swift`.
//!
//! Behaviour checklist:
//! - Toggle and Checkbox expose their label slot and keep exactly one
//!   accessible control each
//! - the switch track is 36pt wide and the check box 16pt, matching the
//!   authored intrinsics
//! - the label text materialises as a measured text input
//! - the accessibility activate action drives the caller's binding

use crate::support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cove_composition::AnyView;
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::{Checkbox, CheckboxState, Toggle};

use support::GlobalTestLock;

fn all_nodes(root: &Node, into: &mut Vec<Node>) {
    into.push(root.clone());
    for child in root.children() {
        all_nodes(&child, into);
    }
}

#[test]
fn toggle_and_checkbox_expose_label_slot_with_one_control_each() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let toggle_on = Arc::new(AtomicBool::new(false));
            let checked = Arc::new(AtomicBool::new(false));

            let toggle_binding = {
                let get = toggle_on.clone();
                let set = toggle_on.clone();
                Binding::new(
                    move || get.load(Ordering::SeqCst),
                    move |value| set.store(value, Ordering::SeqCst),
                )
            };
            let _check_binding = {
                let get = checked.clone();
                let set = checked.clone();
                Binding::new(
                    move || get.load(Ordering::SeqCst),
                    move |value| set.store(value, Ordering::SeqCst),
                )
            };

            let toggle = Toggle::labeled("Notifications", toggle_binding);
            let checkbox = Checkbox::labeled("Accept the terms", {
                let get = checked.clone();
                let set = checked.clone();
                Binding::new(
                    move || {
                        if get.load(Ordering::SeqCst) {
                            CheckboxState::On
                        } else {
                            CheckboxState::Off
                        }
                    },
                    move |state: CheckboxState| {
                        set.store(state == CheckboxState::On, Ordering::SeqCst)
                    },
                )
            });

            let column = AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(toggle),
                        AnyView::new(checkbox),
                    ])),
                ))
                .frame_width(260.0),
            );

            let graph = support::graph();
            graph.install_view(column, Default::default());
            graph.compute_layout(260.0, 120.0);

            let root = graph.tree.borrow().root().expect("root node");
            let mut nodes = Vec::new();
            all_nodes(&root, &mut nodes);

            let toggle_node = nodes
                .iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Toggle
                            && semantics.label == "Notifications"
                    })
                })
                .cloned()
                .expect("a Toggle exposing its label");
            let checkbox_node = nodes
                .iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Checkbox
                            && semantics.label == "Accept the terms"
                    })
                })
                .cloned()
                .expect("a Checkbox exposing its label");

            // The authored intrinsics: switch track 36pt, check box 16pt.
            assert_eq!(
                toggle_node.frame().width,
                36.0,
                "the switch track is 36pt wide"
            );
            assert_eq!(
                checkbox_node.frame().width,
                16.0,
                "the check box is 16pt wide"
            );

            // The label slot measures real text rather than dropping it.
            let has_text = |text: &str| {
                nodes.iter().any(|node| {
                    node.layout_node()
                        .and_then(|layout| layout.text_inputs())
                        .is_some_and(|inputs| inputs.text == text)
                })
            };
            assert!(has_text("Notifications"), "the Toggle label materialises");
            assert!(
                has_text("Accept the terms"),
                "the Checkbox label materialises"
            );

            // One accessible control per widget, not a wrapper plus an inner node.
            let toggle_count = nodes
                .iter()
                .filter(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Toggle
                            && semantics.label == "Notifications"
                    })
                })
                .count();
            let checkbox_count = nodes
                .iter()
                .filter(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Checkbox
                            && semantics.label == "Accept the terms"
                    })
                })
                .count();
            assert_eq!(toggle_count, 1, "exactly one Toggle semantics node");
            assert_eq!(checkbox_count, 1, "exactly one Checkbox semantics node");

            // Activating through the accessibility action drives the binding.
            if let Some(activate) = toggle_node.accessibility_actions().activate {
                activate();
            }
            if let Some(activate) = checkbox_node.accessibility_actions().activate {
                activate();
            }
            assert!(
                toggle_on.load(Ordering::SeqCst),
                "the Toggle action switched it on"
            );
            assert!(
                checked.load(Ordering::SeqCst),
                "the Checkbox action switched it on"
            );
        });
    });
}
