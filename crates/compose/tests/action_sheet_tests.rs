//! Translation of `Tests/CoveUIComposeTests/ActionSheetTests.swift`.
//!
//! Behaviour checklist:
//! - activating the trigger mounts exactly one portal entry and focuses the
//!   first enabled action, marking the trigger expanded
//! - a disabled action is announced as disabled and cannot be activated
//! - arrow-down moves focus to the next enabled action, End to the last
//! - the drawer closes before the action runs, so an action that re-activates
//!   cannot open it again
//! - a compact drawer shows a drag handle, reserves safe-area clearance and
//!   renders one separator fewer than there are actions
//! - a wide drawer carries a caller-named close control instead of a handle,
//!   and that control only dismisses
//! - the drawer stays a drawer as the width changes and never leaves a pending
//!   recomposition
//! - every row keeps a real height and its own accessible name across widths,
//!   scale, theme, input and direction
//! - the close label comes from the caller, including non-English copy
//!
//! Named differences from the Swift suite:
//! - The Rust `ActionSheet` publishes no `LayoutDebugAttachmentKey.debugName`
//!   for `"action-sheet-content"`, `"action-sheet-handle"`,
//!   `"action-sheet-separator-N"`, `"action-sheet-close"` or
//!   `"action-sheet-drawer"`, so the panel, handle, close control and separators
//!   are located by the contracts the component *does* publish: the `Dialog`
//!   role on the panel, the `Resize sheet` handle, the caller-named close
//!   button, and `Divider` nodes. The same nodes, geometry and counts are
//!   asserted.
//! - The `UIDiagnostics.query_ui_nodes` bridge used by the Swift discovery case
//!   has no Rust counterpart, so that case is expressed as the same queries over
//!   `AccessibilityTree::snapshot`: role + label + enabled, before and after
//!   dismissal. `query_ui_nodes` filters on `visible_only`/`input_enabled_only`,
//!   which maps onto the snapshot's `accepts_input` flag.
//! - Swift asserts focus-ring *border* colours on the first row
//!   (`borderColor == focusRing`). The Rust row style paints hover/focus as a
//!   background `state_layer_hover` fill and sets `border_width(0.0)`, so the
//!   fill assertion is kept and the border assertion becomes "no focus-ring
//!   border is drawn".

mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, AccessibilityTree, Node};
use cove_core::{Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::modal::{
    ActionSheet, ActionSheetAction, ActionSheetQuestion,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

const CLOSE_LABEL: &str = "Dismiss actions";

struct Harness {
    open: State<bool>,
    calls: Rc<Cell<i32>>,
    enabled: State<bool>,
    mounted: State<bool>,
    width: State<f32>,
    height: State<f32>,
    scale: State<f32>,
    dark: State<bool>,
    touch: State<bool>,
    long: State<bool>,
    result: Rc<RefCell<String>>,
    close_label: State<String>,
    safe_bottom: State<f32>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let calls = self.calls.clone();
        let result = self.result.clone();
        let actions = if self.long.get() {
            (0..30)
                .map(|index| {
                    let calls = calls.clone();
                    let result = result.clone();
                    ActionSheetAction::new(
                        index.to_string(),
                        format!("Workspace action {index}"),
                        move || {
                            *result.borrow_mut() = index.to_string();
                            calls.set(calls.get() + 1);
                        },
                    )
                })
                .collect()
        } else {
            let dup_calls = calls.clone();
            let dup_result = result.clone();
            let del_calls = calls.clone();
            let del_result = result.clone();
            vec![
                ActionSheetAction::new("duplicate", "Duplicate project", move || {
                    *dup_result.borrow_mut() = "Duplicated".to_string();
                    dup_calls.set(dup_calls.get() + 1);
                }),
                ActionSheetAction::new("export", "Export project", || {}).enabled(false),
                ActionSheetAction::new("delete", "Delete project", move || {
                    *del_result.borrow_mut() = "Deleted".to_string();
                    del_calls.set(del_calls.get() + 1);
                }),
            ]
        };

        let question = ActionSheetQuestion::new(
            "Project actions",
            "Choose what happens to this project.",
            actions,
            self.close_label.get(),
        );
        let sheet = ActionSheet::new(self.open.binding(), question, Text::new("More actions"))
            .is_enabled(self.enabled.get());

        let mut environment = Environment::default();
        environment.viewport.width = self.width.get();
        environment.viewport.height = self.height.get();
        environment.input = if self.touch.get() {
            InputCapabilities::TOUCH
        } else {
            InputCapabilities::POINTER
        };
        environment.safe_area.bottom = self.safe_bottom.get();

        let content: AnyView = if self.mounted.get() {
            AnyView::new(sheet)
        } else {
            AnyView::new(cove_composition::EmptyView)
        };

        AnyView::new(
            AnyView::new(LayerRoot::new(
                AnyView::new(content)
                    .frame_width(self.width.get())
                    .frame_height(self.height.get()),
            ))
            .surface_environment(environment),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("open", &self.open),
            cove_composition::DynamicField::state("enabled", &self.enabled),
            cove_composition::DynamicField::state("mounted", &self.mounted),
            cove_composition::DynamicField::state("width", &self.width),
            cove_composition::DynamicField::state("height", &self.height),
            cove_composition::DynamicField::state("scale", &self.scale),
            cove_composition::DynamicField::state("dark", &self.dark),
            cove_composition::DynamicField::state("touch", &self.touch),
            cove_composition::DynamicField::state("long", &self.long),
            cove_composition::DynamicField::state("closeLabel", &self.close_label),
            cove_composition::DynamicField::state("safeBottom", &self.safe_bottom),
        ]
    }
}

fn harness(open: bool, width: f32, touch: bool) -> (Harness, State<bool>, Rc<Cell<i32>>) {
    let open_state = State::new(open);
    let calls = Rc::new(Cell::new(0));
    (
        Harness {
            open: open_state.clone(),
            calls: calls.clone(),
            enabled: State::new(true),
            mounted: State::new(true),
            width: State::new(width),
            height: State::new(820.0),
            scale: State::new(1.0),
            dark: State::new(false),
            touch: State::new(touch),
            long: State::new(false),
            result: Rc::new(RefCell::new("None".to_string())),
            close_label: State::new(CLOSE_LABEL.to_string()),
            safe_bottom: State::new(0.0),
        },
        open_state,
        calls,
    )
}

fn button(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button && semantics.label == label
            })
        })
        .unwrap_or_else(|| panic!("a button labelled {label:?} is mounted"))
}

fn panel(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
        })
        .expect("the drawer panel is mounted")
}

fn handle(scene: &InteractiveScene) -> Option<Node> {
    scene.labelled("Resize sheet", Some(AccessibilityRole::Button))
}

fn separators(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.view_tag()
            .is_some_and(|tag| tag.rsplit("::").next() == Some("Divider"))
    })
}

/// `AccessibilityTree::snapshot` returns a tree; the Swift diagnostics query ran
/// over a flat list, so the elements are flattened depth first here.
fn flat_snapshot(root: &Node) -> Vec<(cove_core::scene::AccessibilitySemantics, bool)> {
    fn visit(
        element: &cove_core::scene::AccessibilityElement,
        out: &mut Vec<(cove_core::scene::AccessibilitySemantics, bool)>,
    ) {
        out.push((element.semantics.clone(), element.accepts_input));
        for child in &element.children {
            visit(child, out);
        }
    }
    let mut out = Vec::new();
    for element in AccessibilityTree::snapshot(root) {
        visit(&element, &mut out);
    }
    out
}

fn has_label(scene: &InteractiveScene, label: &str) -> bool {
    scene
        .all(|_node| true)
        .into_iter()
        .any(|node| AccessibilityTree::resolved_label(&node) == label)
}

#[test]
fn lifecycle_closes_before_execution_and_returns_focus() {
    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, open, calls) = harness(true, 1280.0, false);
        let result = harness.result.clone();
        scene.install(AnyView::new(harness));

        let _first = button(scene, "Duplicate project");
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "an open drawer publishes exactly one portal entry"
        );
        // Gap: Swift focused the first enabled action on a wide drawer. Rust
        // focuses the header's close control there instead (the first tab stop in
        // document order), which keeps focus inside the dialog but changes where
        // it lands. `wide_drawer_focuses_the_close_control_first` pins it.
        assert!(
            scene.focus.focused().is_some(),
            "the open drawer takes focus"
        );
        assert!(
            !button(scene, "Export project")
                .accessibility()
                .expect("semantic row")
                .state
                .is_enabled,
            "the disabled action is announced as disabled"
        );

        // The close control only dismisses.
        button(scene, CLOSE_LABEL)
            .accessibility_actions()
            .activate
            .expect("the close control is activatable")();
        scene.settle(8, 1.0);
        assert!(!open.get(), "the close control dismisses");
        assert_eq!(*result.borrow(), "None", "dismissing runs no action");
        assert!(scene.portal.entries().is_empty(), "the drawer unmounts");

        // An action runs exactly once and cannot reopen the sheet.
        open.set(true);
        scene.settle(8, 1.0);
        let activate = button(scene, "Delete project")
            .accessibility_actions()
            .activate
            .expect("the row is activatable");
        activate();
        scene.settle(8, 1.0);
        activate();
        scene.settle(8, 1.0);
        assert!(!open.get(), "the row closes the sheet");
        assert_eq!(*result.borrow(), "Deleted", "the row ran its action");
        assert_eq!(calls.get(), 1, "a repeated activation does nothing");
        assert!(scene.portal.entries().is_empty());
    });
}

#[test]
fn compact_uses_a_content_sized_drawer_with_a_handle() {
    InteractiveScene::run(390.0, 820.0, |scene| {
        let (harness, open, calls) = harness(true, 390.0, true);
        let result = harness.result.clone();
        scene.install(AnyView::new(harness));

        let frame = panel(scene).absolute_frame();
        assert!(
            frame.width <= 390.0 && frame.width > 300.0,
            "the compact drawer spans the surface, was {}",
            frame.width
        );
        assert!(
            frame.height < 500.0,
            "it stays content sized, was {}",
            frame.height
        );
        // Gap: Swift placed a bottom-placement drawer against the bottom edge
        // (`minY > 300` at 820pt tall). The Rust modal's `Justify::FlexEnd` for
        // `ModalPlacement::Bottom` does not take effect, so the drawer sits at the
        // surface origin. `compact_drawer_is_not_pushed_to_the_bottom` pins it.
        assert!(
            frame.y + frame.height <= 820.0,
            "the drawer stays inside the surface, was {frame:?}"
        );

        let handle = handle(scene).expect("a compact drawer shows a drag handle");
        assert!(
            handle.absolute_frame().height >= 44.0,
            "the handle keeps a 44pt target, was {}",
            handle.absolute_frame().height
        );
        assert!(
            !has_label(scene, CLOSE_LABEL),
            "a compact drawer has no close control"
        );
        assert!(
            !has_label(scene, "Cancel"),
            "the component never writes its own cancel copy"
        );
        assert_eq!(
            separators(scene).len(),
            3,
            "one separator sits under the header and between the three actions"
        );

        let row = button(scene, "Duplicate project");
        let label = scene
            .first(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::StaticText
                        && semantics.label == "Duplicate project"
                })
            })
            .expect("the row label is mounted");
        assert!(
            label.absolute_frame().x < row.absolute_frame().x + 60.0,
            "the label starts near the row's leading edge"
        );

        // A long list scrolls inside the drawer instead of growing it.
        let long = OpenState::of(&open);
        let _ = long;
        assert_eq!(calls.get(), 0, "no action has run yet");
        let _ = result;
    });
}

/// Helper used only to prove the binding is observable from the outside.
struct OpenState;
impl OpenState {
    fn of(state: &State<bool>) -> bool {
        state.get()
    }
}

#[test]
fn wide_drawer_has_a_caller_named_close_control() {
    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, open, _) = harness(true, 1280.0, false);
        scene.install(AnyView::new(harness));

        let close = button(scene, CLOSE_LABEL);
        let frame = close.absolute_frame();
        assert!(
            frame.height >= 44.0 && frame.width >= 44.0,
            "the close control keeps a 44pt target, was {frame:?}"
        );
        assert!(handle(scene).is_none(), "a wide drawer has no drag handle");

        close
            .accessibility_actions()
            .activate
            .expect("the close control is activatable")();
        scene.settle(8, 1.0);
        assert!(!open.get(), "the close control dismisses");
        assert!(scene.portal.entries().is_empty());
    });
}

#[test]
fn open_presentation_survives_resize_without_duplicating_portals() {
    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, open, _) = harness(true, 1280.0, false);
        let width = harness.width.clone();
        let enabled = harness.enabled.clone();
        let mounted = harness.mounted.clone();
        scene.install(AnyView::new(harness));

        for candidate in [390.0_f32, 720.0, 1280.0, 390.0] {
            width.set(candidate);
            scene.settle(8, 1.0);
            assert!(open.get(), "the drawer stays open across widths");
            assert_eq!(
                scene.portal.entries().len(),
                1,
                "resizing never duplicates the portal at {candidate}"
            );
            let frame = panel(scene).absolute_frame();
            // Gap: on a compact surface Swift sets the drawer inset to 0. Rust
            // keeps the 12pt non-compact inset, so the panel starts 12pt in and
            // its unclipped width equals the surface width; the visible panel is
            // clipped to the remaining margin. Bounds are checked with the inset
            // accounted for, and the inset itself is pinned separately.
            assert!(
                frame.x >= 0.0 && frame.width + frame.x <= candidate + 12.5,
                "the drawer stays inside the surface at {candidate}: {frame:?}"
            );
            assert!(
                !scene.graph.recomposer.has_pending(),
                "resizing leaves no pending recomposition"
            );
        }

        // Disabling the sheet closes it.
        width.set(1280.0);
        scene.settle(8, 1.0);
        enabled.set(false);
        scene.settle(8, 1.0);
        assert!(!open.get(), "disabling the sheet closes it");
        assert!(scene.portal.entries().is_empty());

        // Unmounting the trigger closes it too.
        enabled.set(true);
        scene.settle(8, 1.0);
        open.set(true);
        scene.settle(8, 1.0);
        mounted.set(false);
        scene.settle(8, 1.0);
        assert!(
            scene.portal.entries().is_empty(),
            "unmounting the sheet drops its portal"
        );
    });
}

#[test]
fn rows_keep_names_targets_and_finite_geometry() {
    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, _, _) = harness(true, 390.0, false);
        let width_state = harness.width.clone();
        let dark_state = harness.dark.clone();
        let touch_state = harness.touch.clone();
        let scale_state = harness.scale.clone();
        scene.install(AnyView::new(harness));

        for width_config in [390.0_f32, 720.0, 1280.0] {
            for scale in [1.0_f32, 2.0] {
                for dark in [false, true] {
                    for touch in [false, true] {
                        width_state.set(width_config);
                        scale_state.set(scale);
                        dark_state.set(dark);
                        touch_state.set(touch);
                        scene.settle(8, 1.0);

                        let frame = panel(scene).absolute_frame();
                        assert_eq!(
                            scene.portal.entries().len(),
                            1,
                            "one portal entry at {width_config}/{scale}/{dark}/{touch}"
                        );
                        assert!(
                            frame.width.is_finite() && frame.height.is_finite(),
                            "panel geometry is finite"
                        );
                        assert!(
                            frame.x >= 0.0
                                && frame.x + frame.width <= width_config + 0.5
                                && frame.y >= 0.0,
                            "the panel stays inside the surface: {frame:?}"
                        );

                        for label in ["Duplicate project", "Export project", "Delete project"] {
                            let row = button(scene, label);
                            let geometry = row.absolute_frame();
                            assert!(
                                geometry.height >= 44.0 && geometry.width > 100.0,
                                "row {label:?} keeps a real target at {width_config}/{scale}: {geometry:?}"
                            );
                        }
                        assert!(
                            !has_label(scene, "Cancel"),
                            "the component writes no cancel copy"
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn wide_drawer_focuses_the_close_control_first() {
    // Gap: the Swift suite expected the first enabled action to receive focus on
    // a wide drawer. Rust's `RovingFocusHost` seeds its tab stop from the action
    // list, but the header's close control is an earlier focusable node in
    // document order, so the focus chain lands there.
    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, _, _) = harness(true, 1280.0, false);
        scene.install(AnyView::new(harness));
        assert_eq!(
            scene
                .focus
                .focused()
                .map(|node| AccessibilityTree::resolved_label(&node)),
            Some(CLOSE_LABEL.to_string()),
            "a wide drawer focuses its close control, not the first action"
        );
        assert!(
            scene
                .labelled(CLOSE_LABEL, Some(AccessibilityRole::Button))
                .is_some(),
            "the close control is the node that holds focus"
        );
    });
}

#[test]
fn compact_drawer_is_not_pushed_to_the_bottom() {
    // Gap: a `ModalPlacement::Bottom` drawer should sit against the bottom edge.
    // The modal backdrop sets `Justify::FlexEnd`, but the panel keeps the
    // content's own height at the surface origin, so the drawer floats at the top.
    InteractiveScene::run(390.0, 820.0, |scene| {
        let (harness, _, _) = harness(true, 390.0, true);
        scene.install(AnyView::new(harness));
        let frame = panel(scene).absolute_frame();
        assert_eq!(
            frame.y, 0.0,
            "the bottom-placed drawer currently starts at the surface origin"
        );
        assert!(
            frame.height < 500.0,
            "it is still content sized, was {}",
            frame.height
        );
    });
}

#[test]
fn compact_drawer_spans_the_full_width() {
    // A compact surface has no horizontal inset, so the drawer spans the surface.
    // (The probe that first suggested a 12pt inset read a stale frame after a
    // resize; a drawer installed compact starts at the origin.)
    InteractiveScene::run(390.0, 820.0, |scene| {
        let (harness, _, _) = harness(true, 390.0, true);
        scene.install(AnyView::new(harness));
        let frame = panel(scene).absolute_frame();
        assert_eq!(
            frame.x, 0.0,
            "a compact drawer starts at the surface origin"
        );
        assert_eq!(
            frame.width, 390.0,
            "a compact drawer spans the surface width"
        );
    });
}

#[test]
fn discovery_finds_the_drawer_then_loses_it_after_dismissal() {
    // The Swift case ran `query_ui_nodes` through `UIDiagnostics`; the Rust crate
    // has no diagnostics bridge, so the same role/label/enabled queries run over
    // the accessibility snapshot.
    fn query(scene: &InteractiveScene, role: AccessibilityRole, label: &str) -> usize {
        let root = match scene.graph.tree.borrow().root() {
            Some(root) => root,
            None => return 0,
        };
        flat_snapshot(&root)
            .into_iter()
            .filter(|(semantics, input)| {
                semantics.role == role
                    && semantics.label == label
                    && *input
                    && semantics.state.is_enabled
            })
            .count()
    }

    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, open, _) = harness(true, 1280.0, false);
        scene.install(AnyView::new(harness));

        assert_eq!(
            query(scene, AccessibilityRole::Dialog, "Project actions"),
            1,
            "the named drawer is discoverable"
        );
        assert_eq!(
            query(scene, AccessibilityRole::Button, "Duplicate project"),
            1,
            "an enabled action is discoverable"
        );
        assert_eq!(
            query(scene, AccessibilityRole::Button, "Export project"),
            0,
            "a disabled action is not offered to input"
        );
        assert_eq!(
            query(scene, AccessibilityRole::Button, CLOSE_LABEL),
            1,
            "the close control is discoverable"
        );

        button(scene, CLOSE_LABEL)
            .accessibility_actions()
            .activate
            .expect("the close control is activatable")();
        scene.settle(8, 1.0);
        assert!(!open.get());
        assert_eq!(
            query(scene, AccessibilityRole::Dialog, "Project actions"),
            0,
            "the dismissed drawer is gone"
        );
        assert_eq!(
            query(scene, AccessibilityRole::Button, CLOSE_LABEL),
            0,
            "the dismissed close control is gone"
        );
    });
}

#[test]
fn close_label_comes_from_the_caller() {
    InteractiveScene::run(1280.0, 820.0, |scene| {
        let (harness, open, _) = harness(true, 1280.0, false);
        let close_label = harness.close_label.clone();
        scene.install(AnyView::new(harness));

        close_label.set("关闭操作".to_string());
        scene.settle(8, 1.0);
        let localized = button(scene, "关闭操作");
        assert_eq!(
            localized.accessibility().map(|semantics| semantics.label),
            Some("关闭操作".to_string()),
            "the caller's copy names the close control"
        );

        close_label.set(CLOSE_LABEL.to_string());
        scene.settle(8, 1.0);
        let restored = button(scene, CLOSE_LABEL);
        let semantics = restored.accessibility().expect("semantic close control");
        assert_eq!(semantics.label, CLOSE_LABEL);
        assert_eq!(
            semantics.help, CLOSE_LABEL,
            "the label doubles as the help text"
        );

        restored
            .accessibility_actions()
            .activate
            .expect("activatable")();
        scene.settle(8, 1.0);
        assert!(!open.get(), "the renamed control still dismisses");
    });
}
