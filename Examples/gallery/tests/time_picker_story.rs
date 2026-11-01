//! Behaviour tests for the time-picker story formatter.
//!
//! Translation of `GalleryContent/Stories/Controls/TimePickerStory.swift`. A
//! 12-hour clock has two classic traps: midnight and noon must both read `12`,
//! and the marker must flip at 12 rather than at 13.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::time_picker_story::{
    DEFAULT_HOUR, DEFAULT_MINUTE, TimePickerStory, display_text,
};
use cove_ui_compose::primitives::controls::date_values::TimeOfDay;

#[test]
fn the_story_opens_at_half_past_nine_am() {
    let story = TimePickerStory::new(GalleryOptions::default());
    assert_eq!(
        story.time(),
        Some(TimeOfDay::new(DEFAULT_HOUR, DEFAULT_MINUTE, 0))
    );
    assert_eq!(story.display(), "09:30 AM");
}

#[test]
fn midnight_reads_as_twelve_am() {
    assert_eq!(
        display_text(Some(TimeOfDay::new(0, 5, 0)), true, false),
        "12:05 AM"
    );
}

#[test]
fn noon_reads_as_twelve_pm() {
    assert_eq!(
        display_text(Some(TimeOfDay::new(12, 0, 0)), true, false),
        "12:00 PM"
    );
}

#[test]
fn one_pm_reads_as_one_pm() {
    assert_eq!(
        display_text(Some(TimeOfDay::new(13, 45, 0)), true, false),
        "01:45 PM"
    );
}

#[test]
fn eleven_fifty_nine_stays_am() {
    assert_eq!(
        display_text(Some(TimeOfDay::new(11, 59, 0)), true, false),
        "11:59 AM"
    );
}

#[test]
fn a_twenty_four_hour_clock_uses_zero_padding_and_no_marker() {
    assert_eq!(
        display_text(Some(TimeOfDay::new(9, 5, 0)), false, false),
        "09:05"
    );
    assert_eq!(
        display_text(Some(TimeOfDay::new(0, 0, 0)), false, false),
        "00:00"
    );
    assert_eq!(
        display_text(Some(TimeOfDay::new(23, 59, 0)), false, false),
        "23:59"
    );
}

#[test]
fn seconds_are_appended_when_enabled() {
    assert_eq!(
        display_text(Some(TimeOfDay::new(9, 30, 7)), true, true),
        "09:30:07 AM"
    );
    assert_eq!(
        display_text(Some(TimeOfDay::new(9, 30, 7)), false, true),
        "09:30:07"
    );
}

#[test]
fn a_cleared_time_reads_as_no_time() {
    assert_eq!(display_text(None, true, false), "No time");
    assert_eq!(display_text(None, false, true), "No time");
}

#[test]
fn clearing_and_restoring_the_selection() {
    let story = TimePickerStory::new(GalleryOptions::default());
    story.clear();
    assert_eq!(story.time(), None);
    assert_eq!(story.display(), "No time");
    story.set_time(Some(TimeOfDay::new(12, 0, 0)));
    assert_eq!(story.display(), "12:00 PM");
}

#[test]
fn toggling_the_clock_style_changes_the_display() {
    let story = TimePickerStory::new(GalleryOptions::default());
    assert_eq!(story.display(), "09:30 AM");
    story.set_twelve_hour(false);
    assert_eq!(story.display(), "09:30");
    story.set_shows_seconds(true);
    assert_eq!(story.display(), "09:30:00");
}
