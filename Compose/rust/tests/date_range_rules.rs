//! Behaviour tests for `DateRangePreset` / `DateRangeRules`.
//!
//! Translation of `Primitives/Controls/Fields/DateRange/Rules.swift`. Preset
//! ranges and inclusive day counts are calendar arithmetic, so a wrong month
//! length or an off-by-one at a boundary is silent without these.

use cove_ui_compose::foundation::{Calendar, Date, DateParts};
use cove_ui_compose::primitives::controls::fields::{DateRangePreset, DateRangeRules};

fn calendar() -> Calendar {
    Calendar::new()
}

/// A fixed civil date, built through the same conversion the picker uses.
fn day(year: i32, month: u32, day: u32) -> Date {
    Date::from_epoch_day(DateParts::new(year, month, day).epoch_day())
}

#[test]
fn today_resolves_to_a_single_day() {
    let today = day(2024, 3, 15);
    let range = DateRangePreset::today_range("Today")
        .range(today, &calendar())
        .unwrap();
    assert_eq!(range.0, range.1, "today is a one-day range");
}

#[test]
fn yesterday_moves_back_one_day() {
    let today = day(2024, 3, 1);
    let range = DateRangePreset::yesterday("Yesterday")
        .range(today, &calendar())
        .unwrap();
    // 1 March back one day is 29 February in a leap year.
    assert_eq!(range.0, day(2024, 2, 29));
    assert_eq!(range.0, range.1);
}

#[test]
fn last_days_includes_today() {
    let today = day(2024, 3, 10);
    let range = DateRangePreset::last_days(7, None)
        .range(today, &calendar())
        .unwrap();
    assert_eq!(range.1, today, "the range ends today");
    assert_eq!(
        range.0,
        day(2024, 3, 4),
        "seven days inclusive starts on the 4th"
    );
    assert_eq!(
        DateRangeRules::day_count(range, &calendar()),
        7,
        "the count matches the requested span"
    );
}

#[test]
fn last_days_clamps_to_one() {
    let today = day(2024, 3, 10);
    let range = DateRangePreset::last_days(0, None)
        .range(today, &calendar())
        .unwrap();
    assert_eq!(range.0, today);
    assert_eq!(range.1, today);
}

#[test]
fn this_month_spans_the_real_month_length() {
    let february = DateRangePreset::this_month("This month")
        .range(day(2024, 2, 10), &calendar())
        .unwrap();
    assert_eq!(february.0, day(2024, 2, 1));
    assert_eq!(february.1, day(2024, 2, 29), "2024 is a leap year");

    let march = DateRangePreset::this_month("This month")
        .range(day(2023, 3, 31), &calendar())
        .unwrap();
    assert_eq!(march.1, day(2023, 3, 31), "a 31-day month ends on the 31st");
}

#[test]
fn last_month_crosses_the_year_boundary() {
    let range = DateRangePreset::last_month("Last month")
        .range(day(2024, 1, 15), &calendar())
        .unwrap();
    assert_eq!(range.0, day(2023, 12, 1));
    assert_eq!(range.1, day(2023, 12, 31));
}

#[test]
fn a_reversed_preset_result_is_normalized() {
    // A custom preset may return its bounds backwards; `range` orders them.
    let preset = DateRangePreset::new("custom", "Custom", |_, _| {
        Some((day(2024, 5, 20), day(2024, 5, 10)))
    });
    let range = preset.range(day(2024, 5, 15), &calendar()).unwrap();
    assert_eq!(range.0, day(2024, 5, 10));
    assert_eq!(range.1, day(2024, 5, 20));
}

#[test]
fn the_day_count_counts_both_endpoints() {
    let calendar = calendar();
    let one = (day(2024, 5, 10), day(2024, 5, 10));
    assert_eq!(DateRangeRules::day_count(one, &calendar), 1);
    let three = (day(2024, 5, 10), day(2024, 5, 12));
    assert_eq!(DateRangeRules::day_count(three, &calendar), 3);
}

#[test]
fn an_unlimited_rule_allows_any_range() {
    let rules = DateRangeRules::new();
    let wide = (day(2024, 1, 1), day(2024, 12, 31));
    assert!(rules.allows(wide, &calendar()));
}

#[test]
fn a_maximum_rejects_a_longer_range() {
    let calendar = calendar();
    let rules = DateRangeRules::new().with_maximum_days(7);
    assert!(
        rules.allows((day(2024, 5, 1), day(2024, 5, 7)), &calendar),
        "7 days fits"
    );
    assert!(
        !rules.allows((day(2024, 5, 1), day(2024, 5, 8)), &calendar),
        "8 days does not"
    );
}

#[test]
fn a_maximum_below_one_becomes_one() {
    let mut rules = DateRangeRules::new();
    rules.maximum_days = Some(0);
    rules.validate();
    assert_eq!(rules.maximum_days, Some(1));
}

#[test]
fn can_complete_measures_either_direction() {
    let calendar = calendar();
    let rules = DateRangeRules::new().with_maximum_days(7);
    // Dragging upward must validate the same span as dragging downward.
    assert!(rules.can_complete(day(2024, 5, 7), day(2024, 5, 1), &calendar));
    assert!(!rules.can_complete(day(2024, 5, 8), day(2024, 5, 1), &calendar));
}
