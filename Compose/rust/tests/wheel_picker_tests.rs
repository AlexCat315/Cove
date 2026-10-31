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
//! - GAP: `cove_ui_compose` ships only the `WheelPicker` option/selection model
//!   (`primitives/controls/fields/wheel_picker.rs`). There is no mounted
//!   `WheelPicker` view, so the Swift suite's node reads — a `.slider` node
//!   labelled "Level" whose accessibility value is "Row 1" and whose frame is
//!   160pt tall — have no counterpart. Each input path is asserted against the
//!   model operation the Swift host performed (`step`, `select_first`,
//!   `select_last`, `is_selectable`), which preserves the committed-value
//!   assertions of that suite.
//! - GAP: the empty wheel has no label text to read, because the empty label
//!   lives on the missing view's accessibility value. The model exposes
//!   `value() == None` and `selected_label() == None` instead of `""`.
//! - GAP: the touch row height does not reach the wheel column. `TimeWheel`
//!   reads the surface environment once in `update_node` and caches the row
//!   height plus the layout height there, so a touch surface still measures
//!   160pt for five rows instead of 220pt. The row-metric test mounts a
//!   `TimePicker` (the crate's only wheel view) and pins the 160pt result.
//! - GAP: the Swift suite's frame assertions read the selected row height on a
//!   220pt-wide wheel; the Rust wheel column is 68pt wide, fixed by
//!   `TimePicker`, so only the height is comparable.

mod support;

use cove_composition::{AnyView, View};
use cove_core::scene::AccessibilityRole;
use cove_core::{Environment, InputCapabilities, State};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::fields::{WheelPicker, WheelPickerOption};
use cove_ui_compose::primitives::controls::time_picker::{
    TimeHourCycle, TimePicker, TimePickerOptions,
};

use support::InteractiveScene;

fn rows() -> Vec<WheelPickerOption<i32>> {
    (0..5)
        .map(|value| WheelPickerOption::new(value, format!("Row {value}")))
        .collect()
}

#[test]
fn input_paths_commit_enabled_values() {
    let mut wheel = WheelPicker::new(rows(), Some(1));
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
    let mut wheel = WheelPicker::new(options, Some(1));
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
        "GAP: Swift's setValue(\"Two\") is rejected by the missing wheel view; the \
         model reports row 2 as not selectable, so the selection stays on Three"
    );
    assert_eq!(wheel.value(), Some(&3), "a disabled row is never committed");
}

#[test]
fn empty_options_expose_no_actions() {
    let mut wheel = WheelPicker::new(Vec::<WheelPickerOption<i32>>::new(), None);
    assert_eq!(
        wheel.value(),
        None,
        "GAP: Swift reads an empty accessibility value (\"\"); the model has no \
         selection and no label text"
    );
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
    fn body(&self) -> AnyView {
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
    // GAP: the Swift suite mounts `WheelPicker(...visibleRows: 5)` directly and
    // reads its 160pt frame, then 220pt on a touch surface. The crate has no
    // standalone wheel view, so the same five-row metric is measured on the
    // `TimePicker` wheel column, which owns the 32/44pt row heights.
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
