//! Feedback stories: badges, alerts, progress, empty states.
//!
//! Swift source: `GalleryContent/Stories/Feedback/FeedbackStories.swift`
//! (`BadgeStory`, `AlertStory`, `ProgressStory`, `EmptyStory`).

use cove_composition::Scope;
use cove_core::State;
use cove_core::scene::animation::AnimationCurve;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::{FlexExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::feedback::{
    Alert, AlertTone, Badge, BadgeVariant, EmptyState, ProgressCircle, ProgressView, Spinner,
    StatusTone,
};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::theme::{Theme, ThemeReader};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::catalog::GalleryOptions;

/// The tones the "Status tones" section lists, in `StatusTone.allCases` order.
pub const STATUS_TONES: [StatusTone; 5] = [
    StatusTone::Neutral,
    StatusTone::Info,
    StatusTone::Success,
    StatusTone::Warning,
    StatusTone::Danger,
];

/// Swift's `String(describing: tone).capitalized`.
pub fn tone_label(tone: StatusTone) -> &'static str {
    match tone {
        StatusTone::Neutral => "Neutral",
        StatusTone::Info => "Info",
        StatusTone::Success => "Success",
        StatusTone::Warning => "Warning",
        StatusTone::Danger => "Danger",
    }
}

/// `BadgeStory`.
pub struct BadgeStory;

impl View for BadgeStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let tones = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(
                STATUS_TONES
                    .iter()
                    .map(|tone| AnyView::new(Badge::new(tone_label(*tone)).tone(*tone)))
                    .collect(),
            )),
        ));

        let variants = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Badge::new("Running")
                        .tone(StatusTone::Success)
                        .variant(BadgeVariant::Soft),
                ),
                AnyView::new(
                    Badge::new("Running")
                        .tone(StatusTone::Success)
                        .variant(BadgeVariant::Solid),
                ),
                AnyView::new(
                    Badge::new("Running")
                        .tone(StatusTone::Success)
                        .variant(BadgeVariant::Outline),
                ),
            ])),
        ));

        let glyphs = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Badge::new("Passing")
                        .tone(StatusTone::Success)
                        .icon(icons::status::check_circle()),
                ),
                AnyView::new(
                    Badge::new("3 failures")
                        .tone(StatusTone::Danger)
                        .shows_dot(true),
                ),
                AnyView::new(
                    Badge::new("Unverified")
                        .tone(StatusTone::Neutral)
                        .enabled(false),
                ),
            ])),
        ));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Status tones",
                        "Semantic status labels inherit the active theme.",
                        tones,
                    )),
                    AnyView::new(StorySection::new(
                        "Long status",
                        "Status text wraps inside the available width and keeps its complete meaning.",
                        AnyView::new(
                            Badge::new("Workspace synchronization is waiting for a network connection")
                                .tone(StatusTone::Warning)
                                .shows_dot(true)
                                .debug_name("badge-long"),
                        ),
                    )),
                    AnyView::new(StorySection::new(
                        "Variants",
                        "Soft reads inside dense content, solid is a single callout, and outline sits beside a bordered surface.",
                        variants,
                    )),
                    AnyView::new(StorySection::new(
                        "Glyphs, dots and density",
                        "The glyph carries the same meaning as the colour, so the tone survives a monochrome theme.",
                        glyphs,
                    )),
                ])),
            )
            .debug_name("badge-story"),
        )
    }
}

/// A titled story group rendered as a hairline card.
pub struct StorySection {
    pub title: String,
    pub detail: String,
    pub content: AnyView,
}

impl StorySection {
    pub fn new(title: impl Into<String>, detail: impl Into<String>, content: AnyView) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            content,
        }
    }

    /// The DevTools name for this section (`LayoutDebugExt::debug_name` is a
    /// view modifier, so this accessor needs a distinct name).
    pub fn debug_name_value(&self) -> String {
        format!("story-section-{}", self.title)
    }
}

impl View for StorySection {
    fn body(&self, _cx: &Scope) -> AnyView {
        let title = self.title.clone();
        let detail = self.detail.clone();
        let content = self.content.clone();
        let card = AnyView::new(ThemeReader::new(move |theme: &Theme| {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    12.0,
                    content.clone(),
                )
                .padding(18.0)
                .background_color(theme.colors.surface)
                .corner_radius(theme.radius.xl)
                .border_color(theme.colors.border, 1.0),
            )
        }));
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        Text::new(title)
                            .semantic_font(cove_ui_compose::theme::SemanticFontRef::headline())
                            .semantic_foreground(SemanticColorRef::on_surface()),
                    ),
                    AnyView::new(
                        Text::new(detail).semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                    card,
                ])),
            )
            .flex(0.0)
            .debug_name(&self.debug_name_value()),
        )
    }
}

/// `AlertStory`.
pub struct AlertStory {
    pub dismissed: State<bool>,
    pub result: State<String>,
}

impl AlertStory {
    pub fn new() -> Self {
        Self {
            dismissed: State::new(false),
            result: State::new("No action yet".into()),
        }
    }

    pub fn dismiss(&self) {
        self.dismissed.set(true);
        self.result.set("Alert dismissed".into());
    }

    pub fn reconnect(&self) {
        self.result.set("Reconnection requested".into());
    }

    pub fn work_offline(&self) {
        self.result.set("Working offline".into());
    }

    pub fn show_alert(&self) {
        self.dismissed.set(false);
    }
}

impl Default for AlertStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for AlertStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new("Alert")),
                    AnyView::new(
                        Text::new(self.result.get())
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                    ),
                    AnyView::new(
                        Alert::new("Project saved")
                            .message("All changes are available in the workspace.")
                            .tone(StatusTone::Success)
                            .variant(AlertTone::Soft),
                    ),
                    AnyView::new(
                        Alert::new("Connection interrupted")
                            .message("Retry when the connection is available.")
                            .tone(StatusTone::Warning)
                            .variant(AlertTone::Soft),
                    ),
                    AnyView::new(
                        Alert::new("Import failed")
                            .message("The selected asset format is unsupported.")
                            .tone(StatusTone::Danger)
                            .variant(AlertTone::Soft),
                    ),
                    AnyView::new(
                        Alert::new("Update available")
                            .message("Restart to use the new version.")
                            .tone(StatusTone::Info)
                            .variant(AlertTone::Soft),
                    ),
                ])),
            )
            .debug_name("alert-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("dismissed", &self.dismissed),
            DynamicField::state("result", &self.result),
        ]
    }
}

/// `ProgressStory`.
pub struct ProgressStory {
    pub options: GalleryOptions,
    pub progress: State<f64>,
}

impl ProgressStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            progress: State::new(0.45),
        }
    }

    /// `String(format: "%.0f%% complete", value * 100)`
    pub fn percent_line(&self) -> String {
        format!("{:.0}% complete", self.progress.get() * 100.0)
    }
}

impl View for ProgressStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let value = self.progress.get();
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(StorySection::new(
                        "Progress & activity",
                        "Determinate progress clamps to its range; the spinner stops when its owning node unmounts.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(ProgressView::new(Some(value))),
                                AnyView::new(
                                    ProgressView::new(Some(value))
                                        .tone(StatusTone::Success)
                                        .height(8.0),
                                ),
                                AnyView::new(
                                    Text::new(self.percent_line())
                                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                                ),
                                AnyView::new(BoxView::new(
                                    FlexDirection::Row,
                                    Align::Center,
                                    Justify::FlexStart,
                                    Wrap::NoWrap,
                                    12.0,
                                    AnyView::new(TupleView::new(vec![
                                        AnyView::new(Spinner::new()),
                                        AnyView::new(Text::new("Working…")),
                                    ])),
                                )),
                            ])),
                        )),
                    )),
                    AnyView::new(StorySection::new(
                        "Indeterminate",
                        "Used when the total work is unknown — a fabricated percentage that never advances is worse than admitting the wait.",
                        AnyView::new(BoxView::new(
                            FlexDirection::Column,
                            Align::Stretch,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            12.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(ProgressView::new(None)),
                                AnyView::new(ProgressView::new(None).tone(StatusTone::Warning)),
                                AnyView::new(BoxView::new(
                                    FlexDirection::Row,
                                    Align::Center,
                                    Justify::FlexStart,
                                    Wrap::NoWrap,
                                    16.0,
                                    AnyView::new(TupleView::new(vec![
                                        AnyView::new(
                                            ProgressCircle::new(Some(value))
                                                .diameter(44.0)
                                                .shows_value(true),
                                        ),
                                        AnyView::new(
                                            ProgressCircle::new(Some(value))
                                                .diameter(28.0)
                                                .tone(StatusTone::Success),
                                        ),
                                        AnyView::new(
                                            ProgressCircle::new(None)
                                                .diameter(44.0)
                                                .tone(StatusTone::Info),
                                        ),
                                        AnyView::new(
                                            ProgressCircle::new(Some(0.72))
                                                .diameter(56.0)
                                                .shows_value(true)
                                                .tone(StatusTone::Warning),
                                        ),
                                    ])),
                                )),
                            ])),
                        )),
                    )),
                ])),
            )
            .debug_name("progress-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("progress", &self.progress)]
    }
}

/// `EmptyStory`.
pub struct EmptyStory {
    pub created: State<bool>,
}

impl EmptyStory {
    pub fn new() -> Self {
        Self {
            created: State::new(false),
        }
    }

    pub fn create(&self) {
        self.created.set(true);
    }
}

impl Default for EmptyStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for EmptyStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let created = self.created.get();
        let story = EmptyStoryButton {
            empty: !created,
            story: self.clone_view(),
        };
        AnyView::new(
            FictionSection::new(
                "Empty content",
                "A recovery action turns an empty page into a useful starting point.",
                AnyView::new(story),
            )
            .debug_name("empty-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("created", &self.created)]
    }
}

impl EmptyStory {
    fn clone_view(&self) -> EmptyStoryHandles {
        EmptyStoryHandles {
            created: self.created.clone(),
        }
    }
}

/// Handle bundle so the button closure can flip the state.
pub struct EmptyStoryHandles {
    pub created: State<bool>,
}

struct EmptyStoryButton {
    empty: bool,
    story: EmptyStoryHandles,
}

impl View for EmptyStoryButton {
    fn body(&self, _cx: &Scope) -> AnyView {
        if self.empty {
            let created = self.story.created.clone();
            AnyView::new(EmptyState::with_action(
                "No projects yet",
                "Create a project to get started.",
                cove_ui_compose::primitives::controls::Button::title("Create project", move || {
                    created.set(true)
                }),
            ))
        } else {
            AnyView::new(
                Alert::new("Your first project is ready")
                    .message("Reset the example to restore the empty state.")
                    .tone(StatusTone::Success),
            )
        }
    }
}

/// `StorySection` with a different debug name prefix.
struct FictionSection {
    title: String,
    detail: String,
    content: AnyView,
}

impl FictionSection {
    fn new(title: impl Into<String>, detail: impl Into<String>, content: AnyView) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            content,
        }
    }
}

impl View for FictionSection {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            self.title.clone(),
            self.detail.clone(),
            self.content.clone(),
        ))
    }
}

/// Curve used by the numeric-value fade in the Swift story's `NumericText`.
pub fn numeric_curve() -> AnimationCurve {
    AnimationCurve::EaseInOut
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn tone_labels_match_swift_string_describing_capitalized() {
        assert_eq!(
            STATUS_TONES.map(tone_label),
            ["Neutral", "Info", "Success", "Warning", "Danger"]
        );
    }

    #[test]
    fn alert_story_tracks_dismissal_and_results() {
        let story = AlertStory::new();
        assert!(!story.dismissed.get());
        assert_eq!(story.result.get(), "No action yet");
        story.reconnect();
        assert_eq!(story.result.get(), "Reconnection requested");
        story.work_offline();
        assert_eq!(story.result.get(), "Working offline");
        story.dismiss();
        assert!(story.dismissed.get());
        assert_eq!(story.result.get(), "Alert dismissed");
        story.show_alert();
        assert!(!story.dismissed.get());
        // The result deliberately survives re-showing.
        assert_eq!(story.result.get(), "Alert dismissed");
    }

    #[test]
    fn progress_starts_at_45_percent_and_formats_without_decimals() {
        let story = ProgressStory::new(GalleryOptions::default());
        assert_eq!(story.progress.get(), 0.45);
        assert_eq!(story.percent_line(), "45% complete");
        story.progress.set(1.0);
        assert_eq!(story.percent_line(), "100% complete");
        story.progress.set(0.0);
        assert_eq!(story.percent_line(), "0% complete");
        story.progress.set(0.725);
        assert_eq!(story.percent_line(), "72% complete", "%.0f rounds");
    }

    #[test]
    fn empty_story_flips_once_created() {
        let story = EmptyStory::new();
        assert!(!story.created.get());
        story.create();
        assert!(story.created.get());
    }

    #[test]
    fn story_section_debug_name_matches_the_gallery() {
        let section = StorySection::new("Status tones", "detail", AnyView::new(Text::new("x")));
        assert_eq!(section.debug_name_value(), "story-section-Status tones");
    }
}
