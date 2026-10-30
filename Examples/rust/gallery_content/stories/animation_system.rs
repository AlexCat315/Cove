//! Animation-system stories: tracks, phases, shared geometry and completion.
//!
//! Swift source: `GalleryContent/Stories/Foundation/AnimationSystem.swift`.
//! Every animation type the Swift story authors already exists in
//! `cove_core::scene::animation`, so the state machines port exactly.

use cove_core::scene::animation::{
    Animation, AnimationCurve, AnimationKeyframe, KeyframeTimeline,
};
use cove_core::{Point, State};

/// The three keyframes the scale track plays, and its total duration.
pub fn scale_track() -> KeyframeTimeline<f32> {
    KeyframeTimeline::new(
        1.0,
        vec![
            AnimationKeyframe::new(1.12, 0.12, AnimationCurve::EaseOut),
            AnimationKeyframe::new(0.94, 0.12, AnimationCurve::EaseInOut),
            AnimationKeyframe::new(1.0, 0.16, AnimationCurve::EaseOut),
        ],
    )
}

/// The sequence phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DemoPhase {
    #[default]
    Idle,
    Working,
    Done,
}

impl DemoPhase {
    pub const ALL: [Self; 3] = [Self::Idle, Self::Working, Self::Done];

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Working => "Working",
            Self::Done => "Done",
        }
    }

    /// `phase == .working ? 1.08 : 1`
    pub fn scale(self) -> f32 {
        if self == Self::Working { 1.08 } else { 1.0 }
    }
}

/// `.easeInOut` at 0.2 s, shared by every phase transition.
pub fn phase_animation() -> Animation {
    Animation::new(0.2, AnimationCurve::EaseInOut)
}

/// `.easeInOut` at 0.3 s for the completion demo.
pub fn completion_animation() -> Animation {
    Animation::new(0.3, AnimationCurve::EaseInOut)
}

/// The `AnimationSystemExamples` state.
pub struct AnimationSystemExamples {
    pub is_enabled: bool,
    pub run: State<i32>,
    pub trailing: State<bool>,
    pub looping: State<bool>,
    pub count: State<f64>,
    pub drag: State<Point>,
    pub expanded: State<bool>,
    pub completed: State<i32>,
}

impl AnimationSystemExamples {
    pub fn new(is_enabled: bool) -> Self {
        Self {
            is_enabled,
            run: State::new(0),
            trailing: State::new(false),
            looping: State::new(false),
            count: State::new(12.0),
            drag: State::new(Point::new(0.0, 0.0)),
            expanded: State::new(false),
            completed: State::new(0),
        }
    }

    /// Replays the track and phase sequence.
    pub fn play_sequence(&self) {
        self.run.update(|run| *run += 1);
    }

    pub fn toggle_loop(&self) {
        self.looping.update(|looping| *looping = !*looping);
    }

    /// The loop control's label.
    pub fn loop_label(&self) -> &'static str {
        if self.looping.get() { "Stop loop" } else { "Start loop" }
    }

    /// Whether the repeating track is currently mounted.
    pub fn shows_repeating_track(&self) -> bool {
        self.looping.get() && self.is_enabled
    }

    pub fn move_selection(&self) {
        self.trailing.update(|trailing| *trailing = !*trailing);
    }

    /// The moving surface's size, which changes with its destination.
    pub fn selection_size(&self) -> (f32, f32) {
        if self.trailing.get() { (130.0, 44.0) } else { (90.0, 28.0) }
    }

    /// Adds 10 to the interpolated numeric value.
    pub fn add_ten(&self) {
        self.count.update(|count| *count += 10.0);
    }

    /// Runs the completion demo: toggles the bar and counts the completion.
    pub fn animate_with_completion(&self) {
        self.expanded.update(|expanded| *expanded = !*expanded);
        self.completed.update(|completed| *completed += 1);
    }

    /// The animated bar's width: 80 collapsed, 200 expanded.
    pub fn completion_bar_width(&self) -> f32 {
        if self.expanded.get() { 200.0 } else { 80.0 }
    }

    pub fn completed_line(&self) -> String {
        format!("Completed: {}", self.completed.get())
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn scale_track_matches_the_swift_keyframes() {
        let track = scale_track();
        assert_eq!(track.initial_value, 1.0);
        // 0.12 + 0.12 + 0.16
        assert!((track.duration() - 0.4).abs() < 1e-9);
        // Before it starts, and once it finishes.
        assert_eq!(track.value(0.0), 1.0);
        assert_eq!(track.value(-1.0), 1.0);
        assert_eq!(track.value(1.0), 1.0);
        // Mid-overshoot exceeds 1.
        assert!(track.value(0.12) > 1.0, "the first keyframe overshoots above 1");
        // Between the overshoot and the dip the value falls below the start.
        assert!(track.value(0.30) < 1.0, "the second keyframe dips below 1");
    }

    #[test]
    fn phases_match_the_swift_enum() {
        assert_eq!(DemoPhase::ALL.map(DemoPhase::raw_value), ["Idle", "Working", "Done"]);
        assert_eq!(DemoPhase::default(), DemoPhase::Idle);
        assert_eq!(DemoPhase::Working.scale(), 1.08);
        assert_eq!(DemoPhase::Idle.scale(), 1.0);
        assert_eq!(DemoPhase::Done.scale(), 1.0);
        assert_eq!(phase_animation().duration, 0.2);
        assert_eq!(phase_animation().curve, AnimationCurve::EaseInOut);
        assert_eq!(completion_animation().duration, 0.3);
    }

    #[test]
    fn sequence_replays_increment_the_trigger() {
        let story = AnimationSystemExamples::new(true);
        assert_eq!(story.run.get(), 0);
        story.play_sequence();
        story.play_sequence();
        assert_eq!(story.run.get(), 2);
    }

    #[test]
    fn the_loop_only_mounts_while_enabled() {
        let story = AnimationSystemExamples::new(true);
        assert_eq!(story.loop_label(), "Start loop");
        assert!(!story.shows_repeating_track());
        story.toggle_loop();
        assert_eq!(story.loop_label(), "Stop loop");
        assert!(story.shows_repeating_track());

        // A disabled gallery never mounts the repeating track.
        let disabled = AnimationSystemExamples::new(false);
        disabled.toggle_loop();
        assert!(!disabled.shows_repeating_track());
    }

    #[test]
    fn the_selection_surface_moves_between_two_sizes() {
        let story = AnimationSystemExamples::new(true);
        assert_eq!(story.selection_size(), (90.0, 28.0));
        story.move_selection();
        assert_eq!(story.selection_size(), (130.0, 44.0));
        assert!(story.trailing.get());
        story.move_selection();
        assert_eq!(story.selection_size(), (90.0, 28.0));
    }

    #[test]
    fn numbers_step_by_ten_and_completion_counts() {
        let story = AnimationSystemExamples::new(true);
        assert_eq!(story.count.get(), 12.0);
        story.add_ten();
        assert_eq!(story.count.get(), 22.0);
        assert_eq!(story.completion_bar_width(), 80.0);
        assert_eq!(story.completed_line(), "Completed: 0");
        story.animate_with_completion();
        assert_eq!(story.completion_bar_width(), 200.0);
        assert_eq!(story.completed_line(), "Completed: 1");
        story.animate_with_completion();
        assert_eq!(story.completion_bar_width(), 80.0);
        assert_eq!(story.completed_line(), "Completed: 2");
    }

    #[test]
    fn the_drag_starts_at_the_origin() {
        let story = AnimationSystemExamples::new(true);
        assert_eq!(story.drag.get(), Point::new(0.0, 0.0));
    }
}
