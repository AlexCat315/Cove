//! Translation of
//! `Hosts/Android/Contract/Tests/CoveAndroidContractTests/FrameClockTests.swift`.
//!
//! Behaviour checklist:
//! - the first frame uses the display-refresh default (1/60 s)
//! - idle callbacks do not advance the animation clock by more than one period
//! - dropped frames are bounded to 0.1 s and backwards timestamps are ignored
//! - a new surface starts a fresh clock

use cove_hosts::android::contract::DisplayFrameClock;

/// Timestamps are nanoseconds; the comparison tolerance mirrors the Swift
/// `abs(...) < 0.00000001` helper.
fn expect_advance(clock: &mut DisplayFrameClock, timestamp: u64, expected: f64) {
    let actual = clock.advance(timestamp);
    assert!(
        (actual - expected).abs() < 0.00000001,
        "advance(to: {timestamp}) was {actual}, expected {expected}"
    );
}

#[test]
fn first_frame_uses_display_refresh_default() {
    let mut clock = DisplayFrameClock::new();
    expect_advance(&mut clock, 1_000_000_000, 1.0 / 60.0);
}

#[test]
fn idle_callbacks_do_not_advance_animation_clock() {
    let mut clock = DisplayFrameClock::new();
    // The first callback establishes the baseline at one display period.
    expect_advance(&mut clock, 1_000_000_000, 1.0 / 60.0);
    expect_advance(&mut clock, 1_008_333_333, 0.008333333);
    expect_advance(&mut clock, 1_025_000_000, 0.016666667);

    // A long run of 60 Hz callbacks while nothing is animating.
    let mut timestamp = 1_041_666_667u64;
    while timestamp <= 3_000_000_000 {
        let _ = clock.advance(timestamp);
        timestamp += 16_666_667;
    }

    expect_advance(&mut clock, 3_000_000_040, 0.008333334);
}

#[test]
fn dropped_frames_are_bounded_and_backwards_timestamps_ignored() {
    let mut clock = DisplayFrameClock::new();
    // The first call only seeds the baseline.
    let _ = clock.advance(3_000_000_000);

    assert_eq!(
        clock.advance(3_250_000_040),
        0.1,
        "a 0.25 s gap clamps to the 0.1 s ceiling"
    );
    expect_advance(&mut clock, 3_250_000_040, 0.0);
    expect_advance(&mut clock, 3_200_000_000, 0.0);
    expect_advance(&mut clock, 3_266_666_707, 0.016666667);
}

#[test]
fn new_surface_starts_fresh_clock() {
    let mut clock = DisplayFrameClock::new();
    expect_advance(&mut clock, 1_000_000_000, 1.0 / 60.0);

    clock = DisplayFrameClock::new();
    expect_advance(&mut clock, 10_000_000_000, 1.0 / 60.0);
}
