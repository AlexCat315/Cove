//! Translation of Tests/Headless/Tests/CoveUISceneTests/Scrolling/ScrollSpringTests.swift.
use cove_core::scene::{ScrollSettleSettings, ScrollSpring};

#[test]
fn a_critically_damped_spring_arrives_without_crossing_its_target() {
    let mut spring = ScrollSpring::new(
        80.0,
        0.0,
        0.0,
        ScrollSettleSettings { damping_fraction: 1.0, ..Default::default() },
    );
    for _ in 0..240 {
        spring.advance(1.0 / 60.0);
        assert!(spring.value >= -0.001);
    }
    assert_eq!(spring.value, 0.0);
    assert!(spring.is_at_rest());
}

#[test]
fn an_underdamped_spring_crosses_the_bound_before_settling() {
    let mut spring = ScrollSpring::new(
        80.0,
        0.0,
        0.0,
        ScrollSettleSettings { damping_fraction: 0.45, ..Default::default() },
    );
    let mut crossed = false;
    for _ in 0..600 {
        spring.advance(1.0 / 60.0);
        if spring.value < -0.5 {
            crossed = true;
        }
    }
    assert!(crossed);
    assert_eq!(spring.value, 0.0);
}

#[test]
fn both_directions_converge_on_the_same_rest_state() {
    let mut spring = ScrollSpring::new(-30.0, 0.0, -100.0, ScrollSettleSettings::default());
    for _ in 0..240 {
        spring.advance(1.0 / 60.0);
    }
    assert_eq!(spring.value, -100.0);
}

#[test]
fn a_large_frame_follows_the_same_trajectory_as_many_small_frames() {
    let mut coarse = ScrollSpring::new(100.0, 0.0, 0.0, ScrollSettleSettings::default());
    let mut fine = ScrollSpring::new(100.0, 0.0, 0.0, ScrollSettleSettings::default());
    coarse.advance(0.25);
    for _ in 0..15 {
        fine.advance(0.25 / 15.0);
    }
    assert!((coarse.value - fine.value).abs() < 0.0001);
    assert!(coarse.value < 100.0);
    assert!(coarse.value >= 0.0);
}

#[test]
fn starting_velocity_is_carried_into_the_first_frames() {
    let mut away =
        ScrollSpring::new(0.0, 400.0, 100.0, ScrollSettleSettings::default());
    away.advance(1.0 / 60.0);
    assert!(away.value > 4.0);

    let mut still = ScrollSpring::new(0.0, 0.0, 100.0, ScrollSettleSettings::default());
    still.advance(1.0 / 60.0);
    assert!(still.value < away.value);
}

#[test]
fn rest_detection_snaps_to_the_target_and_clears_velocity() {
    let mut spring = ScrollSpring::new(0.02, 0.2, 0.0, ScrollSettleSettings::default());
    assert!(spring.is_at_rest());
    spring.advance(1.0 / 60.0);
    assert_eq!(spring.value, 0.0);
    assert_eq!(spring.velocity, 0.0);
}

#[test]
fn zero_negative_and_nonfinite_frame_durations_leave_the_spring_unchanged() {
    for time in [0.0_f32, -1.0, f32::NAN, f32::INFINITY] {
        let mut spring = ScrollSpring::new(80.0, 120.0, 0.0, ScrollSettleSettings::default());
        let before = spring;
        spring.advance(time);
        assert_eq!(spring, before);
    }
}

#[test]
fn all_damping_regimes_follow_the_same_path_across_uneven_frames() {
    for ratio in [0.5_f32, 1.0, 1.8] {
        let settings =
            ScrollSettleSettings { damping_fraction: ratio, ..Default::default() };
        let mut coarse = ScrollSpring::new(80.0, 420.0, 0.0, settings);
        let mut uneven = coarse;
        coarse.advance(0.3);
        for delta in [0.005_f32, 0.045, 0.1, 0.002, 0.148] {
            uneven.advance(delta);
        }
        assert!((coarse.value - uneven.value).abs() < 0.0001);
        assert!((coarse.velocity - uneven.velocity).abs() < 0.001);
    }
}
