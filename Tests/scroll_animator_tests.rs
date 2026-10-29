//! Translation of Tests/Headless/Tests/CoveUISceneTests/Scrolling/ScrollAnimatorTests.swift.
use cove_core::scene::{
    AnimatorScheduler, AnyAnimationController, Point, ScrollAnimator, ScrollPhysics, Size,
};
use std::cell::Cell;
use std::rc::Rc;

fn with_isolated_scheduler<R>(body: impl FnOnce() -> R) -> R {
    AnimatorScheduler::default().with_current(body)
}

fn tick(frames: usize, duration: f64) {
    for _ in 0..frames {
        AnimatorScheduler::current().tick(duration);
    }
}

#[derive(Clone)]
struct ManualClock {
    now: Rc<Cell<f64>>,
}

impl ManualClock {
    fn new() -> Self {
        Self { now: Rc::new(Cell::new(0.0)) }
    }
    fn run(&self, frames: usize, interval: f64) {
        for _ in 0..frames {
            self.now.set(self.now.get() + interval);
            AnimatorScheduler::current().tick(interval);
        }
    }
}

fn make_animator(
    physics: ScrollPhysics,
    maximum: f32,
    viewport_height: f32,
) -> Rc<ScrollAnimator> {
    ScrollAnimator::new(
        physics,
        Point::new(0.0, maximum),
        Size { width: 200.0, height: viewport_height },
        Point::default(),
        None,
    )
}

fn make_coasting_animator(
    clock: &ManualClock,
    physics: ScrollPhysics,
    maximum: f32,
) -> Rc<ScrollAnimator> {
    let animator = make_animator(physics, maximum, 100.0);
    let now = clock.now.clone();
    animator.set_clock(Rc::new(move || now.get()));
    animator
}

fn wheel(animator: &ScrollAnimator, clock: &ManualClock, samples: usize, step: f32) {
    for _ in 0..samples {
        clock.now.set(clock.now.get() + 1.0 / 60.0);
        animator.scroll_by(Point::new(0.0, step));
    }
}

fn is_finished(animator: &ScrollAnimator) -> bool {
    AnyAnimationController::is_finished(animator)
}

#[test]
fn movement_inside_the_range_is_passed_through_untouched() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        assert_eq!(animator.scroll_by(Point::new(0.0, 40.0)).y, 40.0);
        assert_eq!(animator.scroll_by(Point::new(0.0, 40.0)).y, 80.0);
        assert!(!animator.is_settling());
    });
}

#[test]
fn pushing_past_the_end_moves_further_but_less_than_the_input() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 100.0));
        let visible = animator.scroll_by(Point::new(0.0, 30.0)).y;
        assert!(visible > 100.0);
        assert!(visible < 130.0);
        assert!(visible < 200.0);
    });
}

#[test]
fn the_leading_bound_stretches_downwards_too() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        let visible = animator.scroll_by(Point::new(0.0, -30.0)).y;
        assert!(visible < 0.0);
        assert!(visible > -30.0);
    });
}

#[test]
fn input_that_stops_leaves_the_offset_springing_back_to_the_bound() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        assert!(animator.offset().y > 100.0);
        assert!(animator.is_settling());
        tick(240, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 100.0);
        assert!(!animator.is_settling());
        assert!(is_finished(&animator));
    });
}

#[test]
fn a_range_that_shrinks_snaps_in_place_instead_of_animating() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        animator.update(Point::new(0.0, 20.0), Size { width: 200.0, height: 100.0 });
        assert_eq!(animator.offset().y, 20.0);
        assert!(!animator.is_settling());
    });
}

#[test]
fn growth_does_not_interrupt_an_in_flight_stretch() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        let stretched = animator.offset().y;
        animator.update(Point::new(0.0, 160.0), Size { width: 200.0, height: 100.0 });
        assert_eq!(animator.offset().y, stretched);
        assert!(animator.is_settling());
    });
}

#[test]
fn new_input_during_the_return_spring_moves_continuously() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        tick(3, 1.0 / 60.0);
        let recovered = animator.offset().y;
        assert!(recovered < 140.0);
        let after = animator.scroll_by(Point::new(0.0, 1.0)).y;
        assert!(after > recovered);
        assert!(after - recovered < 1.0);
    });
}

#[test]
fn an_authoritative_write_wins_over_the_return_spring() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        animator.adopt(Point::new(0.0, 10.0));
        assert_eq!(animator.offset().y, 10.0);
        assert!(!animator.is_settling());
        tick(240, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 10.0);
    });
}

#[test]
fn each_axis_is_independent() {
    with_isolated_scheduler(|| {
        let animator = ScrollAnimator::new(
            ScrollPhysics::DEFAULT,
            Point::new(50.0, 50.0),
            Size { width: 60.0, height: 60.0 },
            Point::default(),
            None,
        );
        animator.scroll_by(Point::new(80.0, 10.0));
        assert!(animator.offset().x > 50.0);
        assert_eq!(animator.offset().y, 10.0);
        tick(240, 1.0 / 60.0);
        assert_eq!(animator.offset().x, 50.0);
        assert_eq!(animator.offset().y, 10.0);
    });
}

#[test]
fn clamped_physics_restores_hard_boundaries() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::clamped(), 100.0, 100.0);
        assert_eq!(animator.scroll_by(Point::new(0.0, 140.0)).y, 100.0);
        assert!(!animator.is_settling());
        assert_eq!(animator.scroll_by(Point::new(0.0, -140.0)).y, 0.0);
        tick(240, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 0.0);
    });
}

#[test]
fn bouncy_physics_overshoots_the_bound_on_the_way_back() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::bouncy(), 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        let mut dipped_below_bound = false;
        for _ in 0..240 {
            AnimatorScheduler::current().tick(1.0 / 60.0);
            if animator.offset().y < 99.5 {
                dipped_below_bound = true;
            }
        }
        assert!(dipped_below_bound);
        assert_eq!(animator.offset().y, 100.0);
    });
}

#[test]
fn a_drag_past_the_end_holds_until_the_gesture_ends() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.move_to(Point::new(0.0, 160.0));
        assert!(animator.offset().y > 100.0);
        assert!(!animator.is_settling());
        animator.move_to(Point::new(0.0, 200.0));
        assert!(animator.offset().y > 100.0);
        animator.settle();
        assert!(animator.is_settling());
        tick(240, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 100.0);
    });
}

#[test]
fn finishing_immediately_lands_on_the_bound() {
    with_isolated_scheduler(|| {
        let animator = make_animator(ScrollPhysics::DEFAULT, 100.0, 100.0);
        animator.scroll_by(Point::new(0.0, 140.0));
        animator.finish_immediately();
        assert_eq!(animator.offset().y, 100.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn a_burst_of_wheel_input_keeps_coasting_once_it_stops() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 10, 8.0);
        let delivered = animator.offset().y;
        assert_eq!(delivered, 80.0);
        clock.run(1, 1.0 / 60.0);
        assert_eq!(animator.offset().y, delivered);
        assert!(!animator.is_gliding());
        clock.run(400, 1.0 / 60.0);
        assert!(animator.offset().y > delivered + 60.0);
        assert!(animator.offset().y < delivered + 130.0);
        assert!(is_finished(&animator));
        assert_eq!(animator.velocity(), Point::default());
    });
}

#[test]
fn one_or_two_notches_are_a_nudge_not_a_throw() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 2, 30.0);
        clock.run(400, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 60.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn deliberate_slow_notches_rest_where_they_land() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        clock.now.set(clock.now.get() + 0.25);
        animator.scroll_by(Point::new(0.0, 30.0));
        clock.now.set(clock.now.get() + 0.25);
        animator.scroll_by(Point::new(0.0, 30.0));
        clock.run(400, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 60.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn a_notch_while_coasting_brakes_and_does_not_relaunch() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 10, 8.0);
        clock.run(30, 1.0 / 60.0);
        assert!(animator.is_gliding());
        clock.now.set(clock.now.get() + 1.0 / 60.0);
        let braked = animator.scroll_by(Point::new(0.0, 30.0)).y;
        assert!(!animator.is_gliding());
        clock.run(400, 1.0 / 60.0);
        assert_eq!(animator.offset().y, braked);
        assert!(is_finished(&animator));
    });
}

#[test]
fn stopping_a_glide_leaves_the_offset_where_it_was() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 10, 8.0);
        clock.run(20, 1.0 / 60.0);
        assert!(animator.is_gliding());
        let caught = animator.offset().y;
        animator.stop_coasting();
        assert!(!animator.is_gliding());
        clock.run(200, 1.0 / 60.0);
        assert_eq!(animator.offset().y, caught);
    });
}

fn coast_distance(frame_interval: f64) -> f32 {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 10, 8.0);
        let delivered = animator.offset().y;
        clock.run((6.0 / frame_interval) as usize, frame_interval);
        animator.offset().y - delivered
    })
}

#[test]
fn how_far_a_flick_travels_does_not_depend_on_the_frame_rate() {
    let at_thirty = coast_distance(1.0 / 30.0);
    let at_two_forty = coast_distance(1.0 / 240.0);
    assert!((at_thirty - at_two_forty).abs() < 2.0);
}

#[test]
fn coasting_into_the_end_of_the_range_bounces_instead_of_stopping() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 200.0);
        wheel(&animator, &clock, 10, 10.0);
        let mut furthest = 0.0_f32;
        for _ in 0..600 {
            clock.run(1, 1.0 / 60.0);
            furthest = furthest.max(animator.offset().y);
        }
        assert!(furthest > 200.0);
        assert!(furthest < 300.0);
        assert_eq!(animator.offset().y, 200.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn clamped_physics_stops_dead_at_the_edge_with_nothing_left_over() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::clamped(), 200.0);
        wheel(&animator, &clock, 10, 10.0);
        for _ in 0..600 {
            clock.run(1, 1.0 / 60.0);
        }
        assert_eq!(animator.offset().y, 200.0);
        assert_eq!(animator.position().y, 200.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn precise_lands_every_notch_and_nothing_else() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::precise(), 1_000.0);
        wheel(&animator, &clock, 10, 8.0);
        clock.run(400, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 80.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn letting_go_of_a_moving_thumb_keeps_the_content_moving() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        for index in 1..=8 {
            clock.now.set(clock.now.get() + 1.0 / 60.0);
            animator.move_to(Point::new(0.0, index as f32 * 12.0));
        }
        let released = animator.offset().y;
        assert_eq!(released, 96.0);
        animator.settle();
        assert!(animator.is_gliding());
        clock.run(400, 1.0 / 60.0);
        assert!(animator.offset().y > released + 100.0);
        assert!(is_finished(&animator));
    });
}

#[test]
fn holding_still_before_letting_go_does_not_fling() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        for index in 1..=8 {
            clock.now.set(clock.now.get() + 1.0 / 60.0);
            animator.move_to(Point::new(0.0, index as f32 * 12.0));
        }
        clock.run(30, 1.0 / 60.0);
        animator.settle();
        assert!(!animator.is_gliding());
        assert_eq!(animator.offset().y, 96.0);
    });
}

#[test]
fn an_authoritative_write_during_a_glide_stops_it_where_it_is() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 10, 8.0);
        clock.run(20, 1.0 / 60.0);
        assert!(animator.is_gliding());
        let mid_flight = animator.offset().y;
        assert!(mid_flight > 80.0);
        animator.adopt(Point::default());
        assert!(!animator.is_gliding());
        clock.run(200, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 0.0);
    });
}

#[test]
fn a_flick_into_either_bound_never_escapes_the_rubber_band() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 40.0);
        wheel(&animator, &clock, 20, 40.0);
        for _ in 0..600 {
            clock.run(1, 1.0 / 60.0);
            assert!(animator.offset().y <= 140.0);
            assert!(animator.offset().y >= -100.0);
        }
        assert_eq!(animator.offset().y, 40.0);
    });
}

#[test]
fn launch_velocity_is_clamped_so_a_burst_cannot_fling_far() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 100_000.0);
        for _ in 0..4 {
            clock.now.set(clock.now.get() + 0.001);
            animator.scroll_by(Point::new(0.0, 10.0));
        }
        let velocity = animator.current_velocity();
        assert!((velocity.y - 1_100.0).abs() < 1.0);
        let delivered = animator.offset().y;
        clock.run(600, 1.0 / 60.0);
        assert!(animator.offset().y - delivered < 300.0);
    });
}

#[test]
fn click_during_the_quiet_window_cancels_the_pending_throw_immediately() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 6, 8.0);
        let held = animator.offset();
        animator.stop_coasting();
        assert_eq!(AnimatorScheduler::current().active_count(), 0);
        clock.run(120, 1.0 / 60.0);
        assert_eq!(animator.offset(), held);
        assert!(is_finished(&animator));
    });
}

#[test]
fn authoritative_writes_clear_pending_release_and_cancellation_clears_old_samples() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 6, 8.0);
        animator.adopt(Point::new(0.0, 200.25));
        clock.run(120, 1.0 / 60.0);
        assert_eq!(animator.offset().y, 200.25);
        wheel(&animator, &clock, 6, 8.0);
        animator.cancel();
        animator.scroll_by(Point::new(0.0, 1.0));
        let held = animator.offset();
        clock.run(120, 1.0 / 60.0);
        assert_eq!(animator.offset(), held);
    });
}

#[test]
fn one_edge_rebounds_while_the_other_axis_continues_gliding() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = ScrollAnimator::new(
            ScrollPhysics::DEFAULT,
            Point::new(75.0, 2_000.0),
            Size { width: 100.0, height: 100.0 },
            Point::default(),
            None,
        );
        let now = clock.now.clone();
        animator.set_clock(Rc::new(move || now.get()));
        for _ in 0..6 {
            clock.now.set(clock.now.get() + 1.0 / 60.0);
            animator.scroll_by(Point::new(10.0, 10.0));
        }
        animator.settle();
        clock.run(5, 1.0 / 60.0);
        assert!(animator.is_settling());
        assert!(animator.is_gliding());
        assert!(animator.offset().x > 75.0);
        let y = animator.offset().y;
        clock.run(30, 1.0 / 60.0);
        assert!((animator.offset().x - 75.0).abs() < 1.0);
        assert!(animator.offset().y > y);
        clock.run(120, 1.0 / 60.0);
        assert!(is_finished(&animator));
        assert_eq!(AnimatorScheduler::current().active_count(), 0);
    });
}

#[test]
fn a_glide_decelerates_through_the_slow_tail_without_rounding_or_reversal() {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 1_000.0);
        wheel(&animator, &clock, 6, 8.0);
        animator.settle();
        let mut previous = animator.offset().y;
        let mut distance = f32::MAX;
        let mut last_distance = 0.0_f32;
        while !is_finished(&animator) {
            clock.run(1, 1.0 / 120.0);
            let next = animator.offset().y - previous;
            assert!(next >= 0.0);
            assert!(next <= distance + 0.0001);
            previous = animator.offset().y;
            distance = next;
            last_distance = next;
        }
        assert!(last_distance < 0.03);
    });
}

fn edge_handoff_sample(interval: f64) -> Point {
    with_isolated_scheduler(|| {
        let clock = ManualClock::new();
        let animator = make_coasting_animator(&clock, ScrollPhysics::DEFAULT, 100.0);
        wheel(&animator, &clock, 6, 10.0);
        animator.settle();
        clock.run((0.3 / interval).round() as usize, interval);
        animator.offset()
    })
}

#[test]
fn a_glide_crossing_its_edge_follows_the_same_path_at_thirty_and_two_forty_hz() {
    let coarse = edge_handoff_sample(1.0 / 30.0);
    let fine = edge_handoff_sample(1.0 / 240.0);
    assert!((coarse.y - fine.y).abs() < 0.001);
}
