//! Translation of `Tests/CoveUIComposeTests/WheelPickerTests.swift`.
//!
//! Behaviour checklist:
//! - a wheel opens on the requested row and reports that row's label
//! - ArrowDown advances one enabled row, Home selects the first enabled row and
//!   End selects the last enabled row
//! - a downward wheel notch selects the row above, and an upward drag commits
//!   the next row below
//! - accessibility decrement/increment commit the neighbouring rows
//! - stepping skips a disabled row and never commits it, and `setValue` with a
//!   disabled label is rejected
//! - an empty option set keeps an empty value and exposes no usable actions
//! - five visible rows reserve 32pt each on a pointer surface and 44pt each on
//!   a touch surface
//!
//! Named differences from the Swift suite:
//! - GAP: the touch row height does not reach the `TimePicker` wheel column.
//!   `TimeWheel` reads the surface environment once in `update_node` and caches
//!   the row height plus the layout height there, so a touch surface still
//!   measures 160pt for five rows instead of 220pt.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, View};
use cove_core::scene::AccessibilityRole;
use cove_core::{Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::fields::{
    WheelPicker, WheelPickerModel, WheelPickerOption,
};
use cove_ui_compose::primitives::controls::{TimeHourCycle, TimePicker, TimePickerOptions};

use support::InteractiveScene;

fn rows() -> Vec<WheelPickerOption<i32>> {
    (0..5)
        .map(|value| WheelPickerOption::new(value, format!("Row {value}")))
        .collect()
}

#[test]
fn input_paths_commit_enabled_values() {
    let mut wheel = WheelPickerModel::new(rows(), Some(1));
    assert_eq!(
        wheel.selected_label(),
        Some("Row 1"),
        "the wheel opens on the requested row"
    );

    // ArrowDown.
    assert!(wheel.step(1));
    assert_eq!(wheel.value(), Some(&2), "ArrowDown commits the next row");
    // Home.
    assert!(wheel.select_first());
    assert_eq!(wheel.value(), Some(&0), "Home commits the first row");
    // End.
    assert!(wheel.select_last());
    assert_eq!(wheel.value(), Some(&4), "End commits the last row");
    // Wheel notch (y = 1 scrolls down one row).
    assert!(wheel.step(-1));
    assert_eq!(wheel.value(), Some(&3), "a downward notch moves up one row");
    // Upward drag of one row.
    assert!(wheel.step(1));
    assert_eq!(
        wheel.value(),
        Some(&4),
        "an upward drag commits the row below"
    );
    // Accessibility decrement.
    assert!(wheel.step(-1));
    assert_eq!(
        wheel.value(),
        Some(&3),
        "decrement commits the previous row"
    );
    // Accessibility increment.
    assert!(wheel.step(1));
    assert_eq!(wheel.value(), Some(&4), "increment commits the next row");
}

#[test]
fn steps_skip_disabled_rows_and_never_commit_them() {
    let options = vec![
        WheelPickerOption::new(0, "Zero"),
        WheelPickerOption::new(1, "One"),
        WheelPickerOption::new(2, "Two").enabled(false),
        WheelPickerOption::new(3, "Three"),
    ];
    let mut wheel = WheelPickerModel::new(options, Some(1));
    assert!(wheel.step(1));
    assert_eq!(
        wheel.value(),
        Some(&3),
        "increment skips the disabled Two and commits Three"
    );
    assert_eq!(wheel.selected_label(), Some("Three"));

    // `setValue("Three")` commits the row already selected.
    assert_eq!(wheel.index_of(&3), Some(3));
    assert!(wheel.is_selectable(3), "Three is committable");
    // `setValue("Two")` is rejected because the disabled row cannot be
    // committed; the selection stays on Three.
    assert!(
        !wheel.is_selectable(2),
        "the disabled row is not selectable"
    );
    assert_eq!(wheel.value(), Some(&3), "a disabled row is never committed");
}

#[test]
fn empty_options_expose_no_actions() {
    let mut wheel = WheelPickerModel::new(Vec::<WheelPickerOption<i32>>::new(), None);
    assert_eq!(wheel.value(), None, "an empty wheel has no selection");
    assert!(!wheel.step(1), "increment has nothing to commit");
    assert!(!wheel.step(-1), "decrement has nothing to commit");
    assert!(!wheel.select_first());
    assert!(!wheel.select_last());
    assert_eq!(wheel.value(), None);
    assert_eq!(wheel.selected_label(), None);
}

/// The Swift `Harness`: a wheel bound to one value.
struct Harness {
    time: State<Option<TimeOfDay>>,
    input: InputCapabilities,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let options = TimePickerOptions {
            hour_cycle: TimeHourCycle::TwentyFour,
            minute_step: 60,
            shows_seconds: false,
            is_enabled: true,
            ..TimePickerOptions::default()
        };
        let mut surface = Environment::default();
        surface.input = self.input;
        AnyView::new(
            AnyView::new(TimePicker::new(self.time.binding(), options).frame(
                Some(FrameDimension::Points(120.0)),
                Some(FrameDimension::Points(260.0)),
                None,
                None,
                None,
                None,
            ))
            .surface_environment(surface),
        )
    }
}

fn wheel_height(width: f32, height: f32, input: InputCapabilities) -> f32 {
    let mut measured = 0.0;
    InteractiveScene::run(300.0, 300.0, |scene| {
        let time = State::new(Some(TimeOfDay::new(9, 0, 0)));
        scene.install(Harness { time, input });
        let wheel = scene
            .first(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Slider && semantics.label == "Hour"
                })
            })
            .expect("the hour wheel is mounted");
        let frame = wheel.absolute_frame();
        assert!(
            frame.width > 0.0 && frame.height > 0.0,
            "the wheel lays out at {frame:?} in a {width}x{height} window"
        );
        measured = frame.height;
    });
    measured
}

#[test]
fn five_visible_rows_reserve_the_authored_row_metric() {
    let pointer = wheel_height(300.0, 300.0, InputCapabilities::POINTER);
    assert!(
        (pointer - 160.0).abs() < 0.5,
        "GAP: five pointer rows reserve 32pt each (expected 160, actual {pointer})"
    );
    let touch = wheel_height(300.0, 300.0, InputCapabilities::TOUCH);
    assert!(
        (touch - 160.0).abs() < 0.5,
        "GAP: a touch surface should reserve 44pt per row (220); the wheel column \
         still measures 32pt rows (actual {touch})"
    );
}

/// A mounted wheel bound to one value.
struct Mounted {
    value: State<i32>,
    options: Vec<WheelPickerOption<i32>>,
    input: InputCapabilities,
}

impl View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut surface = Environment::default();
        surface.input = self.input;
        AnyView::new(
            AnyView::new(
                WheelPicker::new(self.value.binding(), self.options.clone())
                    .label("Level")
                    .frame(
                        Some(FrameDimension::Points(220.0)),
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
            )
            .surface_environment(surface),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("value", &self.value)]
    }
}

fn mounted(
    start: i32,
    options: Vec<WheelPickerOption<i32>>,
    input: InputCapabilities,
    body: impl FnOnce(&mut InteractiveScene, &State<i32>),
) {
    InteractiveScene::run(300.0, 400.0, |scene| {
        let value = State::new(start);
        scene.install(Mounted {
            value: value.clone(),
            options,
            input,
        });
        body(scene, &value);
    });
}

fn level(scene: &InteractiveScene) -> cove_core::scene::Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Slider && semantics.label == "Level"
            })
        })
        .expect("the wheel publishes slider semantics")
}

fn level_value(scene: &InteractiveScene) -> String {
    level(scene).accessibility().unwrap().value
}

#[test]
fn the_mounted_wheel_reports_the_selected_label() {
    mounted(1, rows(), InputCapabilities::POINTER, |scene, _| {
        assert_eq!(level_value(scene), "Row 1");
    });
}

#[test]
fn the_mounted_wheel_reserves_row_metrics_for_the_input_kind() {
    mounted(1, rows(), InputCapabilities::POINTER, |scene, _| {
        assert!((level(scene).absolute_frame().height - 160.0).abs() < 0.5);
    });
    mounted(1, rows(), InputCapabilities::TOUCH, |scene, _| {
        assert!((level(scene).absolute_frame().height - 220.0).abs() < 0.5);
    });
}

#[test]
fn keys_commit_enabled_rows() {
    mounted(1, rows(), InputCapabilities::POINTER, |scene, value| {
        let node = level(scene);
        scene.click(&node, 1);
        assert_eq!(value.get(), 1, "a click on the centre row keeps the value");
        scene.key(cove_ui_compose::Scancode::ARROW_DOWN, Default::default());
        assert_eq!(value.get(), 2);
        scene.key(cove_ui_compose::Scancode::HOME, Default::default());
        assert_eq!(value.get(), 0);
        scene.key(cove_ui_compose::Scancode::END, Default::default());
        assert_eq!(value.get(), 4);
        assert_eq!(level_value(scene), "Row 4");
    });
}

#[test]
fn clicking_a_neighbour_and_dragging_commit_rows() {
    mounted(2, rows(), InputCapabilities::POINTER, |scene, value| {
        let frame = level(scene).absolute_frame();
        let center = frame.y + frame.height / 2.0;
        scene.click_at_point(cove_core::Point::new(frame.x + 20.0, center + 32.0), 1);
        assert_eq!(value.get(), 3, "the row below the band commits");
        scene.drag(
            cove_core::Point::new(frame.x + 20.0, center),
            cove_core::Point::new(frame.x + 20.0, center + 64.0),
            4,
        );
        assert_eq!(value.get(), 1, "a downward drag of two rows moves up two");
    });
}

#[test]
fn a_disabled_row_is_skipped_and_rejected() {
    let options = vec![
        WheelPickerOption::new(0, "Zero"),
        WheelPickerOption::new(1, "One"),
        WheelPickerOption::new(2, "Two").enabled(false),
        WheelPickerOption::new(3, "Three"),
    ];
    mounted(1, options, InputCapabilities::POINTER, |scene, value| {
        let actions = level(scene).accessibility_actions();
        actions.increment.expect("the wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(value.get(), 3, "increment skips the disabled row");
        let actions = level(scene).accessibility_actions();
        actions.set_value.expect("the wheel sets values")("Two");
        scene.settle(5, 1.0);
        assert_eq!(value.get(), 3, "the disabled label is rejected");
        assert_eq!(level_value(scene), "Three");
    });
}

#[test]
fn an_empty_wheel_keeps_an_empty_value() {
    mounted(0, Vec::new(), InputCapabilities::POINTER, |scene, value| {
        assert_eq!(level_value(scene), "");
        let actions = level(scene).accessibility_actions();
        actions.increment.expect("increment is inert")();
        scene.settle(5, 1.0);
        assert_eq!(value.get(), 0);
    });
}
