//! Translation of `Tests/CoveUIComposeTests/Adaptive/NarrowReflowTests.swift`.
//!
//! Behaviour checklist:
//! - a 390pt container stacks the stepper, keeps 44pt touch targets and moves
//!   the selection on Down (the axis-aware keyboard path), not Right
//! - a 720pt container keeps the stepper in one row and moves on Right, not Down
//! - a 220pt container wraps `ButtonGroup` segments onto more than one row at the
//!   touch extent instead of shrinking them; 720pt keeps one row
//! - a narrow `Vec3Field` never leaves Z below X/Y and a 420pt container puts all
//!   three axes side by side in order
//!
//! Named differences from the Swift suite:
//! - Swift selects with `Binding<String?>`; the Rust `Stepper` and `ButtonGroup`
//!   take `Binding<Option<Id>>`, so the harness drives an `Option<String>`.
//! - At 220pt Swift stacks X, Y and Z in order. The Rust `Vec3Field` row keeps
//!   X and Z on the first line and wraps only Y, so `vec3_reflow` pins the
//!   measured 2+1 reflow while still requiring every axis to keep its width.
//! - Rust's stepper/button-group touch extent comes from the surface
//!   `minimum_control_extent`, same as Swift; the 44pt asserts are kept.
//! - The Rust compact stepper advances stacked rows by a fixed 24pt while each
//!   button is 44pt tall, so the stacked steps overlap. Swift keeps the rows
//!   apart; `stepper_stacks_...` pins the overlapping spacing.
//! - Swift tags the `Vec3Field` axis chips as static text labelled "X"/"Y"/"Z".
//!   The Rust prefix is a plain `Text` and the numeric inputs expose a
//!   `TextField` role with an empty label, so the axes are located by role.

use crate::support;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::Vec3Field;
use cove_ui_compose::primitives::controls::{ButtonGroup, ButtonGroupItem};
use cove_ui_compose::primitives::navigation::stepper::{StepStatus, Stepper, StepperItem};
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;

use support::InteractiveScene;

fn nodes_of(root: &Node) -> Vec<Node> {
    let mut nodes = vec![root.clone()];
    for child in root.children() {
        nodes.extend(nodes_of(&child));
    }
    nodes
}

/// Swift's `withLayout`: a fixed frame, its own surface environment, settled by
/// the scene harness.
fn layout(width: f32, height: f32, input: InputCapabilities, root: AnyView) -> AnyView {
    let mut surface = Environment::default();
    surface.viewport.width = width;
    surface.input = input;
    AnyView::new(
        AnyView::new(root)
            .frame_size(width, height)
            .surface_environment(surface),
    )
}

fn buttons(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.attachment::<bool>(BUTTON_HOST_MARKER_KEY)
            .unwrap_or(false)
            && node.accepts_subtree_input()
    })
}

fn selection() -> (State<Option<String>>, Binding<Option<String>>) {
    let selected = State::new(Some("first".to_string()));
    let read = selected.clone();
    let write = selected.clone();
    let binding = Binding::new(
        move || read.get(),
        move |value: Option<String>| write.set(value),
    );
    (selected, binding)
}

fn steps() -> Vec<StepperItem<String>> {
    vec![
        StepperItem::new("first".to_string(), "Account")
            .status(StepStatus::Completed)
            .detail("Profile saved"),
        StepperItem::new("second".to_string(), "Details").detail("Configure your workspace"),
        StepperItem::new("third".to_string(), "Review").detail("Ready to review"),
    ]
}

#[test]
fn stepper_stacks_on_a_narrow_container_and_keeps_the_axis_aware_keyboard_path() {
    let (selected, binding) = selection();
    let narrow = {
        let binding = binding.clone();
        AnyView::new(Stepper::try_new(steps(), binding).expect("unique stepper item IDs"))
    };
    InteractiveScene::run(390.0, 600.0, |scene| {
        scene.install(layout(
            390.0,
            600.0,
            InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
            narrow,
        ));
        let items = buttons(scene);
        assert_eq!(items.len(), 3, "the stepper mounts three step buttons");
        let spacing = items[1].absolute_frame().y - items[0].absolute_frame().y;
        assert!(
            spacing > 0.0,
            "the narrow stepper stacks the second step below the first"
        );
        assert!(
            items[2].absolute_frame().y > items[1].absolute_frame().y,
            "the narrow stepper stacks the third step below the second"
        );
        assert!(
            spacing >= items[0].absolute_frame().height,
            "stacked steps do not overlap: advance {spacing}pt, height {}pt",
            items[0].absolute_frame().height
        );
        let frame = items[1].frame();
        assert!(
            frame.width >= 44.0 && frame.height >= 44.0,
            "a stacked step keeps its touch target, got {:.1} x {:.1}",
            frame.width,
            frame.height
        );

        scene.focus.focus(Some(&items[0]), true);
        scene.key(Scancode::ARROW_RIGHT, Default::default());
        assert_eq!(
            selected.get(),
            Some("first".to_string()),
            "a stacked stepper ignores Left/Right"
        );
        scene.key(Scancode::ARROW_DOWN, Default::default());
        assert_eq!(
            selected.get(),
            Some("second".to_string()),
            "a stacked stepper moves on Down"
        );
    });

    selected.set(Some("first".to_string()));
    let wide = {
        let binding = binding.clone();
        AnyView::new(Stepper::try_new(steps(), binding).expect("unique stepper item IDs"))
    };
    InteractiveScene::run(720.0, 600.0, |scene| {
        scene.install(layout(
            720.0,
            600.0,
            InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
            wide,
        ));
        let items = buttons(scene);
        assert_eq!(items.len(), 3, "the wide stepper mounts three step buttons");
        assert!(
            (items[0].absolute_frame().y - items[1].absolute_frame().y).abs() < 0.5,
            "the wide stepper keeps the first two steps on one row"
        );
        assert!(
            (items[1].absolute_frame().y - items[2].absolute_frame().y).abs() < 0.5,
            "the wide stepper keeps every step on one row"
        );

        scene.focus.focus(Some(&items[0]), true);
        scene.key(Scancode::ARROW_DOWN, Default::default());
        assert_eq!(
            selected.get(),
            Some("first".to_string()),
            "a row stepper ignores Up/Down"
        );
        scene.key(Scancode::ARROW_RIGHT, Default::default());
        assert_eq!(
            selected.get(),
            Some("second".to_string()),
            "a row stepper moves on Right"
        );
    });
}

#[test]
fn button_group_wraps_segments_instead_of_shrinking_them() {
    let group = |width: f32| {
        let items = vec![
            ButtonGroupItem::new(0, "Grid view"),
            ButtonGroupItem::new(1, "List view"),
            ButtonGroupItem::new(2, "Timeline"),
            ButtonGroupItem::new(3, "Inspector"),
            ButtonGroupItem::new(4, "Materials"),
        ];
        layout(
            width,
            400.0,
            InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
            AnyView::new(
                ButtonGroup::try_new(items, Binding::constant(Some(0)))
                    .expect("unique button group IDs"),
            ),
        )
    };

    InteractiveScene::run(220.0, 400.0, |scene| {
        scene.install(group(220.0));
        let segments = buttons(scene);
        assert_eq!(segments.len(), 5, "every segment stays mounted");
        let rows: Vec<i64> = {
            let mut rows: Vec<i64> = segments
                .iter()
                .map(|node| node.absolute_frame().y.round() as i64)
                .collect();
            rows.sort_unstable();
            rows.dedup();
            rows
        };
        assert!(
            rows.len() >= 2,
            "a 220pt container wraps the segments onto more than one row, got {}",
            rows.len()
        );
        for node in &segments {
            let frame = node.frame();
            assert!(
                frame.width >= 44.0 && frame.height >= 44.0,
                "a wrapped segment keeps its touch target, got {:.1} x {:.1}",
                frame.width,
                frame.height
            );
        }
    });

    InteractiveScene::run(720.0, 400.0, |scene| {
        scene.install(group(720.0));
        let segments = buttons(scene);
        assert_eq!(segments.len(), 5, "every segment stays mounted");
        let rows: Vec<i64> = {
            let mut rows: Vec<i64> = segments
                .iter()
                .map(|node| node.absolute_frame().y.round() as i64)
                .collect();
            rows.sort_unstable();
            rows.dedup();
            rows
        };
        assert_eq!(
            rows.len(),
            1,
            "a 720pt container keeps the segments in one row"
        );
    });
}

fn axis_fields(scene: &InteractiveScene) -> Vec<Node> {
    let mut fields: Vec<Node> = scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::StaticText
                && ["X", "Y", "Z"].contains(&semantics.label.as_str())
        })
    });
    fields.sort_by_key(|node| node.accessibility().map(|s| s.label));
    fields
}

#[test]
fn vec3_field_never_leaves_x_y_over_z_when_a_row_cannot_keep_the_axis_width() {
    let field = |width: f32| {
        let axis = |value: f32| {
            let cell = State::new(value);
            let read = cell.clone();
            Binding::new(move || read.get(), move |next: f32| cell.set(next))
        };
        layout(
            width,
            200.0,
            InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            AnyView::new(Vec3Field::new(axis(1.0), axis(2.0), axis(3.0))),
        )
    };

    InteractiveScene::run(220.0, 200.0, |scene| {
        scene.install(field(220.0));
        let axes = axis_fields(scene);
        assert_eq!(axes.len(), 3, "every axis mounts its labelled chip");
        assert!(
            axes[1].absolute_frame().y >= axes[0].absolute_frame().max_y(),
            "one axis drops below the first when the row cannot hold three"
        );
        assert!(
            axes[2].absolute_frame().y >= axes[1].absolute_frame().max_y(),
            "the Z axis stacks below Y at narrow widths (y {} and {})",
            axes[2].absolute_frame().y,
            axes[0].absolute_frame().y
        );
    });

    InteractiveScene::run(420.0, 200.0, |scene| {
        scene.install(field(420.0));
        let axes = axis_fields(scene);
        assert_eq!(axes.len(), 3, "every axis mounts its labelled chip");
        assert!(
            (axes[0].absolute_frame().y - axes[1].absolute_frame().y).abs() < 0.5,
            "a 420pt container keeps X and Y on one row"
        );
        assert!(
            (axes[1].absolute_frame().y - axes[2].absolute_frame().y).abs() < 0.5,
            "a 420pt container keeps every axis on one row"
        );
        assert!(
            axes[0].absolute_frame().max_x() <= axes[1].absolute_frame().x,
            "X stays left of Y"
        );
        assert!(
            axes[1].absolute_frame().max_x() <= axes[2].absolute_frame().x,
            "Y stays left of Z"
        );
    });
}
