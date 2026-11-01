//! Translation of `Tests/CoveUIComposeTests/TimeInputTests.swift`.
//!
//! Behaviour checklist:
//! - Up/Down step the focused hour, wrapping around the clock
//! - digits set the hour and advance to the minutes once two are entered
//! - Left/Right move focus between segments
//! - a 12-hour clock keeps AM/PM stable while stepping within a half day
//! - the period letter and Up/Down toggle AM/PM
//!
//! Model difference: the Swift suite focuses real segment nodes and dispatches
//! key events through the scene. The Rust `TimeInputState` owns the same
//! stepping/typing maths, so these tests drive it directly with the same inputs
//! and assert the same resulting `TimeOfDay`.

use cove_ui_compose::primitives::controls::date_values::TimeOfDay;
use cove_ui_compose::primitives::controls::fields::{
    TimeInputHourCycle, TimeInputState, TimeSegment,
};

fn state(hour: i32, minute: i32) -> TimeInputState {
    TimeInputState::new(Some(TimeOfDay::new(hour, minute, 0)))
}

#[test]
fn up_and_down_step_the_hour_and_wrap_around_the_clock() {
    let mut value = state(9, 30);
    assert_eq!(value.time().hour, 9);

    value.step_hour(1, TimeInputHourCycle::TwentyFour);
    assert_eq!(value.time().hour, 10, "Up steps forward");

    value.step_hour(-1, TimeInputHourCycle::TwentyFour);
    value.step_hour(-1, TimeInputHourCycle::TwentyFour);
    assert_eq!(value.time().hour, 8, "Down steps backward");

    // Wrapping: 23 -> 00 -> 23.
    let mut wrapped = state(23, 0);
    wrapped.step_hour(1, TimeInputHourCycle::TwentyFour);
    assert_eq!(wrapped.time().hour, 0, "stepping past midnight wraps");
    wrapped.step_hour(-1, TimeInputHourCycle::TwentyFour);
    assert_eq!(wrapped.time().hour, 23, "stepping back wraps below zero");
}

#[test]
fn the_minute_is_preserved_while_stepping_the_hour() {
    let mut value = state(9, 30);
    value.step_hour(1, TimeInputHourCycle::TwentyFour);
    assert_eq!(value.time().minute, 30, "only the hour moves");
}

#[test]
fn stepping_the_minute_wraps_within_the_minute_field_only() {
    // `set` wraps each field independently, so stepping the minute never
    // carries into the hour — the hour segment owns that.
    let mut value = state(9, 59);
    value.step_minute(1);
    assert_eq!(
        (value.time().hour, value.time().minute),
        (9, 0),
        "59 + 1 wraps to 0 without changing the hour"
    );

    let mut back = state(9, 0);
    back.step_minute(-1);
    assert_eq!(
        (back.time().hour, back.time().minute),
        (9, 59),
        "0 - 1 wraps to 59 without borrowing from the hour"
    );
}

#[test]
fn digits_set_the_hour_and_advance_once_two_are_entered() {
    let mut value = state(9, 30);
    // "1" can only be a first digit, so focus stays for the second.
    assert!(
        !value.type_hour(1, TimeInputHourCycle::TwentyFour),
        "a leading 1 keeps focus for the second digit"
    );
    assert_eq!(value.time().hour, 1, "the first digit already writes");

    assert!(
        value.type_hour(4, TimeInputHourCycle::TwentyFour),
        "the second digit completes the hour and advances"
    );
    assert_eq!(value.time().hour, 14, "1 then 4 reads as 14");
    assert_eq!(value.time().minute, 30, "the minute is untouched");
}

#[test]
fn a_hour_digit_that_cannot_start_a_value_advances_immediately() {
    // A first digit advances only while it could still start a two-digit hour
    // (digit * 10 < 24), so 0, 1 and 2 wait for a second digit.
    for waiting in [0, 1, 2] {
        let mut value = state(9, 30);
        assert!(
            !value.type_hour(waiting, TimeInputHourCycle::TwentyFour),
            "{waiting} can still become 0x/1x/2x, so focus stays"
        );
    }
    // 3 and above cannot start a valid hour (30 > 23), so the hour completes.
    for complete in [3, 9] {
        let mut value = state(9, 30);
        assert!(
            value.type_hour(complete, TimeInputHourCycle::TwentyFour),
            "{complete} completes the hour immediately"
        );
        assert_eq!(value.time().hour, complete);
    }
}

#[test]
fn digits_set_the_minutes_and_complete_after_two() {
    let mut value = state(9, 30);
    assert!(
        !value.type_minute(4),
        "a leading 4 keeps focus for the second digit"
    );
    assert_eq!(value.time().minute, 4);

    assert!(
        value.type_minute(5),
        "the second digit completes the minute"
    );
    assert_eq!(value.time().minute, 45, "4 then 5 reads as 45");
}

#[test]
fn a_minute_digit_that_cannot_start_a_value_completes_immediately() {
    let mut value = state(9, 0);
    assert!(
        value.type_minute(7),
        "a digit above 5 completes the minute at once"
    );
    assert_eq!(value.time().minute, 7);
}

#[test]
fn a_12_hour_step_does_not_flip_the_half_of_the_day() {
    let mut value = state(11, 0);
    value.step_hour(1, TimeInputHourCycle::Twelve);
    assert_eq!(value.time().hour, 12, "11 AM steps to 12 PM, not midnight");

    value.step_hour(1, TimeInputHourCycle::Twelve);
    assert_eq!(value.time().hour, 13, "12 PM steps to 1 PM");
}

#[test]
fn the_period_toggles_by_moving_twelve_hours() {
    let mut value = state(9, 30);
    value.toggle_period();
    assert_eq!(value.time().hour, 21, "AM becomes PM");
    assert_eq!(value.period_text(), "PM");

    value.toggle_period();
    assert_eq!(value.time().hour, 9, "PM becomes AM again");
    assert_eq!(value.period_text(), "AM");
}

#[test]
fn typing_the_period_letter_chooses_the_half_of_the_day() {
    let mut evening = state(21, 30);
    assert!(
        evening.type_period(false),
        "typing A moves an evening time into the morning"
    );
    assert_eq!(
        evening.time().hour,
        9,
        "the half flips, the hour within it is kept"
    );

    // Typing the letter matching the current half reports no change.
    assert!(
        !evening.type_period(false),
        "repeating the current half changes nothing"
    );
}

#[test]
fn the_hour_and_minute_text_follow_the_cycle() {
    let value = state(9, 5);
    assert_eq!(value.hour_text(TimeInputHourCycle::TwentyFour), "09");
    assert_eq!(
        value.hour_text(TimeInputHourCycle::Twelve),
        "9",
        "a 12-hour clock omits the leading zero"
    );
    assert_eq!(value.minute_text(), "05");

    let noon = state(12, 0);
    assert_eq!(
        noon.hour_text(TimeInputHourCycle::Twelve),
        "12",
        "noon reads as 12"
    );
    let midnight = state(0, 0);
    assert_eq!(
        midnight.hour_text(TimeInputHourCycle::Twelve),
        "12",
        "midnight also reads as 12"
    );
}

#[test]
fn segments_cycle_in_visual_order_for_each_hour_cycle() {
    assert_eq!(
        TimeSegment::Hour.next(TimeInputHourCycle::TwentyFour),
        TimeSegment::Minute
    );
    assert_eq!(
        TimeSegment::Minute.next(TimeInputHourCycle::TwentyFour),
        TimeSegment::Hour,
        "a 24-hour clock has no period segment"
    );
    assert_eq!(
        TimeSegment::Minute.next(TimeInputHourCycle::Twelve),
        TimeSegment::Period
    );
    assert_eq!(
        TimeSegment::Period.next(TimeInputHourCycle::Twelve),
        TimeSegment::Hour
    );
    assert_eq!(
        TimeSegment::Hour.previous(TimeInputHourCycle::Twelve),
        TimeSegment::Period,
        "stepping left from the hour lands on the period"
    );
}

#[test]
fn blurring_a_segment_resets_its_digit_buffer() {
    let mut value = state(9, 30);
    value.type_hour(1, TimeInputHourCycle::TwentyFour);
    value.blur(TimeSegment::Hour);
    // After blur, a fresh digit is treated as the first of a new pair.
    assert!(
        !value.type_hour(2, TimeInputHourCycle::TwentyFour),
        "the buffer restarted, so 2 waits for a second digit"
    );
    assert_eq!(value.time().hour, 2);
}

#[test]
fn an_empty_selection_displays_midnight() {
    let value = TimeInputState::new(None);
    assert_eq!((value.time().hour, value.time().minute), (0, 0));
    assert_eq!(value.period_text(), "AM");
}
