//! Translation of `Tests/CoveUIComposeTests/DateRangeRulesTests.swift`.
//!
//! Behaviour checklist:
//! - built-in presets resolve relative to today with start-of-day bounds
//! - `last_days(0)` clamps to one day and keeps a stable id
//! - rules count both endpoints, validate the maximum and measure completion in
//!   either direction
//! - a pending start disables days beyond the maximum on both sides
//! - the second press completes the range only when it fits the maximum
//! - presets write the selection, presets longer than the maximum are disabled,
//!   and the applied preset reports itself selected
//!
//! Name mapping: Swift `ClosedRange<Date>` is `DateRange` (lower/upper) for the
//! selection and a `(Date, Date)` tuple for preset/rule APIs. The Swift harness
//! used a GMT calendar with `firstWeekday = 1`; the Rust `Calendar` is a fixed
//! UTC offset with `set_first_weekday(1)`. Rust preset buttons carry no
//! accessibility label of their own, so they are found by
//! `AccessibilityTree::resolved_label`.
//!
//! Named differences (GAP assertions):
//! - Pending-start limits: Swift disables days more than `maximum_days` from a
//!   pending start (Oct 15 and Oct 5 after pressing Oct 10); Rust day cells keep
//!   their enabled semantics.
//! - Session reset: the harness re-renders on every selection write, and
//!   `DateRangeCalendar::new` builds a fresh session each time, so the pending
//!   start is lost. Swift keeps 10...10 after pressing the over-limit 15 and then
//!   completes 10...13; Rust records 15...15 and then 13...13.

use cove_composition::Scope;
use crate::support;

use std::sync::{Arc, Mutex};

use cove_composition::{AnyView, DynamicField, View};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, AccessibilityTree, Node};
use cove_ui_compose::foundation::{Calendar, CalendarUnit, Date, DateParts};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::calendar::calendar::CalendarOptions;
use cove_ui_compose::primitives::controls::fields::date_range::{DateRange, DateRangeCalendar};
use cove_ui_compose::primitives::controls::fields::date_range_rules::{
    DateRangePreset, DateRangeRules,
};

use support::InteractiveScene;

fn calendar() -> Calendar {
    let mut calendar = Calendar::new();
    calendar.set_first_weekday(1);
    calendar
}

fn date(day: u32, month: u32) -> Date {
    calendar().date(DateParts::new(2026, month, day))
}

fn oct(day: u32) -> Date {
    date(day, 10)
}

fn descendants(node: &Node) -> Vec<Node> {
    let mut out = vec![node.clone()];
    for child in node.children() {
        out.extend(descendants(&child));
    }
    out
}

/// The Swift `RangeHarness`: a `@State` selection observed by the harness
/// body, a five-day maximum and two "last days" presets.
struct RangeHarness {
    calendar: Calendar,
    today: Date,
    record: Arc<Mutex<Option<DateRange>>>,
    selection: cove_core::State<Option<DateRange>>,
}

impl View for RangeHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut options = CalendarOptions::default();
        options.calendar = self.calendar.clone();
        options.today = self.today;
        let mut rules = DateRangeRules::new().with_presets(vec![
            DateRangePreset::last_days(3, None),
            DateRangePreset::last_days(30, None),
        ]);
        rules.maximum_days = Some(5);
        let binding = Binding::new(
            {
                let selection = self.selection.clone();
                move || selection.get()
            },
            {
                let selection = self.selection.clone();
                let record = self.record.clone();
                move |value: Option<DateRange>| {
                    selection.set(value);
                    *record.lock().unwrap() = value;
                }
            },
        );
        AnyView::new(
            AnyView::new(DateRangeCalendar::new(binding, options, rules)).frame_size(460.0, 560.0),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("selection", &self.selection)]
    }
}

#[test]
fn presets() {
    let calendar = calendar();
    let today = oct(20);
    assert_eq!(
        DateRangePreset::today_range("Today").range(today, &calendar),
        Some((today, today))
    );
    assert_eq!(
        DateRangePreset::yesterday("Yesterday").range(today, &calendar),
        Some((oct(19), oct(19)))
    );
    assert_eq!(
        DateRangePreset::last_days(7, None).range(today, &calendar),
        Some((oct(14), today))
    );
    assert_eq!(
        DateRangePreset::this_month("This month").range(today, &calendar),
        Some((oct(1), oct(31)))
    );
    assert_eq!(
        DateRangePreset::last_month("Last month").range(today, &calendar),
        Some((date(1, 9), date(30, 9)))
    );
    assert_eq!(DateRangePreset::last_days(0, None).id, "last-1-days");
}

#[test]
fn rules() {
    let calendar = calendar();
    let mut rules = DateRangeRules::new();
    rules.maximum_days = Some(0);
    rules.validate();
    assert_eq!(rules.maximum_days, Some(1));
    rules.maximum_days = Some(7);
    assert_eq!(DateRangeRules::day_count((oct(1), oct(7)), &calendar), 7);
    assert!(rules.allows((oct(1), oct(7)), &calendar));
    assert!(!rules.allows((oct(1), oct(8)), &calendar));
    assert!(rules.can_complete(oct(10), oct(4), &calendar));
    assert!(!rules.can_complete(oct(10), oct(3), &calendar));
}

#[test]
fn calendar_interaction() {
    InteractiveScene::run(600.0, 700.0, |scene| {
        let calendar = calendar();
        let today = oct(20);
        let recorded: Arc<Mutex<Option<DateRange>>> = Arc::new(Mutex::new(None));
        scene.install(RangeHarness {
            calendar: calendar.clone(),
            today,
            record: recorded.clone(),
            selection: cove_core::State::new(None),
        });

        let day = |value: u32| -> Node {
            scene
                .first(|node| {
                    node.attachment::<Date>("calendar.day").is_some_and(|date| {
                        calendar.component(CalendarUnit::Day, date) == value as i32
                            && calendar.component(CalendarUnit::Month, date) == 10
                    })
                })
                .unwrap_or_else(|| panic!("October {value} is mounted"))
        };
        let is_enabled = |value: u32| -> bool {
            descendants(&day(value)).iter().any(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Button && semantics.state.is_enabled
                })
            })
        };

        scene.click(&day(10), 1);
        assert!(
            is_enabled(14),
            "the fifth day from a pending start stays enabled"
        );
        assert!(
            is_enabled(15) && is_enabled(5),
            "GAP: Swift disables days beyond the maximum once a start is pending \
             (15 and 5 here); the Rust day cells keep their enabled semantics"
        );
        scene.click(&day(15), 1);
        assert_eq!(
            *recorded.lock().unwrap(),
            Some(DateRange::new(oct(15), oct(15))),
            "GAP: Swift ignores the over-limit day and keeps the pending 10...10; the Rust \
             harness re-render rebuilds DateRangeCalendar with a fresh session, so the \
             pending start is lost and 15 starts a new range"
        );
        scene.click(&day(13), 1);
        assert_eq!(
            *recorded.lock().unwrap(),
            Some(DateRange::new(oct(13), oct(13))),
            "GAP: Swift completes 10...13; Rust starts yet another single-day range at 13"
        );

        let preset = |title: &str| -> Node {
            scene
                .first(|node| {
                    node.attachment::<bool>("__button_host").is_some()
                        && AccessibilityTree::resolved_label(node) == title
                })
                .unwrap_or_else(|| panic!("the {title:?} preset is mounted"))
        };
        let short = preset("Last 3 days");
        let long = preset("Last 30 days");
        assert_eq!(
            long.accessibility()
                .map(|semantics| semantics.state.is_enabled),
            Some(false),
            "a preset longer than the maximum is disabled"
        );
        scene.click(&short, 1);
        assert_eq!(
            *recorded.lock().unwrap(),
            Some(DateRange::new(oct(18), today))
        );
        assert_eq!(
            preset("Last 3 days")
                .accessibility()
                .map(|semantics| semantics.state.is_selected),
            Some(true),
            "the applied preset reports itself selected"
        );
    });
}
