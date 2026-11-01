//! Timing-curve stories.
//!
//! Swift source: `GalleryContent/Stories/Foundation/AnimationCurves.swift`.
//! The story authors real curves through the shared `Animation` type, so it
//! exercises the same evaluation the runtime uses.

use cove_core::State;
use cove_core::scene::animation::{
    Animation, AnimationCurve, AnimationStepPosition, LinearTimingCurve, TimingPoint,
};

/// The four curves the story offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CurveChoice {
    #[default]
    Steps,
    Linear,
    Bounce,
    Elastic,
}

impl CurveChoice {
    pub const ALL: [Self; 4] = [Self::Steps, Self::Linear, Self::Bounce, Self::Elastic];

    /// The Swift `rawValue`, used in the toggle button's debug name.
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Steps => "steps",
            Self::Linear => "linear",
            Self::Bounce => "bounce",
            Self::Elastic => "elastic",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Steps => "Steps",
            Self::Linear => "Linear points",
            Self::Bounce => "Bounce out",
            Self::Elastic => "Elastic out",
        }
    }

    /// The curve this choice plays.
    pub fn curve(self) -> AnimationCurve {
        match self {
            Self::Steps => AnimationCurve::Steps(5, AnimationStepPosition::End),
            // Plateau at 0.6 then overshoot to 1.2 before settling at 1.
            Self::Linear => AnimationCurve::PiecewiseLinear(LinearTimingCurve::new(vec![
                TimingPoint {
                    time: 0.0,
                    progress: 0.0,
                },
                TimingPoint {
                    time: 0.25,
                    progress: 0.6,
                },
                TimingPoint {
                    time: 0.5,
                    progress: 0.6,
                },
                TimingPoint {
                    time: 0.75,
                    progress: 1.2,
                },
                TimingPoint {
                    time: 1.0,
                    progress: 1.0,
                },
            ])),
            Self::Bounce => AnimationCurve::BounceOut,
            Self::Elastic => AnimationCurve::ElasticOut,
        }
    }

    /// The animation the "Play motion" button runs.
    pub fn animation(self) -> Animation {
        Animation::new(1.0, self.curve()).with_overshoot(true)
    }
}

/// The timing-curve story state.
pub struct AnimationCurveExamples {
    pub is_enabled: bool,
    pub choice: State<CurveChoice>,
    pub expanded: State<bool>,
}

impl AnimationCurveExamples {
    pub fn new(is_enabled: bool) -> Self {
        Self {
            is_enabled,
            choice: State::new(CurveChoice::Steps),
            expanded: State::new(false),
        }
    }

    /// Target bar width in points: 80 collapsed, 200 expanded.
    pub fn target_width(&self) -> f32 {
        if self.expanded.get() { 200.0 } else { 80.0 }
    }

    /// `choice.title · target width Npt`
    pub fn caption(&self) -> String {
        format!(
            "{} · target width {}pt",
            self.choice.get().title(),
            self.target_width() as i32
        )
    }

    /// The debug name of a curve toggle.
    pub fn toggle_debug_name(choice: CurveChoice) -> String {
        format!("animation-curve-choice-{}", choice.raw_value())
    }
}

impl cove_ui_compose::View for AnimationCurveExamples {
    fn body(&self) -> cove_ui_compose::AnyView {
        use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
        use cove_ui_compose::modifiers::layout::LayoutFrameExt;
        use cove_ui_compose::modifiers::style::AppearanceExt;
        use cove_ui_compose::primitives::controls::Button;
        use cove_ui_compose::primitives::text::Text;
        use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
        use cove_ui_compose::theme::styles::buttons::{SecondaryButtonStyle, ToggleButtonStyle};
        use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
        use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView};

        use crate::gallery_content::stories::StorySection;

        let enabled = self.is_enabled;
        let choice = self.choice.get();

        // One toggle per curve, the active one selected.
        let choices = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(
                CurveChoice::ALL
                    .iter()
                    .map(|candidate| {
                        let state = self.choice.clone();
                        let value = *candidate;
                        AnyView::new(
                            Button::title(value.title(), move || state.set(value))
                                .enabled(enabled)
                                .selected(choice == value)
                                .button_style(ToggleButtonStyle::default())
                                .debug_name(&Self::toggle_debug_name(value)),
                        )
                    })
                    .collect(),
            )),
        ));

        let play = {
            let expanded = self.expanded.clone();
            AnyView::new(
                Button::title("Play motion", move || {
                    expanded.update(|value| *value = !*value)
                })
                .enabled(enabled)
                .button_style(SecondaryButtonStyle)
                .debug_name("animation-curve-play"),
            )
        };

        // The animated bar: 80pt collapsed, 200pt expanded.
        let bar = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(Text::new("")),
                    )
                    .frame_size(self.target_width(), 28.0)
                    .semantic_background(SemanticColorRef::accent())
                    .corner_radius(7.0)
                    .debug_name("animation-curve-bar"),
                ),
            )
            .frame_height(64.0)
            .semantic_background(SemanticColorRef::surface_sunken())
            .corner_radius(8.0),
        );

        AnyView::new(StorySection::new(
            "Timing curves",
            "Choose a curve and play in either direction. Steps move in intervals; linear points can plateau and overshoot; bounce and elastic settle after oscillating.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    choices,
                    play,
                    bar,
                    AnyView::new(
                        Text::new(self.caption())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                ])),
            )),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_ui_compose::DynamicField> {
        vec![
            cove_ui_compose::DynamicField::state("choice", &self.choice),
            cove_ui_compose::DynamicField::state("expanded", &self.expanded),
        ]
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn curve_choices_match_the_swift_enum() {
        assert_eq!(
            CurveChoice::ALL.map(CurveChoice::raw_value),
            ["steps", "linear", "bounce", "elastic"]
        );
        assert_eq!(
            CurveChoice::ALL.map(CurveChoice::title),
            ["Steps", "Linear points", "Bounce out", "Elastic out"]
        );
        assert_eq!(CurveChoice::default(), CurveChoice::Steps);
    }

    #[test]
    fn steps_curve_advances_in_five_intervals() {
        let curve = CurveChoice::Steps.curve();
        assert_eq!(curve, AnimationCurve::Steps(5, AnimationStepPosition::End));
        // With the default End position each interval holds, then jumps.
        assert_eq!(curve.evaluate(0.0), 0.0);
        assert_eq!(curve.evaluate(0.1), 0.0);
        assert_eq!(curve.evaluate(0.2), 0.2);
        assert_eq!(curve.evaluate(0.99), 0.8);
        assert_eq!(curve.evaluate(1.0), 1.0);
    }

    #[test]
    fn linear_points_plateau_then_overshoot() {
        let AnimationCurve::PiecewiseLinear(curve) = CurveChoice::Linear.curve() else {
            panic!("linear choice must build a piecewise-linear curve")
        };
        // Anchors are exact.
        assert_eq!(curve.evaluate(0.0), 0.0);
        assert_eq!(curve.evaluate(0.25), 0.6);
        assert_eq!(curve.evaluate(0.5), 0.6);
        assert_eq!(curve.evaluate(0.75), 1.2);
        assert_eq!(curve.evaluate(1.0), 1.0);
        // The plateau between 0.25 and 0.5 holds 0.6, and the midpoint of the
        // ascent to 1.2 interpolates halfway.
        assert!((curve.evaluate(0.375) - 0.6).abs() < 1e-6);
        assert!((curve.evaluate(0.625) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn bounce_and_elastic_settle_at_both_ends() {
        for choice in [CurveChoice::Bounce, CurveChoice::Elastic] {
            let curve = choice.curve();
            assert_eq!(curve.evaluate(0.0), 0.0, "{choice:?} starts at 0");
            assert_eq!(curve.evaluate(1.0), 1.0, "{choice:?} ends at 1");
        }
    }

    #[test]
    fn play_animation_is_one_second_with_overshoot() {
        for choice in CurveChoice::ALL {
            let animation = choice.animation();
            assert_eq!(animation.duration, 1.0);
            assert!(animation.allows_overshoot, "overshoot stays enabled");
            assert_eq!(animation.curve, choice.curve());
        }
    }

    #[test]
    fn target_width_and_caption_follow_expansion() {
        let story = AnimationCurveExamples::new(true);
        assert_eq!(story.target_width(), 80.0);
        assert_eq!(story.caption(), "Steps · target width 80pt");
        story.expanded.set(true);
        assert_eq!(story.target_width(), 200.0);
        assert_eq!(story.caption(), "Steps · target width 200pt");
        story.choice.set(CurveChoice::Bounce);
        assert_eq!(story.caption(), "Bounce out · target width 200pt");
    }

    #[test]
    fn toggle_debug_names_match_the_gallery() {
        assert_eq!(
            AnimationCurveExamples::toggle_debug_name(CurveChoice::Elastic),
            "animation-curve-choice-elastic"
        );
    }
}
