//! Behaviour tests for the wheel picker.
//!
//! Translation of `Primitives/Controls/WheelPicker.swift`. A wheel that lands on
//! a disabled row is a real bug — the row is visible, so the selection looks
//! valid while being unconmittable. These pin the skip and clamp rules.

use cove_ui_compose::primitives::controls::fields::{WheelPicker, WheelPickerOption};

fn volumes() -> WheelPicker<i32> {
    // 0…10 with the top row disabled, matching the story's volume wheel.
    let options = (0..=10)
        .map(|value| WheelPickerOption::new(value, format!("{value}")).enabled(value != 10))
        .collect();
    WheelPicker::new(options, Some(6))
}

#[test]
fn the_wheel_starts_on_the_requested_value() {
    let wheel = volumes();
    assert_eq!(wheel.value(), Some(&6));
    assert_eq!(wheel.selected_label(), Some("6"));
}

#[test]
fn a_disabled_starting_value_falls_back_to_the_first_enabled_row() {
    let wheel = volumes();
    let options = wheel.options.clone();
    let restart = WheelPicker::new(options, Some(10));
    assert_eq!(
        restart.value(),
        Some(&0),
        "10 is disabled, so land on the first usable row"
    );
}

#[test]
fn an_unknown_starting_value_falls_back_to_the_first_enabled_row() {
    let options = vec![
        WheelPickerOption::new(1, "one"),
        WheelPickerOption::new(2, "two"),
    ];
    let wheel = WheelPicker::new(options, Some(99));
    assert_eq!(wheel.value(), Some(&1));
}

#[test]
fn stepping_moves_one_enabled_row() {
    let mut wheel = volumes();
    assert!(wheel.step(1));
    assert_eq!(wheel.value(), Some(&7));
    assert!(wheel.step(-1));
    assert_eq!(wheel.value(), Some(&6));
}

#[test]
fn stepping_onto_a_disabled_row_skips_it() {
    let mut wheel = volumes();
    // From 9, stepping up would land on the disabled 10, so nothing moves.
    wheel.step(1);
    wheel.step(1);
    wheel.step(1);
    assert_eq!(wheel.value(), Some(&9));
    assert!(!wheel.step(1), "there is nothing enabled above 9");
    assert_eq!(
        wheel.value(),
        Some(&9),
        "the selection does not land on a disabled row"
    );
}

#[test]
fn stepping_stops_at_the_bottom() {
    let mut wheel = volumes();
    for _ in 0..20 {
        wheel.step(-1);
    }
    assert_eq!(wheel.value(), Some(&0));
}

#[test]
fn home_and_end_jump_to_the_edges() {
    let mut wheel = volumes();
    assert!(wheel.select_last());
    assert_eq!(
        wheel.value(),
        Some(&9),
        "the last *enabled* row, not the last row"
    );
    assert!(wheel.select_first());
    assert_eq!(wheel.value(), Some(&0));
    // Re-selecting the current value reports no change.
    assert!(!wheel.select_first());
}

#[test]
fn disabled_rows_are_not_selectable() {
    let wheel = volumes();
    assert!(!wheel.is_selectable(10));
    assert!(wheel.is_selectable(9));
    assert!(!wheel.is_selectable(99), "out of range is not selectable");
}

#[test]
fn a_wheel_with_no_enabled_rows_has_no_selection() {
    let options: Vec<WheelPickerOption<i32>> =
        vec![WheelPickerOption::new(0, "off").enabled(false)];
    let mut wheel = WheelPicker::new(options, None);
    assert_eq!(wheel.value(), None);
    assert!(!wheel.step(1));
    assert!(!wheel.select_first());
    assert!(!wheel.select_last());
    assert_eq!(wheel.selected_label(), None);
}

#[test]
fn the_visible_window_is_centred_and_slides_at_the_ends() {
    let wheel = volumes();
    // Five rows around 6: 4..9.
    assert_eq!(wheel.visible_window(5), 4..9);

    let mut at_top = volumes();
    at_top.select_first();
    assert_eq!(
        at_top.visible_window(5),
        0..5,
        "the window does not run before zero"
    );

    let mut at_bottom = volumes();
    at_bottom.select_last();
    // 11 options, five rows: the window slides back to 6..11.
    assert_eq!(at_bottom.visible_window(5), 6..11);
}

#[test]
fn a_zero_sized_window_is_empty() {
    let wheel = volumes();
    assert_eq!(wheel.visible_window(0), 0..0);
}

#[test]
fn options_are_identified_by_value_not_label() {
    // Two rows may share a label; identity follows the value.
    let options = vec![
        WheelPickerOption::new(0, "same"),
        WheelPickerOption::new(1, "same"),
    ];
    let wheel = WheelPicker::new(options, Some(1));
    assert_eq!(wheel.index_of(&1), Some(1));
    assert_eq!(wheel.selected_label(), Some("same"));
}
