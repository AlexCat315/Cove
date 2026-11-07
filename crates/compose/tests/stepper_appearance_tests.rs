//! Stepper appearance regressions: solid status markers, connectors that span
//! the real gap between steps, a continuous vertical rail, and selection
//! emphasis carried by the marker and title instead of a row fill.

use crate::support;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Binding, Color, Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::navigation::stepper::{
    StepStatus, Stepper, StepperAxis, StepperItem, StepperOptions,
};
use cove_ui_compose::theme::theme_for_node;

use support::InteractiveScene;

fn layout(width: f32, height: f32, root: AnyView) -> AnyView {
    let mut surface = Environment::default();
    surface.viewport.width = width;
    surface.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
    AnyView::new(
        AnyView::new(root)
            .frame_size(width, height)
            .surface_environment(surface),
    )
}

fn selection() -> (State<Option<String>>, Binding<Option<String>>) {
    let selected = State::new(Some("details".to_string()));
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
        StepperItem::new("account".to_string(), "Account")
            .status(StepStatus::Completed)
            .detail("Profile saved"),
        StepperItem::new("details".to_string(), "Details").detail("Configure your workspace"),
        StepperItem::new("review".to_string(), "Review").detail("Ready to review"),
    ]
}

fn tagged(scene: &InteractiveScene, type_name: &str) -> Vec<Node> {
    scene.all(|node| node.view_tag().is_some_and(|tag| tag.contains(type_name)))
}

#[test]
fn adaptive_compact_branch_stacks_steps_without_collapsing() {
    let (_selected, binding) = selection();
    let stepper = AnyView::new(Stepper::try_new(steps(), binding).expect("unique ids"));
    let page = AnyView::new(cove_composition::BoxView::new(
        cove_core::scene::FlexDirection::Column,
        cove_core::scene::Align::Stretch,
        cove_core::scene::Justify::FlexStart,
        cove_core::scene::Wrap::NoWrap,
        24.0,
        AnyView::new(cove_composition::TupleView::new(vec![
            stepper,
            AnyView::new(
                cove_ui_compose::primitives::text::Text::new("Below the stepper".to_string())
                    .debug_name("below"),
            ),
        ])),
    ));
    InteractiveScene::run(476.0, 820.0, |scene| {
        scene.install(layout(476.0, 820.0, page));
        let cells = tagged(scene, "StepperChrome");
        assert_eq!(cells.len(), 3, "the compact branch keeps every step");
        let mut previous_bottom: Option<f32> = None;
        for cell in &cells {
            let frame = cell.absolute_frame();
            assert!(
                frame.height >= 32.0,
                "a stacked step keeps its marker's full height, got {:.1}",
                frame.height
            );
            if let Some(bottom) = previous_bottom {
                assert!(
                    (frame.y - bottom).abs() < 0.5,
                    "stacked steps follow each other instead of overlapping, {:.1} vs {:.1}",
                    frame.y,
                    bottom
                );
            }
            previous_bottom = Some(frame.y + frame.height);
        }
        let block = tagged(scene, "ContainerSizeProbe")
            .into_iter()
            .next()
            .expect("the stepper's measured block");
        let block_bottom = block.absolute_frame().y + block.absolute_frame().height;
        assert!(
            block_bottom >= previous_bottom.unwrap(),
            "the stacked steps stay inside the stepper's own box"
        );
        let sibling = scene.named("below").expect("sibling after the stepper");
        assert!(
            sibling.absolute_frame().y >= block_bottom,
            "the content after a stacked stepper starts below it, {:.1} vs {:.1}",
            sibling.absolute_frame().y,
            block_bottom
        );
    });
}

#[test]
fn horizontal_connectors_span_the_gap_between_markers() {
    let (_selected, binding) = selection();
    let stepper = AnyView::new(Stepper::try_new(steps(), binding).expect("unique ids"));
    InteractiveScene::run(720.0, 600.0, |scene| {
        scene.install(layout(720.0, 600.0, stepper));
        let markers = tagged(scene, "StepperMarker");
        let lines = tagged(scene, "StepperConnector");
        assert_eq!(markers.len(), 3);
        assert_eq!(lines.len(), 2, "only the gaps between steps carry a line");
        let theme = theme_for_node(&markers[0]);
        for (index, line) in lines.iter().enumerate() {
            let frame = line.absolute_frame();
            let current = markers[index].absolute_frame();
            let next = markers[index + 1].absolute_frame();
            assert_eq!(frame.height, 1.0, "the connector is a hairline");
            assert!(
                frame.width > 24.0,
                "the connector spans the flex gap instead of a fixed 24pt dash, got {:.1}",
                frame.width
            );
            assert!(
                frame.x >= current.max_x(),
                "the connector starts after the current marker"
            );
            assert!(
                frame.max_x() <= next.x,
                "the connector never runs under the next marker"
            );
            assert!(
                next.x - frame.max_x() <= 17.0,
                "the connector reaches across the cell boundary, gap {:.1}",
                next.x - frame.max_x()
            );
            assert!(
                ((frame.y + frame.height / 2.0) - (current.y + current.height / 2.0)).abs() < 1.0,
                "the connector rides the marker's center line"
            );
        }
        assert_eq!(
            lines[0].background_color(),
            Some(theme.colors.success),
            "the line leaving a completed step is success green"
        );
        assert_eq!(
            lines[1].background_color(),
            Some(theme.colors.border),
            "the line leaving a waiting step stays neutral"
        );
    });
}

#[test]
fn vertical_rails_connect_marker_to_marker_without_a_break() {
    let (_selected, binding) = selection();
    let stepper = AnyView::new(
        Stepper::try_new(steps(), binding)
            .expect("unique ids")
            .options(StepperOptions {
                axis: StepperAxis::Vertical,
                ..StepperOptions::default()
            }),
    );
    InteractiveScene::run(360.0, 600.0, |scene| {
        scene.install(layout(360.0, 600.0, stepper));
        let markers = tagged(scene, "StepperMarker");
        let lines = tagged(scene, "StepperConnector");
        assert_eq!(markers.len(), 3);
        assert_eq!(lines.len(), 2, "the last step has no trailing rail");
        for (index, line) in lines.iter().enumerate() {
            let frame = line.absolute_frame();
            let current = markers[index].absolute_frame();
            let next = markers[index + 1].absolute_frame();
            assert_eq!(frame.width, 1.0, "the rail is a hairline");
            assert!(
                ((frame.x + frame.width / 2.0) - (current.x + current.width / 2.0)).abs() < 1.0,
                "the rail hangs directly under the marker"
            );
            assert!(
                frame.y >= current.max_y() - 0.5,
                "the rail starts at the marker's bottom edge, gap {:.1}",
                frame.y - current.max_y()
            );
            assert!(
                next.y - frame.max_y() <= 0.5,
                "the rail reaches the next marker's top edge, gap {:.1}",
                next.y - frame.max_y()
            );
        }
    });
}

#[test]
fn connectors_report_the_step_outcome() {
    let (_selected, binding) = selection();
    let items = vec![
        StepperItem::new("account".to_string(), "Account")
            .status(StepStatus::Completed)
            .detail("Profile saved"),
        StepperItem::new("details".to_string(), "Details")
            .status(StepStatus::Failed)
            .detail("Configure your workspace"),
        StepperItem::new("review".to_string(), "Review").detail("Complete details first"),
    ];
    for vertical in [false, true] {
        let stepper = AnyView::new(
            Stepper::try_new(items.clone(), binding.clone())
                .expect("unique ids")
                .options(StepperOptions {
                    axis: if vertical {
                        StepperAxis::Vertical
                    } else {
                        StepperAxis::Horizontal
                    },
                    ..StepperOptions::default()
                }),
        );
        InteractiveScene::run(720.0, 600.0, |scene| {
            scene.install(layout(720.0, 600.0, stepper));
            let markers = tagged(scene, "StepperMarker");
            let lines = tagged(scene, "StepperConnector");
            let theme = theme_for_node(&markers[0]);
            assert_eq!(lines.len(), 2);
            assert_eq!(
                lines[0].background_color(),
                Some(theme.colors.success),
                "the line leaving a completed step is success green ({vertical} axis)"
            );
            assert_eq!(
                lines[1].background_color(),
                Some(theme.colors.error),
                "the line leaving a failed step is error red ({vertical} axis)"
            );
        });
    }
}

#[test]
fn marker_fills_follow_status_and_selection() {
    let (_selected, binding) = selection();
    let items = vec![
        StepperItem::new("account".to_string(), "Account").status(StepStatus::Completed),
        StepperItem::new("details".to_string(), "Details"),
        StepperItem::new("review".to_string(), "Review"),
        StepperItem::new("audit".to_string(), "Audit").status(StepStatus::Failed),
    ];
    let stepper = AnyView::new(
        Stepper::try_new(items, binding)
            .expect("unique ids")
            .options(StepperOptions {
                axis: StepperAxis::Vertical,
                ..StepperOptions::default()
            }),
    );
    InteractiveScene::run(360.0, 600.0, |scene| {
        scene.install(layout(360.0, 600.0, stepper));
        let markers = tagged(scene, "StepperMarker");
        assert_eq!(markers.len(), 4);
        let theme = theme_for_node(&markers[0]);
        assert_eq!(
            markers[0].background_color(),
            Some(theme.colors.success),
            "a completed step is a solid success disc"
        );
        assert_eq!(markers[0].border_width(), 0.0);
        assert_eq!(
            markers[1].background_color(),
            Some(theme.colors.accent),
            "the selected waiting step is a solid accent disc"
        );
        assert_eq!(markers[1].border_width(), 0.0);
        assert_eq!(
            markers[2].background_color(),
            Some(Color::CLEAR),
            "an unselected waiting step has no fill"
        );
        assert_eq!(
            markers[2].border_color(),
            Some(theme.colors.border_strong),
            "a waiting step is outlined"
        );
        assert_eq!(markers[2].border_width(), 1.0);
        assert_eq!(
            markers[3].background_color(),
            Some(theme.colors.error),
            "a failed step is a solid error disc"
        );
        assert_eq!(markers[3].border_width(), 0.0);
    });
}

#[test]
fn selection_emphasizes_the_title_without_a_row_fill() {
    let (_selected, binding) = selection();
    let stepper = AnyView::new(Stepper::try_new(steps(), binding).expect("unique ids"));
    InteractiveScene::run(720.0, 600.0, |scene| {
        scene.install(layout(720.0, 600.0, stepper));
        let markers = tagged(scene, "StepperMarker");
        let theme = theme_for_node(&markers[0]);
        let selected_title = scene
            .labelled("Details", Some(AccessibilityRole::StaticText))
            .expect("the selected step's title");
        assert_eq!(
            selected_title.foreground_color(),
            Some(theme.colors.accent),
            "the selected step's title carries the accent"
        );
        let resting_title = scene
            .labelled("Account", Some(AccessibilityRole::StaticText))
            .expect("the completed step's title");
        let resting = resting_title.foreground_color();
        assert!(
            resting.is_none() || resting == Some(theme.colors.on_surface),
            "an unselected title keeps the inherited surface foreground, got {resting:?}"
        );
        let row_fill = theme.colors.accent.multiplied_alpha(0.08);
        assert!(
            scene
                .all(|node| node.background_color() == Some(row_fill))
                .is_empty(),
            "no step paints the old accent-tinted selection rectangle"
        );
    });
}
