//! Behaviour tests for `TimeInputState`.
//!
//! Translation of `Primitives/Controls/Fields/TimeInput.swift`. Segment stepping
//! and digit buffering are where 12-hour clocks go wrong — stepping 11 to 12 must
//! not flip AM/PM, and a leading zero must wait for the second digit.

use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::fields::{
    TimeInputHourCycle, TimeInputState, TimeSegment,
};

const TWELVE: TimeInputHourCycle = TimeInputHourCycle::Twelve;
const TWENTY_FOUR: TimeInputHourCycle = TimeInputHourCycle::TwentyFour;

#[test]
fn a_twelve_hour_clock_reads_midnight_and_noon_as_twelve() {
    let midnight = TimeInputState::new(Some(TimeOfDay::new(0, 5, 0)));
    assert_eq!(midnight.hour_text(TWELVE), "12");
    assert_eq!(midnight.period_text(), "AM");

    let noon = TimeInputState::new(Some(TimeOfDay::new(12, 0, 0)));
    assert_eq!(noon.hour_text(TWELVE), "12");
    assert_eq!(noon.period_text(), "PM");

    let afternoon = TimeInputState::new(Some(TimeOfDay::new(13, 30, 0)));
    assert_eq!(afternoon.hour_text(TWELVE), "1", "no leading zero on a 12-hour clock");
    assert_eq!(afternoon.minute_text(), "30");
}

#[test]
fn a_twenty_four_hour_clock_keeps_two_digits() {
    let state = TimeInputState::new(Some(TimeOfDay::new(7, 5, 0)));
    assert_eq!(state.hour_text(TWENTY_FOUR), "07");
    assert_eq!(state.minute_text(), "05");
}

#[test]
fn an_empty_selection_reads_as_midnight() {
    let state = TimeInputState::new(None);
    assert_eq!(state.time(), TimeOfDay::new(0, 0, 0));
    assert_eq!(state.hour_text(TWENTY_FOUR), "00");
}

#[test]
fn stepping_hour_in_a_twelve_hour_clock_stays_in_the_same_half() {
    // 11 AM stepping up becomes 12 PM, not 12 AM.
    let mut morning = TimeInputState::new(Some(TimeOfDay::new(11, 0, 0)));
    morning.step_hour(1, TWELVE);
    assert_eq!(morning.value, Some(TimeOfDay::new(12, 0, 0)));
    assert_eq!(morning.period_text(), "PM");

    // 12 PM stepping down stays in the PM half: within (0 - 1) rem 12 == 11,
    // so the result is 23:00, which still reads as 11 PM.
    let mut noon = TimeInputState::new(Some(TimeOfDay::new(12, 0, 0)));
    noon.step_hour(-1, TWELVE);
    assert_eq!(noon.value, Some(TimeOfDay::new(23, 0, 0)));
    assert_eq!(noon.hour_text(TWELVE), "11");
    assert_eq!(noon.period_text(), "PM");
}

#[test]
fn stepping_hour_wraps_in_a_twenty_four_hour_clock() {
    let mut state = TimeInputState::new(Some(TimeOfDay::new(23, 0, 0)));
    state.step_hour(1, TWENTY_FOUR);
    assert_eq!(state.value, Some(TimeOfDay::new(0, 0, 0)));
    state.step_hour(-1, TWENTY_FOUR);
    assert_eq!(state.value, Some(TimeOfDay::new(23, 0, 0)));
}

#[test]
fn stepping_minutes_wraps_without_carrying_into_the_hour() {
    // Swift's `set` clamps each field independently: minute 60 becomes 0 and the
    // hour is left alone. Matching that exactly matters — a carry would make
    // stepping past :59 change the hour behind the person's back.
    let mut state = TimeInputState::new(Some(TimeOfDay::new(10, 59, 0)));
    state.step_minute(1);
    assert_eq!(state.value, Some(TimeOfDay::new(10, 0, 0)));
    state.step_minute(-1);
    assert_eq!(state.value, Some(TimeOfDay::new(10, 59, 0)));
}

#[test]
fn toggling_period_moves_twelve_hours() {
    let mut state = TimeInputState::new(Some(TimeOfDay::new(9, 15, 0)));
    state.toggle_period();
    assert_eq!(state.value, Some(TimeOfDay::new(21, 15, 0)));
    assert_eq!(state.period_text(), "PM");
}

#[test]
fn typing_a_high_first_digit_advances_immediately() {
    // In a 24-hour clock, 3*10 >= 24, so focus advances after one digit.
    let mut state = TimeInputState::new(Some(TimeOfDay::new(0, 0, 0)));
    assert!(state.type_hour(3, TWENTY_FOUR), "a high first digit completes the hour");
    assert_eq!(state.value, Some(TimeOfDay::new(3, 0, 0)));
}

#[test]
fn typing_a_low_first_digit_waits_for_the_second() {
    // 1*10 < 24, so the field keeps focus and the second digit forms the value.
    let mut state = TimeInputState::new(Some(TimeOfDay::new(0, 0, 0)));
    assert!(!state.type_hour(1, TWENTY_FOUR), "1 alone is ambiguous");
    assert!(state.type_hour(9, TWENTY_FOUR), "19 completes and advances");
    assert_eq!(state.value, Some(TimeOfDay::new(19, 0, 0)));
}

#[test]
fn a_leading_zero_in_a_twelve_hour_clock_waits() {
    let mut state = TimeInputState::new(Some(TimeOfDay::new(0, 0, 0)));
    assert!(!state.type_hour(0, TWELVE), "0 alone is not a 12-hour value");
    state.type_hour(5, TWELVE);
    assert_eq!(state.value, Some(TimeOfDay::new(5, 0, 0)));
}

#[test]
fn typing_minutes_completes_on_a_high_digit() {
    let mut state = TimeInputState::new(Some(TimeOfDay::new(10, 0, 0)));
    // 7*10 >= 60, so one digit completes the field and advances focus.
    assert!(state.type_minute(7));
    assert_eq!(state.value, Some(TimeOfDay::new(10, 7, 0)));

    // The buffer still holds "7", so a following 4 forms 74 — out of range, so
    // the value is unchanged and the buffer resets. Swift returns `true` here
    // because the segment is done either way.
    assert!(state.type_minute(4));
    assert_eq!(state.value, Some(TimeOfDay::new(10, 7, 0)), "74 is not a minute");

    // A fresh low digit then waits for its second.
    assert!(!state.type_minute(4));
    assert_eq!(state.value, Some(TimeOfDay::new(10, 4, 0)));
    assert!(state.type_minute(5));
    assert_eq!(state.value, Some(TimeOfDay::new(10, 45, 0)));
}

#[test]
fn the_period_letter_only_toggles_when_it_differs() {
    let mut morning = TimeInputState::new(Some(TimeOfDay::new(9, 0, 0)));
    assert!(!morning.type_period(false), "AM already selected");
    assert!(morning.type_period(true));
    assert_eq!(morning.period_text(), "PM");
}

#[test]
fn blurring_resets_only_that_segment_buffer() {
    let mut state = TimeInputState::new(Some(TimeOfDay::new(0, 0, 0)));
    state.type_hour(1, TWENTY_FOUR);
    state.blur(TimeSegment::Hour);
    assert!(state.hour_buffer.is_empty());
    // A fresh first digit then behaves as a first digit again.
    assert!(!state.type_hour(1, TWENTY_FOUR));
}

#[test]
fn segments_cycle_in_visual_order() {
    assert_eq!(TimeSegment::Hour.next(TWELVE), TimeSegment::Minute);
    assert_eq!(TimeSegment::Minute.next(TWELVE), TimeSegment::Period);
    assert_eq!(TimeSegment::Period.next(TWELVE), TimeSegment::Hour);
    assert_eq!(TimeSegment::Minute.next(TWENTY_FOUR), TimeSegment::Hour);
    assert_eq!(TimeSegment::Hour.previous(TWELVE), TimeSegment::Period);
}
