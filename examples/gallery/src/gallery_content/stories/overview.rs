//! Overview stories: the dashboard and its metric cards.
//!
//! Swift sources: `GalleryContent/Stories/Overview/Dashboard.swift`,
//! `GalleryContent/Stories/Overview/MetricCard.swift`.
//!
//! Authored sample values are deterministic. Actions change real component
//! state; the gallery reset recreates this story and restores the initial data.

use cove_composition::Scope;
use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Color, State};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::{ButtonGroup, ButtonGroupItem};
use cove_ui_compose::primitives::data::chart::{
    ChartContent, ChartGuides, ChartMark, ChartOrientation, ChartPlot, ChartSeries,
    ChartSeriesStyle, ChartStacking, ChartStyle, ChartValueAxis, ChartXDomain,
};
use cove_ui_compose::primitives::feedback::{Badge, ProgressView, StatusTone};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::ThemeReader;
use cove_ui_compose::theme::styles::buttons::SecondaryButtonStyle;
use cove_ui_compose::theme::styles::buttons::button_style::ButtonStyleViewExt;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, DynamicField, LayoutDebugExt, TupleView, View};

use crate::gallery_content::catalog::GalleryOptions;

/// Metric tone references, matching `MetricCard`'s `SemanticColorRef` inputs.
pub const METRIC_HISTORY_FRAME_TIME: [f32; 12] = [
    22.0, 18.0, 26.0, 20.0, 25.0, 16.0, 21.0, 15.0, 24.0, 18.0, 21.0, 19.0,
];
pub const METRIC_HISTORY_ASSETS: [f32; 12] = [
    9.0, 12.0, 18.0, 12.0, 26.0, 22.0, 32.0, 38.0, 34.0, 52.0, 58.0, 64.0,
];

/// The card chrome every dashboard panel shares.
pub struct DashboardCard {
    content: AnyView,
}

impl DashboardCard {
    pub fn new<Content: View + 'static>(content: Content) -> Self {
        Self {
            content: AnyView::new(content),
        }
    }
}

impl View for DashboardCard {
    fn body(&self, _cx: &Scope) -> AnyView {
        let content = self.content.clone();
        AnyView::new(ThemeReader::new(move |theme| {
            let border = theme.colors.border.multiplied_alpha(0.55);
            let shadow = theme.elevation.low.color.unwrap_or(Color::CLEAR);
            let radius = theme.radius.xl;
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    16.0,
                    content.clone(),
                )
                .padding(20.0)
                .background_color(theme.colors.surface_raised)
                .corner_radius(radius)
                .border_color(border, 1.0)
                .shadow(shadow, 0.0, 2.0, 8.0),
            )
        }))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        Vec::new()
    }
}

/// A titled metric with an optional history chart.
pub struct MetricCard {
    pub title: String,
    pub value: String,
    pub unit: String,
    pub detail: String,
    pub history: Vec<f32>,
    pub tone: StatusTone,
}

impl MetricCard {
    pub fn new(
        title: impl Into<String>,
        value: impl Into<String>,
        unit: impl Into<String>,
        detail: impl Into<String>,
        history: Vec<f32>,
        tone: StatusTone,
    ) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            unit: unit.into(),
            detail: detail.into(),
            history,
            tone,
        }
    }

    /// The debug name the gallery uses for this metric.
    pub fn debug_name(&self) -> String {
        format!("dashboard-metric-{}", self.title)
    }
}

impl View for MetricCard {
    fn body(&self, _cx: &Scope) -> AnyView {
        // `TextScalerReader` widens the card with the text scaler; the minimum
        // stays at 180 points and the flex basis at max(200, scale(180)).
        let title = self.title.clone();
        let value = self.value.clone();
        let unit = self.unit.clone();
        let detail = self.detail.clone();
        let history = self.history.clone();
        let tone = self.tone;

        let history_view = if history.is_empty() {
            AnyView::new(DashboardSpacer)
        } else {
            let mut plot = ChartPlot::new(ChartContent::Cartesian {
                series: vec![ChartSeries::values(
                    history,
                    tone.color(),
                    ChartMark::Area,
                    "",
                    ChartValueAxis::Left,
                    ChartSeriesStyle {
                        line_width: Some(2.0),
                        ..ChartSeriesStyle::default()
                    },
                )],
                guides: ChartGuides::default(),
                x: ChartXDomain::Numeric,
            });
            // A metric sparkline: fixed 0...100 domain, no grid, 2-pt stroke.
            plot.style = ChartStyle::new(
                Some(0.0),
                Some(100.0),
                false,
                2.0,
                2.0,
                2.0,
                None,
                SemanticColorRef::divider(),
                ChartStacking::None,
                0.6,
                ChartOrientation::Vertical,
            );
            AnyView::new(plot.frame_height(42.0))
        };

        let card = AnyView::new(DashboardCard::new(AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Text::new(title).semantic_foreground(SemanticColorRef::on_surface_variant()),
                ),
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::FlexEnd,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    5.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            Text::new(value).semantic_foreground(SemanticColorRef::on_surface()),
                        ),
                        AnyView::new(
                            Text::new(unit)
                                .semantic_foreground(SemanticColorRef::on_surface_variant()),
                        ),
                    ])),
                )),
                history_view,
                AnyView::new(
                    Text::new(detail).semantic_foreground(SemanticColorRef::on_surface_muted()),
                ),
            ])),
        ))))
        .debug_name(&self.debug_name());

        AnyView::new(
            card.frame(None, None, Some(180.0), Some(184.0), None, None)
                .flex_with(1.0, 1.0, Some(200.0)),
        )
    }
}

/// One timeline row: a status dot, a title and a detail line.
struct ActivityRow {
    title: String,
    detail: &'static str,
    tone: StatusTone,
}

impl ActivityRow {
    fn new(title: impl Into<String>, detail: &'static str, tone: StatusTone) -> Self {
        Self {
            title: title.into(),
            detail,
            tone,
        }
    }
}

impl View for ActivityRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Badge::new("").tone(self.tone).shows_dot(true)),
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    5.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Text::new(self.title.clone())),
                        AnyView::new(
                            Text::new(self.detail)
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                    ])),
                )),
            ])),
        ))
    }
}

/// A flexible gap, matching Swift's `Spacer()`.
///
/// Swift's `Spacer()` is an empty view with no content of its own, so this
/// returns `EmptyView` directly. It used to route through a bare
/// `EmptyDashboardSpace` with an empty `impl View`, which made `kind()` fall
/// back to `Component` and panicked on mount with "this view is materialized
/// directly through ViewGraph" — the last instance of that defect class.
struct DashboardSpacer;

impl View for DashboardSpacer {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(cove_composition::EmptyView)
    }
}

/// The `introduction` page: a working composition of live state.
pub struct IntroductionStory {
    pub options: GalleryOptions,
    period: State<Option<String>>,
    is_paused: State<bool>,
    progress: State<f64>,
    completed: State<i32>,
    activity: State<String>,
}

impl IntroductionStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            period: State::new(Some("today".to_string())),
            is_paused: State::new(false),
            progress: State::new(0.64),
            completed: State::new(8),
            activity: State::new("Asset import started · just now".into()),
        }
    }

    /// Pause/resume the simulated import, recording the matching activity line.
    pub fn toggle_pause(&self) {
        let paused = !self.is_paused.get();
        self.is_paused.set(paused);
        self.activity.set(
            if paused {
                "Asset import paused · just now"
            } else {
                "Asset import resumed · just now"
            }
            .into(),
        );
    }

    /// Advance the import by 0.12, completing it at 1.0.
    pub fn advance(&self) {
        if self.progress.get() >= 1.0 {
            self.progress.set(0.0);
            self.activity
                .set("New asset import started · just now".into());
            return;
        }
        let next = (self.progress.get() + 0.12).min(1.0);
        self.progress.set(next);
        if next >= 1.0 {
            self.completed.update(|value| *value += 1);
            self.activity
                .set("Asset import completed · just now".into());
        } else {
            self.activity.set("Imported next batch · just now".into());
        }
    }

    /// The badge tone for the current import state.
    pub fn import_tone(&self) -> StatusTone {
        if self.progress.get() >= 1.0 {
            StatusTone::Success
        } else if self.is_paused.get() {
            StatusTone::Warning
        } else {
            StatusTone::Info
        }
    }

    /// The badge label for the current import state.
    pub fn import_label(&self) -> &'static str {
        if self.progress.get() >= 1.0 {
            "Complete"
        } else if self.is_paused.get() {
            "Paused"
        } else {
            "Running"
        }
    }

    /// The asset count, which depends on the selected period.
    pub fn asset_value(&self) -> &'static str {
        if self.period.get().as_deref() == Some("week") {
            "1,284"
        } else {
            "248"
        }
    }

    pub fn asset_detail(&self) -> &'static str {
        if self.period.get().as_deref() == Some("week") {
            "Imported this week"
        } else {
            "Imported today"
        }
    }

    /// Percent imported, rounded down as `Int(progress * 100)` does.
    pub fn percent_imported(&self) -> i32 {
        (self.progress.get() * 100.0) as i32
    }
}

impl View for IntroductionStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;

        // Period switcher: `today` / `week`, controlled by the story state.
        let period_selection = Binding::new(
            {
                let period = self.period.clone();
                move || period.get()
            },
            {
                let period = self.period.clone();
                move |value: Option<String>| period.set(value)
            },
        );
        let period_group = AnyView::new(
            ButtonGroup::try_new(
                vec![
                    ButtonGroupItem::new("today".to_string(), "Today"),
                    ButtonGroupItem::new("week".to_string(), "This week"),
                ],
                period_selection,
            )
            .expect("the gallery segments are static with unique ids")
            .enabled(enabled),
        );
        let period_group = AnyView::new(period_group.debug_name("dashboard-period"));

        let header = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    4.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Text::new("Your workspace, at a glance")),
                        AnyView::new(
                            Text::new("Sample project · local session")
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                    ])),
                )),
                period_group,
            ])),
        ));

        let engine = if self.is_paused.get() {
            "Paused"
        } else {
            "Ready"
        };
        let metrics = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(MetricCard::new(
                    "Engine",
                    engine,
                    "",
                    "Diffuse · native renderer",
                    Vec::new(),
                    StatusTone::Success,
                )),
                AnyView::new(MetricCard::new(
                    "Frame time",
                    "8.4",
                    "ms",
                    "Sample trace · 119 frames / second",
                    METRIC_HISTORY_FRAME_TIME.to_vec(),
                    StatusTone::Info,
                )),
                AnyView::new(MetricCard::new(
                    "Assets",
                    self.asset_value(),
                    "",
                    self.asset_detail(),
                    METRIC_HISTORY_ASSETS.to_vec(),
                    StatusTone::Info,
                )),
                AnyView::new(MetricCard::new(
                    "Completed",
                    self.completed.get().to_string(),
                    "tasks",
                    "All changes saved locally",
                    Vec::new(),
                    StatusTone::Success,
                )),
            ])),
        ));

        // Asset import card with its live progress, controls and activity line.
        let import = {
            let paused = self.is_paused.clone();
            let activity = self.activity.clone();
            let progress = self.progress.clone();
            let completed = self.completed.clone();
            let is_paused = self.is_paused.get();
            let value = self.progress.get();

            let pause = AnyView::new(
                Button::title(if is_paused { "Resume" } else { "Pause" }, {
                    let paused = paused.clone();
                    let activity = activity.clone();
                    move || {
                        let next = !paused.get();
                        paused.set(next);
                        activity.set(
                            if next {
                                "Asset import paused · just now"
                            } else {
                                "Asset import resumed · just now"
                            }
                            .into(),
                        );
                    }
                })
                .enabled(enabled && value < 1.0)
                .button_style(SecondaryButtonStyle)
                .debug_name("dashboard-pause"),
            );

            let advance = AnyView::new(
                Button::title(
                    if value >= 1.0 {
                        "Import again"
                    } else {
                        "Advance"
                    },
                    {
                        let activity = activity.clone();
                        let progress = progress.clone();
                        let completed = completed.clone();
                        move || {
                            if progress.get() >= 1.0 {
                                progress.set(0.0);
                                activity.set("New asset import started · just now".into());
                            } else {
                                let next = (progress.get() + 0.12).min(1.0);
                                progress.set(next);
                                if next >= 1.0 {
                                    completed.update(|count| *count += 1);
                                    activity.set("Asset import completed · just now".into());
                                } else {
                                    activity.set("Imported next batch · just now".into());
                                }
                            }
                        }
                    },
                )
                .enabled(enabled && !is_paused)
                .debug_name("dashboard-advance"),
            );

            AnyView::new(DashboardCard::new(AnyView::new(BoxView::new(
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
                            AnyView::new(Text::new("Asset import")),
                            AnyView::new(
                                Badge::new(self.import_label())
                                    .tone(self.import_tone())
                                    .shows_dot(true),
                            ),
                        ])),
                    )),
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(
                                cove_ui_compose::primitives::media::Icon::new(
                                    icons::file::folder(),
                                    28.0,
                                )
                                .semantic_foreground(SemanticColorRef::accent()),
                            ),
                            AnyView::new(BoxView::new(
                                FlexDirection::Column,
                                Align::FlexStart,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                5.0,
                                AnyView::new(TupleView::new(vec![
                                    AnyView::new(Text::new("Environment assets")),
                                    AnyView::new(
                                        Text::new("Textures, materials and scene geometry")
                                            .semantic_foreground(
                                                SemanticColorRef::on_surface_muted(),
                                            ),
                                    ),
                                ])),
                            )),
                        ])),
                    )),
                    AnyView::new(
                        AnyView::new(ProgressView::new(Some(value)).height(6.0))
                            .debug_name("dashboard-progress"),
                    ),
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        10.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(
                                Text::new(format!("{}% imported", self.percent_imported()))
                                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
                            ),
                            pause,
                            advance,
                        ])),
                    )),
                ])),
            ))))
        };

        let activity_card = AnyView::new(DashboardCard::new(AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Text::new("Recent activity")),
                AnyView::new(ActivityRow::new(
                    self.activity.get(),
                    "Environment assets",
                    StatusTone::Info,
                )),
                AnyView::new(Text::new("Scene saved")),
                AnyView::new(Text::new("Shader compiled")),
            ])),
        ))));

        let cards = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::Wrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(AnyView::new(import).flex_with(2.0, 1.0, Some(420.0))),
                AnyView::new(AnyView::new(activity_card).flex_with(1.0, 1.0, Some(260.0))),
            ])),
        ));

        let footer = AnyView::new(DashboardCard::new(AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    6.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(Text::new("Made from real components")),
                        AnyView::new(
                            Text::new(format!(
                                "Explore {} stories. Switch appearance, change density, use the keyboard, or reset this session.",
                                crate::gallery_content::catalog::GalleryCatalog::entries().len()
                            ))
                            .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                    ])),
                )),
                AnyView::new(
                    Badge::new("Light + dark")
                        .tone(StatusTone::Info)
                        .shows_dot(true),
                ),
            ])),
        ))));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                20.0,
                AnyView::new(TupleView::new(vec![header, metrics, cards, footer])),
            )
            .debug_name("dashboard-story"),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("period", &self.period),
            DynamicField::state("isPaused", &self.is_paused),
            DynamicField::state("progress", &self.progress),
            DynamicField::state("completed", &self.completed),
            DynamicField::state("activity", &self.activity),
        ]
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn import_advances_in_twelfths_and_completes() {
        let story = IntroductionStory::new(GalleryOptions::default());
        assert_eq!(story.progress.get(), 0.64);
        assert_eq!(story.percent_imported(), 64);
        assert_eq!(story.import_label(), "Running");
        assert_eq!(story.import_tone(), StatusTone::Info);

        story.advance();
        assert_eq!(story.progress.get(), 0.76);
        assert_eq!(story.activity.get(), "Imported next batch · just now");

        // Reaching 1.0 completes exactly once and bumps the task count.
        for _ in 0..2 {
            story.advance();
        }
        assert_eq!(story.progress.get(), 1.0);
        assert_eq!(story.completed.get(), 9);
        assert_eq!(story.activity.get(), "Asset import completed · just now");
        assert_eq!(story.import_label(), "Complete");
        assert_eq!(story.import_tone(), StatusTone::Success);
    }

    #[test]
    fn advancing_a_complete_import_restarts_it() {
        let story = IntroductionStory::new(GalleryOptions::default());
        while story.progress.get() < 1.0 {
            story.advance();
        }
        story.advance();
        assert_eq!(story.progress.get(), 0.0);
        assert_eq!(story.activity.get(), "New asset import started · just now");
        assert_eq!(story.import_label(), "Running");
    }

    #[test]
    fn pause_and_resume_record_activity() {
        let story = IntroductionStory::new(GalleryOptions::default());
        story.toggle_pause();
        assert!(story.is_paused.get());
        assert_eq!(story.activity.get(), "Asset import paused · just now");
        assert_eq!(story.import_label(), "Paused");
        assert_eq!(story.import_tone(), StatusTone::Warning);
        story.toggle_pause();
        assert!(!story.is_paused.get());
        assert_eq!(story.activity.get(), "Asset import resumed · just now");
    }

    #[test]
    fn assets_depend_on_the_selected_period() {
        let story = IntroductionStory::new(GalleryOptions::default());
        assert_eq!(story.asset_value(), "248");
        assert_eq!(story.asset_detail(), "Imported today");
        story.period.set(Some("week".into()));
        assert_eq!(story.asset_value(), "1,284");
        assert_eq!(story.asset_detail(), "Imported this week");
    }

    #[test]
    fn period_switcher_defaults_to_today_and_drives_the_asset_metric() {
        let story = IntroductionStory::new(GalleryOptions::default());
        assert_eq!(story.period.get(), Some("today".to_string()));
        assert_eq!(story.asset_value(), "248");
        assert_eq!(story.asset_detail(), "Imported today");
        // Selecting "week" is what the ButtonGroup writes back.
        story.period.set(Some("week".to_string()));
        assert_eq!(story.asset_value(), "1,284");
        assert_eq!(story.asset_detail(), "Imported this week");
        // Clearing the selection falls back to the today figures.
        story.period.set(None);
        assert_eq!(story.asset_value(), "248");
    }

    #[test]
    fn pause_blocks_advance_and_completion_unlocks_import_again() {
        let story = IntroductionStory::new(GalleryOptions::default());
        // While paused the activity line reports the pause and the label follows.
        story.toggle_pause();
        assert_eq!(story.activity.get(), "Asset import paused · just now");
        assert_eq!(story.import_label(), "Paused");
        story.toggle_pause();
        assert_eq!(story.import_label(), "Running");
        // Completing once is idempotent at the boundary: the label stays Complete.
        while story.progress.get() < 1.0 {
            story.advance();
        }
        assert_eq!(story.import_label(), "Complete");
        assert_eq!(story.import_tone(), StatusTone::Success);
        let completed = story.completed.get();
        story.advance();
        assert_eq!(
            story.completed.get(),
            completed,
            "restarting does not re-count"
        );
        assert_eq!(story.import_label(), "Running");
    }

    #[test]
    fn metric_debug_names_match_the_gallery() {
        let card = MetricCard::new("Frame time", "8.4", "ms", "", Vec::new(), StatusTone::Info);
        // Call the inherent accessor explicitly: `LayoutDebugExt::debug_name`
        // also applies to views and takes the name as an argument.
        assert_eq!(MetricCard::debug_name(&card), "dashboard-metric-Frame time");
    }
}
