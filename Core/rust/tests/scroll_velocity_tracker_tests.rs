//! Translation of Tests/Headless/Tests/CoveUISceneTests/Scrolling/ScrollVelocityTrackerTests.swift.
use cove_core::scene::ScrollVelocityTracker;

#[test]
fn a_single_sample_carries_no_direction() {
    let mut tracker = ScrollVelocityTracker::default();
    tracker.add(0.0, 40.0);
    assert_eq!(tracker.velocity(Some(0.5), None), 0.0);
    assert_eq!(tracker.sample_count(), 1);
}

#[test]
fn steady_movement_reports_its_own_speed() {
    let mut tracker = ScrollVelocityTracker::new(0.1);
    for index in 0..6 {
        tracker.add(f64::from(index) * 0.016, index as f32 * 8.0);
    }
    let measured = tracker.velocity(None, None);
    assert!((measured - 500.0).abs() < 1.0);
}

#[test]
fn movement_in_the_negative_direction_reports_a_negative_speed() {
    let mut tracker = ScrollVelocityTracker::new(0.1);
    for index in 0..6 {
        tracker.add(f64::from(index) * 0.016, -(index as f32) * 8.0);
    }
    assert!(tracker.velocity(None, None) < -400.0);
}

#[test]
fn samples_older_than_the_window_stop_counting() {
    let mut tracker = ScrollVelocityTracker::new(0.1);
    tracker.add(0.0, 0.0);
    tracker.add(0.2, 100.0);
    assert_eq!(tracker.velocity(Some(0.2), None), 0.0);

    tracker.reset();
    tracker.add(0.0, 0.0);
    tracker.add(0.05, 50.0);
    assert!((tracker.velocity(Some(0.05), None) - 1_000.0).abs() < 1.0);
}

#[test]
fn uneven_sample_spacing_still_recovers_the_underlying_speed() {
    let mut tracker = ScrollVelocityTracker::new(0.5);
    let samples = [
        (0.000, 0.0_f32),
        (0.017, 12.0),
        (0.041, 25.0),
        (0.058, 40.0),
        (0.089, 58.0),
    ];
    for (time, value) in samples {
        tracker.add(time, value);
    }
    let measured = tracker.velocity(None, None);
    let rough_rate = 58.0_f32 / 0.089_f32;
    assert!((measured - rough_rate).abs() < rough_rate * 0.1);
}

#[test]
fn coming_to_rest_measures_zero_not_whatever_the_last_gap_implies() {
    let mut tracker = ScrollVelocityTracker::new(0.1);
    for index in 0..5 {
        tracker.add(f64::from(index) * 0.02, index as f32 * 10.0);
    }
    assert!(tracker.velocity(None, None) > 400.0);
    for index in 5..12 {
        tracker.add(f64::from(index) * 0.02, 40.0);
    }
    assert!(tracker.velocity(None, None).abs() < 1.0);
}

#[test]
fn a_clock_that_runs_backwards_restarts_instead_of_reporting_nonsense() {
    let mut tracker = ScrollVelocityTracker::default();
    tracker.add(5.0, 100.0);
    tracker.add(1.0, 0.0);
    assert_eq!(tracker.sample_count(), 1);
    tracker.add(1.05, 50.0);
    assert!((tracker.velocity(None, None) - 1_000.0).abs() < 1.0);

    let mut idle = ScrollVelocityTracker::default();
    idle.add(0.0, 0.0);
    idle.add(0.05, 40.0);
    assert!((idle.velocity(None, None) - 800.0).abs() < 1.0);
    assert_eq!(idle.velocity(Some(2.0), Some(0.25)), 0.0);
}

#[test]
fn non_finite_samples_are_ignored_rather_than_poisoning_the_fit() {
    let mut tracker = ScrollVelocityTracker::default();
    tracker.add(f64::NAN, 10.0);
    tracker.add(0.0, f32::INFINITY);
    assert_eq!(tracker.sample_count(), 0);
    tracker.add(0.0, 0.0);
    tracker.add(0.1, 10.0);
    assert!((tracker.velocity(None, None) - 100.0).abs() < 1.0);
}
