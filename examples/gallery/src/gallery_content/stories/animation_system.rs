//! Animation-system stories: tracks, phases, shared geometry and completion.
//!
//! Swift source: `GalleryContent/Stories/Foundation/AnimationSystem.swift`.
//! Every animation type the Swift story authors already exists in
//! `cove_core::scene::animation`, so the state machines port exactly.

use cove_composition::Scope;
use cove_core::scene::animation::{Animation, AnimationCurve, AnimationKeyframe, KeyframeTimeline};
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
        if self.looping.get() {
            "Stop loop"
        } else {
            "Start loop"
        }
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
        if self.trailing.get() {
            (130.0, 44.0)
        } else {
            (90.0, 28.0)
        }
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

impl cove_ui_compose::View for AnimationSystemExamples {
    fn body(&self, _cx: &Scope) -> cove_ui_compose::AnyView {
        use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
        use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};

        use cove_ui_compose::primitives::controls::Button;
        use cove_ui_compose::primitives::feedback::{Badge, StatusTone};
        use cove_ui_compose::primitives::text::Text;
        use cove_ui_compose::theme::styles::buttons::SecondaryButtonStyle;
        use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
        use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
        use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView};

        use crate::gallery_content::stories::StorySection;

        let enabled = self.is_enabled;

        // Tracks & phases
        let play = {
            let run = self.run.clone();
            AnyView::new(
                AnyView::new(
                    Button::title("Play sequence", move || run.update(|value| *value += 1))
                        .enabled(enabled),
                )
                .debug_name("animation-sequence-play"),
            )
        };
        let track_badge = AnyView::new(Badge::new("Keyframes").tone(StatusTone::Info));
        let phase_badge =
            AnyView::new(Badge::new(DemoPhase::Idle.raw_value()).tone(StatusTone::Info));
        let badges = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            track_badge,
                        )
                        .debug_name("animation-keyframe"),
                    ),
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Column,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            phase_badge,
                        )
                        .debug_name("animation-phase"),
                    ),
                ])),
            )
            .frame_height(58.0),
        );

        let loop_toggle = {
            let looping = self.looping.clone();
            // Styling returns `ModifiedContent`, so it must be the last call
            // before erasing; the debug name is applied to the erased view.
            AnyView::new(
                AnyView::new(
                    Button::title(
                        if self.looping.get() {
                            "Stop loop"
                        } else {
                            "Start loop"
                        },
                        move || looping.update(|value| *value = !*value),
                    )
                    .enabled(enabled)
                    .button_style(SecondaryButtonStyle),
                )
                .debug_name("animation-loop-toggle"),
            )
        };

        let tracks = AnyView::new(StorySection::new(
            "Tracks & phases",
            "Play a scale track beside a sequence of domain phases. Both stop when their owning view leaves the tree.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![play, badges, loop_toggle])),
            )),
        ));

        // Numbers & completion
        let add = {
            let count = self.count.clone();
            AnyView::new(
                Button::title("Add 10", move || count.update(|value| *value += 10.0))
                    .enabled(enabled)
                    .debug_name("animation-number-add"),
            )
        };
        let complete = {
            let expanded = self.expanded.clone();
            let completed = self.completed.clone();
            AnyView::new(
                Button::title("Animate with completion", move || {
                    expanded.update(|value| *value = !*value);
                    completed.update(|value| *value += 1);
                })
                .enabled(enabled)
                .button_style(SecondaryButtonStyle)
                .debug_name("animation-completion-play"),
            )
        };
        let bar = AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(Text::new("")),
            )
            .frame_size(self.completion_bar_width(), 16.0)
            .semantic_background(SemanticColorRef::accent())
            .debug_name("animation-completion-bar"),
        );

        let numbers = AnyView::new(StorySection::new(
            "Numbers & completion",
            "Numbers interpolate without delaying their accessible value. Completion runs after the animation finishes and is discarded on interruption.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(
                                Text::new(format!("{:.2}", self.count.get()))
                                    .debug_name("animation-number"),
                            ),
                            add,
                        ])),
                    )),
                    complete,
                    bar,
                    AnyView::new(
                        Text::new(self.completed_line()).debug_name("animation-completion-count"),
                    ),
                ])),
            )),
        ));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![tracks, numbers])),
            )
            .flex(0.0)
            .debug_name("animation-system"),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_ui_compose::DynamicField> {
        vec![
            cove_ui_compose::DynamicField::state("run", &self.run),
            cove_ui_compose::DynamicField::state("trailing", &self.trailing),
            cove_ui_compose::DynamicField::state("looping", &self.looping),
            cove_ui_compose::DynamicField::state("count", &self.count),
            cove_ui_compose::DynamicField::state("expanded", &self.expanded),
            cove_ui_compose::DynamicField::state("completed", &self.completed),
        ]
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
        assert!(
            track.value(0.12) > 1.0,
            "the first keyframe overshoots above 1"
        );
        // Between the overshoot and the dip the value falls below the start.
        assert!(track.value(0.30) < 1.0, "the second keyframe dips below 1");
    }

    #[test]
    fn phases_match_the_swift_enum() {
        assert_eq!(
            DemoPhase::ALL.map(DemoPhase::raw_value),
            ["Idle", "Working", "Done"]
        );
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
