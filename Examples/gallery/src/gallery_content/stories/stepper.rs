//! Stepper story: a controlled workflow with saved outcomes.
//!
//! Swift source: `GalleryContent/Stories/Controls/StepperStory.swift`.

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::feedback::{Alert, StatusTone};
use cove_ui_compose::primitives::navigation::stepper::{
    StepStatus, Stepper, StepperAxis, StepperItem, StepperOptions,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
use cove_ui_compose::theme::styles::buttons::{DestructiveButtonStyle, SecondaryButtonStyle};
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::catalog::GalleryOptions;
use crate::gallery_content::stories::StorySection;

/// The workflow step identifiers, in order.
pub const STEP_IDS: [&str; 3] = ["account", "details", "review"];

/// The detail line for the details step, which depends on the outcome.
pub fn details_detail(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Failed => "Configure your workspace",
        _ => "Configure your workspace",
    }
}

/// The review step's detail depends on whether details succeeded.
pub fn review_detail(unlocked: bool) -> &'static str {
    if unlocked {
        "Ready to review"
    } else {
        "Complete details first"
    }
}

/// `StepperStory`.
pub struct StepperStory {
    pub options: GalleryOptions,
    pub selected: State<Option<String>>,
    pub status: State<StepStatus>,
    pub unlock_review: State<bool>,
}

impl StepperStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selected: State::new(Some("details".into())),
            status: State::new(StepStatus::Pending),
            unlock_review: State::new(false),
        }
    }

    /// Start: in progress, review locked again.
    pub fn start(&self) {
        self.status.set(StepStatus::InProgress);
        self.unlock_review.set(false);
    }

    /// Fail: failed, review locked again.
    pub fn fail(&self) {
        self.status.set(StepStatus::Failed);
        self.unlock_review.set(false);
    }

    /// Complete: completed, review unlocked.
    pub fn complete(&self) {
        self.status.set(StepStatus::Completed);
        self.unlock_review.set(true);
    }

    /// `Current: {selection}`, or `Current: None`.
    pub fn current_line(&self) -> String {
        format!(
            "Current: {}",
            self.selected.get().unwrap_or_else(|| "None".into())
        )
    }

    /// The steps, rebuilt from the current outcomes.
    pub fn items(&self) -> Vec<StepperItem<String>> {
        let unlocked = self.unlock_review.get();
        vec![
            // The account step is always completed and preserves its detail.
            StepperItem::new("account".to_string(), "Account")
                .status(StepStatus::Completed)
                .detail("Profile saved"),
            StepperItem::new("details".to_string(), "Details")
                .status(self.status.get())
                .detail(details_detail(self.status.get())),
            // Review stays unavailable until details succeed.
            StepperItem::new("review".to_string(), "Review")
                .enabled(unlocked)
                .detail(review_detail(unlocked)),
        ]
    }

    pub fn selection_binding(&self) -> Binding<Option<String>> {
        Binding::new(
            {
                let selected = self.selected.clone();
                move || selected.get()
            },
            {
                let selected = self.selected.clone();
                move |value| selected.set(value)
            },
        )
    }
}

impl View for StepperStory {
    fn body(&self) -> AnyView {
        let enabled = self.options.is_enabled;

        let horizontal = AnyView::new(
            Stepper::new(self.items(), self.selection_binding()).options(StepperOptions {
                is_enabled: enabled,
                ..StepperOptions::default()
            }),
        );

        let start = {
            let status = self.status.clone();
            let unlock = self.unlock_review.clone();
            AnyView::new(
                Button::title("Start", move || {
                    status.set(StepStatus::InProgress);
                    unlock.set(false);
                })
                .enabled(enabled),
            )
        };
        let fail = {
            let status = self.status.clone();
            let unlock = self.unlock_review.clone();
            AnyView::new(
                Button::title("Fail", move || {
                    status.set(StepStatus::Failed);
                    unlock.set(false);
                })
                .enabled(enabled)
                .button_style(DestructiveButtonStyle),
            )
        };
        let complete = {
            let status = self.status.clone();
            let unlock = self.unlock_review.clone();
            AnyView::new(
                Button::title("Complete", move || {
                    status.set(StepStatus::Completed);
                    unlock.set(true);
                })
                .enabled(enabled)
                .button_style(SecondaryButtonStyle),
            )
        };

        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                start,
                fail,
                complete,
                AnyView::new(
                    Text::new(self.current_line())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ));

        // The failure alert appears only while the details step has failed.
        let failure = if self.status.get() == StepStatus::Failed {
            AnyView::new(
                Alert::new("Details could not be saved")
                    .message("Retry with Start, or review your configuration.")
                    .tone(StatusTone::Danger),
            )
        } else {
            AnyView::new(Text::new(""))
        };

        let vertical = AnyView::new(
            Stepper::new(self.items(), self.selection_binding())
                .options(StepperOptions {
                    axis: StepperAxis::Vertical,
                    is_enabled: enabled,
                    label: "Setup progress".into(),
                    ..StepperOptions::default()
                })
                .frame_width(360.0),
        );

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "A controlled workflow",
                        "Click a step or use Left/Right and Home/End. A narrow container stacks the steps and switches to Up/Down. Review stays unavailable until details succeed. Going back preserves the saved outcomes.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(TupleView::new(vec![horizontal, controls, failure])),
                        )),
                    )),
                    AnyView::new(StorySection::new(
                        "Vertical steps",
                        "Up/Down follow the vertical axis. The status glyph and text describe the outcome without relying on color.",
                        vertical,
                    )),
                ])),
            )
            .flex(0.0)
            .debug_name("stepper-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("selected", &self.selected),
            DynamicField::state("status", &self.status),
            DynamicField::state("unlockReview", &self.unlock_review),
        ]
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    fn story() -> StepperStory {
        StepperStory::new(GalleryOptions::default())
    }

    #[test]
    fn initial_state_matches_the_swift_story() {
        let story = story();
        assert_eq!(story.selected.get(), Some("details".to_string()));
        assert_eq!(story.status.get(), StepStatus::Pending);
        assert!(!story.unlock_review.get());
        assert_eq!(story.current_line(), "Current: details");
    }

    #[test]
    fn start_fails_and_completes_set_the_expected_outcome() {
        let story = story();
        story.start();
        assert_eq!(story.status.get(), StepStatus::InProgress);
        assert!(!story.unlock_review.get(), "start re-locks review");

        story.fail();
        assert_eq!(story.status.get(), StepStatus::Failed);
        assert!(!story.unlock_review.get(), "fail re-locks review");

        story.complete();
        assert_eq!(story.status.get(), StepStatus::Completed);
        assert!(story.unlock_review.get(), "complete unlocks review");
    }

    #[test]
    fn review_unlocks_only_after_completion() {
        let story = story();
        story.start();
        let items = story.items();
        assert_eq!(items.len(), 3);
        // Account is always completed with its detail preserved.
        assert_eq!(items[0].id, "account");
        assert_eq!(items[0].status, StepStatus::Completed);
        assert_eq!(items[0].detail, "Profile saved");
        // Review is disabled with the "complete details first" hint.
        assert!(!items[2].is_enabled);
        assert_eq!(items[2].detail, "Complete details first");
        assert_eq!(review_detail(false), "Complete details first");
        assert_eq!(review_detail(true), "Ready to review");

        story.complete();
        let items = story.items();
        assert_eq!(items[2].detail, "Ready to review");
        assert_eq!(items[1].status, StepStatus::Completed);
    }

    #[test]
    fn current_line_reports_none_when_nothing_is_selected() {
        let story = story();
        story.selected.set(None);
        assert_eq!(story.current_line(), "Current: None");
    }

    #[test]
    fn step_ids_match_the_swift_workflow() {
        assert_eq!(STEP_IDS, ["account", "details", "review"]);
        let story = story();
        let items = story.items();
        assert_eq!(items[0].id, STEP_IDS[0]);
        assert_eq!(items[1].id, STEP_IDS[1]);
        assert_eq!(items[2].id, STEP_IDS[2]);
        assert_eq!(items[1].title, "Details");
        assert_eq!(items[2].title, "Review");
    }
}
