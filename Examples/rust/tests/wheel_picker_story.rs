//! Behaviour tests for the wheel-picker story.
//!
//! Translation of `GalleryContent/Stories/Controls/WheelPickerStory.swift`. The
//! option tables and the snapping rules are what the gallery documents.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::wheel_picker_story::{
    hour_options, minute_options, period_options, time_label, volume_options, WheelPickerStory,
};

fn story() -> WheelPickerStory {
    WheelPickerStory::new(GalleryOptions::default())
}

#[test]
fn the_story_starts_at_half_past_nine_am() {
    let story = story();
    assert_eq!(story.time_label(), "09:30 AM");
    assert_eq!(story.volume(), 6);
}

#[test]
fn reset_restores_every_wheel() {
    let story = story();
    story.set_time_24h(13, 47);
    story.reset();
    assert_eq!(story.hour(), 9);
    assert_eq!(story.minute(), 30);
    assert_eq!(story.period(), 0);
    assert_eq!(story.volume(), 6);
}

#[test]
fn the_hour_wheel_lists_twelve_rows_and_reads_zero_as_twelve() {
    let options = hour_options();
    assert_eq!(options.len(), 12);
    assert_eq!(options[0].label, "12", "0 reads as 12");
    assert_eq!(options[9].label, "09");
}

#[test]
fn the_minute_wheel_steps_in_fives() {
    let options = minute_options();
    assert_eq!(options.len(), 12);
    assert_eq!(options[0].label, "00");
    assert_eq!(options[1].label, "05");
    assert_eq!(options[11].label, "55");
}

#[test]
fn the_volume_wheel_disables_its_top_row_only() {
    let options = volume_options();
    assert_eq!(options.len(), 11);
    assert!(options[10].is_enabled == false, "level 10 is unavailable");
    assert!(options[9].is_enabled);
}

#[test]
fn setting_a_pm_time_flips_the_half_day() {
    let story = story();
    story.set_time_24h(13, 5);
    assert_eq!(story.period(), 1);
    assert_eq!(story.hour(), 1, "13:xx folds to 1 PM");
    assert_eq!(story.time_label(), "01:05 PM");
}

#[test]
fn setting_midnight_reads_as_twelve_am() {
    let story = story();
    story.set_time_24h(0, 0);
    assert_eq!(story.hour(), 12);
    assert_eq!(story.period(), 0);
    assert_eq!(story.time_label(), "12:00 AM");
}

#[test]
fn minutes_snap_down_to_the_wheel_step() {
    let story = story();
    story.set_time_24h(9, 47);
    assert_eq!(story.minute(), 45, "47 snaps down to the nearest five");
}

#[test]
fn the_time_label_pads_both_fields() {
    assert_eq!(time_label(9, 5, 0), "09:05 AM");
    assert_eq!(time_label(12, 0, 1), "12:00 PM");
    assert_eq!(time_label(0, 30, 0), "12:30 AM");
}

#[test]
fn the_period_wheel_has_exactly_two_rows() {
    let options = period_options();
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].label, "AM");
    assert_eq!(options[1].label, "PM");
}

#[test]
fn the_composed_wheels_commit_the_story_values() {
    let story = story();
    assert_eq!(story.hour_wheel().value(), Some(&9));
    assert_eq!(story.minute_wheel().value(), Some(&30));
    assert_eq!(story.period_wheel().value(), Some(&0));
    assert_eq!(story.volume_wheel().value(), Some(&6));
}
