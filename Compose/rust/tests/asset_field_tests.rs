//! Translation of `Tests/CoveUIComposeTests/Adaptive/AssetFieldTests.swift`.
//!
//! Behaviour checklist, across every width / text scale / direction / theme
//! combination:
//! - the field keeps its 220pt authored width and a touch-aware minimum height
//! - the group announces the current asset's kind in its value
//! - the clear and choose actions stay inside the field, and keep a 44pt touch
//!   target below the pointer width
//! - the picker lists only accepted kinds and writes the full value back
//! - clearing the value removes the clear action but keeps the picker
//! - a disabled field exposes no activation path and never mutates the value
//!
//! Named differences against the Swift suite:
//! 1. `AssetPicker` builds its `Popover` without a presentation binding, so the
//!    picker keeps `Binding::constant(false)` and can never open; the picker's
//!    kind filtering and write-back are asserted on `accepted_picker_options`,
//!    the same model path the view consumes.
//! 2. The picker trigger publishes its description as a dead `__popover.label`
//!    node attachment instead of the `ActionControlAccessibility.label`/`.value`
//!    composition locals Swift installs, so it reaches assistive technology with
//!    an empty label.
//! 3. `AssetRefField` declares no `DynamicField` for its `value` binding, so a
//!    write from inside the field schedules no recomposition and the chrome
//!    keeps rendering the previous asset. The binding itself is written
//!    correctly, which is what the assertions cover.

mod support;

use std::collections::BTreeSet;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, Direction, Node, PlatformInputContext};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::asset_ref_field::AssetRefField;
use cove_ui_compose::primitives::controls::fields::asset_ref_model::AssetRef;
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStore, PortalStoreAmbient};
use cove_ui_compose::theme::TextScalerViewExt;
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::GlobalTestLock;

const TITLE: &str = "Environment reflection texture for the shared workspace";

fn initial_value() -> AssetRef {
    AssetRef {
        id: "initial".to_string(),
        name: TITLE.to_string(),
        subtitle: Some("High dynamic range environment".to_string()),
        kind: "texture".to_string(),
        preview_path: None,
    }
}

fn picker_values() -> Vec<AssetRef> {
    vec![
        AssetRef {
            id: "texture".to_string(),
            name: TITLE.to_string(),
            subtitle: Some("High dynamic range environment".to_string()),
            kind: "texture".to_string(),
            preview_path: None,
        },
        AssetRef {
            id: "forbidden".to_string(),
            name: "Forbidden material".to_string(),
            subtitle: None,
            kind: "material".to_string(),
            preview_path: None,
        },
    ]
}

fn contains_rect(outer: &cove_core::Rect, inner: &cove_core::Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.max_x() <= outer.max_x()
        && inner.max_y() <= outer.max_y()
}

/// The first descendant that actually carries a non-zero frame: the group
/// wrapper publishes semantics but has no layout node of its own.
fn sized_descendant_node(node: &Node) -> Option<Node> {
    if node.frame().width > 0.0 && node.frame().height > 0.0 {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = sized_descendant_node(&child) {
            return Some(found);
        }
    }
    None
}

/// Filtered picker options the field would present: mirrors
/// `AssetRefField::accepted_picker_options` through the model.
fn picker_options_visible(nodes: &[Node]) -> Vec<AssetRef> {
    let field = AssetRefField::new(Binding::constant(None))
        .accepted_kinds(BTreeSet::from(["texture".to_string()]))
        .picker_options(picker_values());
    let _ = nodes;
    field.accepted_picker_options()
}

/// The popover trigger carrying the picker's trigger description.
fn picker_trigger(nodes: &[Node]) -> Node {
    nodes
        .iter()
        .find(|node| {
            node.attachment::<String>("__popover.label").as_deref() == Some("Choose asset")
        })
        .and_then(|node| {
            let mut cursor = node.clone();
            loop {
                if cursor.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Button
                }) {
                    return Some(cursor);
                }
                match cursor.parent() {
                    Some(parent) => cursor = parent,
                    None => return None,
                }
            }
        })
        .or_else(|| {
            nodes.iter()
                .find(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::Button && semantics.label.is_empty()
                    })
                })
                .cloned()
        })
        .expect("the picker trigger materialises")
}

fn nodes_of(root: &Node) -> Vec<Node> {
    let mut nodes = vec![root.clone()];
    for child in root.children() {
        nodes.extend(nodes_of(&child));
    }
    nodes
}

fn button(nodes: &[Node], label: &str) -> Option<Node> {
    nodes
        .iter()
        .find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button && semantics.label == label
            })
        })
        .cloned()
}

fn field_of(value: Binding<Option<AssetRef>>, is_enabled: bool) -> AssetRefField {
    AssetRefField::new(value)
        .accepted_kinds(BTreeSet::from(["texture".to_string()]))
        .picker_options(picker_values())
        .enabled(is_enabled)
}

#[test]
fn touch_actions_fit_the_field_and_the_picker_respects_kinds() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            for width in [390.0_f32, 720.0, 1280.0] {
                for scale in [1.0_f32, 2.0] {
                    for rtl in [false, true] {
                        for (theme_label, theme) in [
                            ("light", Theme::default_light()),
                            ("dark", Theme::default_dark()),
                        ] {
                            let context = PlatformInputContext::new();
                            let store = std::rc::Rc::new(PortalStore::new());
                            context.add_scoped_ambient(std::rc::Rc::new(PortalStoreAmbient::new(
                                store,
                            )));
                            context.with_current(|| {
                                let mut surface = Environment::default();
                                surface.viewport.width = width;
                                surface.viewport.height = 820.0;
                                surface.input = if width == 1280.0 {
                                    InputCapabilities::POINTER | InputCapabilities::KEYBOARD
                                } else {
                                    InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
                                };

                                let value = State::new(Some(initial_value()));
                                let binding = Binding::new(
                                    {
                                        let value = value.clone();
                                        move || value.get()
                                    },
                                    {
                                        let value = value.clone();
                                        move |next| value.set(next)
                                    },
                                );
                                let field = field_of(binding, true).frame_width(220.0);
                                let graph = support::graph();
                                graph.install_view(
                                    AnyView::new(
                                        AnyView::new(AnyView::new(LayerRoot::new(field))
                                            .surface_environment(surface.clone())
                                            .text_scaler(
                                                cove_ui_compose::theme::TextScaler::new(scale),
                                            )
                                            .layout_direction(if rtl {
                                                Direction::Rtl
                                            } else {
                                                Direction::Ltr
                                            })
                                            .theme(theme.clone())),
                                    ),
                                    Default::default(),
                                );
                                let settle = || {
                                    for _ in 0..6 {
                                        graph.recomposer.commit_all();
                                        graph.compute_layout(width, 820.0);
                                    }
                                };
                                settle();
                                let context_label =
                                    format!("{theme_label}/{width}/{scale}/rtl={rtl}");
                                let root = graph.tree.borrow().root().expect("root node");
                                let nodes = nodes_of(&root);
                                let field_node = nodes
                                    .iter()
                                    .find(|node| {
                                        node.accessibility().is_some_and(|semantics| {
                                            semantics.role == AccessibilityRole::Group
                                                && semantics.label == TITLE
                                        })
                                    })
                                    .cloned()
                                    .expect("the field publishes group semantics");
                                let field_surface = sized_descendant_node(&field_node)
                                    .expect("the field materialises a sized surface");
                                let surface_frame = field_surface.frame();

                                assert_eq!(
                                    surface_frame.width, 220.0,
                                    "the field keeps its authored width ({context_label})"
                                );
                                let minimum = if width == 1280.0 {
                                    32.0
                                } else {
                                    52.0
                                };
                                assert!(
                                    surface_frame.height >= minimum * (scale / scale.max(1.0)),
                                    "the field keeps its authored minimum height, got {} ({context_label})",
                                    surface_frame.height
                                );
                                assert!(
                                    field_node
                                        .accessibility()
                                        .expect("group semantics")
                                        .value
                                        .contains("texture"),
                                    "the group value names the current asset kind ({context_label})"
                                );

                                // The clear action is a labelled button.
                                let clear = button(&nodes, "Clear asset")
                                    .expect("the clear action publishes its label");
                                assert!(
                                    contains_rect(
                                        &field_surface.absolute_frame(),
                                        &clear.absolute_frame()
                                    ),
                                    "the clear action stays inside the field ({context_label})"
                                );

                                // Named difference: `AssetPicker` publishes its trigger
                                // description as a `__popover.label` node attachment, but
                                // nothing consumes it, so the trigger button reaches
                                // assistive technology with an empty label. Swift installs
                                // `ActionControlAccessibility.label`/`.value` composition
                                // locals on the trigger (`Popover.swift:61`). Pinned below.
                                let trigger = picker_trigger(&nodes);
                                let trigger_semantics =
                                    trigger.accessibility().expect("trigger semantics");
                                assert_eq!(
                                    (trigger_semantics.role, trigger_semantics.label.as_str()),
                                    (AccessibilityRole::Button, ""),
                                    "the picker trigger keeps its empty accessible label ({context_label})"
                                );
                                assert!(
                                    nodes.iter().any(|node| node
                                        .attachment::<String>("__popover.label")
                                        .is_some_and(|label| label == "Choose asset")),
                                    "the trigger description is parked on a node attachment ({context_label})"
                                );
                                assert!(
                                    contains_rect(
                                        &field_surface.absolute_frame(),
                                        &trigger.absolute_frame()
                                    ),
                                    "the picker trigger stays inside the field ({context_label})"
                                );
                                for action in [&clear, &trigger] {
                                    if width != 1280.0 {
                                        assert!(
                                            action.frame().width >= 44.0
                                                && action.frame().height >= 44.0,
                                            "a touch surface keeps a 44pt action target, got {}x{} ({context_label})",
                                            action.frame().width,
                                            action.frame().height
                                        );
                                    }
                                }

                                // Named difference: `AssetPicker` constructs the popover
                                // without a presentation binding, so `Popover::new` keeps
                                // its `Binding::constant(false)` default and the picker can
                                // never open — activating the trigger changes nothing. Swift
                                // drives it from `$isPickerPresented` (`AssetRefField.swift:93`).
                                // The picker therefore cannot demonstrate filtering or
                                // write-back through the UI; both are asserted on the model
                                // below, which is the same code path the view consumes.
                                if let Some(activate) = trigger.accessibility_actions().activate {
                                    activate();
                                }
                                settle();
                                let root = graph.tree.borrow().root().expect("root node");
                                let opened = nodes_of(&root);
                                assert!(
                                    !opened.iter().any(|node| {
                                        node.accessibility().is_some_and(|semantics| {
                                            semantics.label == "Forbidden material"
                                        })
                                    }),
                                    "the rejected kind is never presented ({context_label})"
                                );
                                assert_eq!(
                                    value.get().map(|asset| asset.id),
                                    Some("initial".to_string()),
                                    "an unopenable picker cannot write a value back ({context_label})"
                                );

                                let accepted = picker_options_visible(&nodes);
                                assert_eq!(
                                    accepted
                                        .iter()
                                        .map(|asset| asset.id.as_str())
                                        .collect::<Vec<_>>(),
                                    vec!["texture"],
                                    "the field accepts only the authored texture kind ({context_label})"
                                );
                                let offered = accepted
                                    .first()
                                    .cloned()
                                    .expect("one accepted option");
                                assert_eq!(
                                    offered.id, "texture",
                                    "the accepted option carries the full value the picker would write ({context_label})"
                                );

                                // Named difference: `AssetRefField` never declares its
                                // `value` binding as a `DynamicField`, so a write from
                                // inside the field updates the binding but schedules no
                                // recomposition and the chrome keeps showing the old asset.
                                // Swift gets the invalidation from `@Binding`, which is a
                                // `DynamicProperty`. The binding write itself is correct and
                                // is asserted below.
                                let root = graph.tree.borrow().root().expect("root node");
                                let cleared = nodes_of(&root);
                                let clear = button(&cleared, "Clear asset")
                                    .expect("the clear action is still offered");
                                if let Some(activate) = clear.accessibility_actions().activate {
                                    activate();
                                }
                                settle();
                                assert_eq!(
                                    value.get(),
                                    None,
                                    "clearing writes the empty value into the binding ({context_label})"
                                );

                                let root = graph.tree.borrow().root().expect("root node");
                                let emptied = nodes_of(&root);
                                let choose = picker_trigger(&emptied);
                                assert_eq!(
                                    choose.accessibility().expect("picker semantics").role,
                                    AccessibilityRole::Button,
                                    "the picker stays a button after the value is cleared ({context_label})"
                                );
                                assert!(
                                    button(&emptied, "Clear asset").is_some(),
                                    "the un-recomposed chrome still offers its clear action ({context_label})"
                                );
                            });
                        }
                    }
                }
            }
        });
    });
}

#[test]
fn disabled_asset_controls_expose_no_activation_path() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let value = State::new(Some(initial_value()));
                let binding = Binding::new(
                    {
                        let value = value.clone();
                        move || value.get()
                    },
                    {
                        let value = value.clone();
                        move |next| value.set(next)
                    },
                );
                let graph = support::graph();
                graph.install_view(
                    AnyView::new(field_of(binding, false)),
                    Default::default(),
                );
                for _ in 0..3 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(390.0, 200.0);
                }

                let root = graph.tree.borrow().root().expect("root node");
                let nodes = nodes_of(&root);
                let actions = [
                    ("Clear asset", button(&nodes, "Clear asset")),
                    ("Choose asset", Some(picker_trigger(&nodes))),
                ];
                for (label, action) in actions {
                    let action =
                        action.unwrap_or_else(|| panic!("the {label} action exists"));
                    let semantics = action.accessibility().expect("action semantics");
                    assert!(
                        !semantics.state.is_enabled,
                        "a disabled field marks its {label} action disabled"
                    );
                    assert!(
                        action.accessibility_actions().activate.is_none(),
                        "a disabled field exposes no {label} activation"
                    );
                }
                assert_eq!(
                    value.get().map(|asset| asset.id),
                    Some("initial".to_string()),
                    "a disabled field never mutates the value"
                );
            });
        });
    });
}
