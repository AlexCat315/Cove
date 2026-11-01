//! Translation of `Tests/CoveUIComposeTests/TimePickerTests.swift`.
//!
//! Behaviour checklist:
//! - in a 12-hour picker, incrementing the hour wheel moves 09:30 to 10:30,
//!   the minute wheel to 10:35 (honouring `minute_step`), the AM/PM wheel
//!   across noon to 22:35, and decrementing it returns to 10:35
//! - in a 24-hour picker that shows seconds, incrementing hours, seconds and
//!   then hours again clamps at the authored maximum 10:45:59
//!
//! The Rust wheels publish exactly the labels the Swift suite looked up
//! (`Hour`, `Minute`, `Second`, `AM or PM`) with role `Slider`, so the wheels are
//! located the same way.

mod support;

use cove_composition::{AnyView, View};
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::State;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::time_picker::{
    TimeHourCycle, TimePicker, TimePickerOptions,
};

use support::InteractiveScene;

/// The Swift `Harness`: owns the selection and applies the caller's options.
struct Harness {
    time: State<Option<TimeOfDay>>,
    cycle: TimeHourCycle,
    minute_step: i32,
    shows_seconds: bool,
    minimum: TimeOfDay,
    maximum: TimeOfDay,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        let options = TimePickerOptions {
            hour_cycle: self.cycle,
            minute_step: self.minute_step,
            shows_seconds: self.shows_seconds,
            is_enabled: true,
            minimum: self.minimum,
            maximum: self.maximum,
            label: "Time".to_string(),
        };
        AnyView::new(TimePicker::new(self.time.binding(), options).frame(
            Some(FrameDimension::Points(340.0)),
            Some(FrameDimension::Points(220.0)),
            None,
            None,
            None,
            None,
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("time", &self.time)]
    }
}

fn wheel(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Slider && semantics.label == label
            })
        })
        .unwrap_or_else(|| panic!("a wheel labelled {label:?} is mounted"))
}

#[test]
fn twelve_hour_wheels_commit_a_time_of_day() {
    InteractiveScene::run(380.0, 300.0, |scene| {
        let time = State::new(Some(TimeOfDay::new(9, 30, 0)));
        let read = time.clone();
        scene.install(AnyView::new(Harness {
            time: time.clone(),
            cycle: TimeHourCycle::Twelve,
            minute_step: 5,
            shows_seconds: false,
            minimum: TimeOfDay::new(0, 0, 0),
            maximum: TimeOfDay::new(23, 59, 59),
        }));
        // The harness install consumed the text environment, so re-read the value.
        let observed = || read.get();

        wheel(scene, "Hour")
            .accessibility_actions()
            .increment
            .expect("the hour wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(10, 30, 0)),
            "incrementing the hour wheel moves 09:30 to 10:30"
        );

        wheel(scene, "Minute")
            .accessibility_actions()
            .increment
            .expect("the minute wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(10, 35, 0)),
            "the minute wheel honours the 5-minute step"
        );

        wheel(scene, "AM or PM")
            .accessibility_actions()
            .increment
            .expect("the period wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(22, 35, 0)),
            "crossing AM to PM adds twelve hours"
        );

        wheel(scene, "AM or PM")
            .accessibility_actions()
            .decrement
            .expect("the period wheel decrements")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(10, 35, 0)),
            "decrementing the period returns to the morning"
        );
    });
}

#[test]
fn twenty_four_hour_mode_seconds_and_bounds_clamp() {
    InteractiveScene::run(380.0, 300.0, |scene| {
        let time = State::new(Some(TimeOfDay::new(9, 30, 0)));
        let read = time.clone();
        scene.install(AnyView::new(Harness {
            time: time.clone(),
            cycle: TimeHourCycle::TwentyFour,
            minute_step: 5,
            shows_seconds: true,
            minimum: TimeOfDay::new(8, 0, 0),
            maximum: TimeOfDay::new(10, 45, 59),
        }));
        let observed = || read.get();

        wheel(scene, "Hour")
            .accessibility_actions()
            .increment
            .expect("the hour wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(10, 30, 0)),
            "24-hour mode increments the hour directly"
        );

        wheel(scene, "Second")
            .accessibility_actions()
            .increment
            .expect("the second wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(10, 30, 1)),
            "the visible second wheel commits a second"
        );

        wheel(scene, "Hour")
            .accessibility_actions()
            .increment
            .expect("the hour wheel increments")();
        scene.settle(5, 1.0);
        assert_eq!(
            observed(),
            Some(TimeOfDay::new(10, 45, 59)),
            "the authored maximum clamps the committed time"
        );
    });
}
