//! Data stories: chart exploration and the adaptive table sample.
//!
//! Swift sources: `GalleryContent/Stories/Data/{ChartExploration,AdaptiveTables}.swift`.
//!
//! The chart story's state machine (focus a range, reset, bump the data
//! revision) is the part the gallery actually drives, so it lives here as a
//! type rather than being buried in a body closure.

use cove_composition::Scope;
use cove_core::State;
use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::{Button, Checkbox};
use cove_ui_compose::primitives::data::chart::{
    Chart, ChartAnnotation, ChartBand, ChartConfiguration, ChartDataTable, ChartDatum, ChartGuides,
    ChartHistogram, ChartLegendPlacementState, ChartMark, ChartOrientation, ChartPoint, ChartRange,
    ChartScaleKind, ChartSeries, ChartSeriesStyle, ChartSlice, ChartStacking, ChartStyle,
    ChartThreshold, ChartValueAxis, ChartXDomain, PieCenter,
};
use cove_ui_compose::primitives::data::data_table::table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout,
};
use cove_ui_compose::primitives::data::data_table::{DataTable, DataTableOptions};
use cove_ui_compose::primitives::data::lists::{Table, TableOptions};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The number of samples the exploration chart plots.
pub const CHART_SAMPLE_COUNT: usize = 60;
/// The zoom range the "Focus" button selects.
pub const CHART_FOCUS_RANGE: (f32, f32) = (20.0, 40.0);
/// Fixed vertical domain, so the pulsing signal never rescales the axis.
pub const CHART_VALUE_DOMAIN: (f32, f32) = (0.0, 45.0);

/// `ChartExplorationStory`: a signal you can zoom, brush and re-seed.
#[derive(Clone)]
pub struct ChartExplorationStory {
    /// Zoomed viewport, or `None` for the full domain.
    viewport: State<Option<ChartRange>>,
    /// Brushed selection, or `None`.
    selection: State<Option<ChartRange>>,
    /// Seeds the signal phase so "Update data" visibly changes the curve.
    revision: Rc<Cell<u32>>,
}

impl Default for ChartExplorationStory {
    fn default() -> Self {
        Self {
            viewport: State::new(None),
            selection: State::new(None),
            revision: Rc::new(Cell::new(0)),
        }
    }
}

impl ChartExplorationStory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn viewport(&self) -> Option<ChartRange> {
        self.viewport.get()
    }

    pub fn selection(&self) -> Option<ChartRange> {
        self.selection.get()
    }

    pub fn revision(&self) -> u32 {
        self.revision.get()
    }

    /// Focuses the interesting part of the signal.
    pub fn focus(&self) {
        self.viewport.set(Some(ChartRange::new(
            CHART_FOCUS_RANGE.0,
            CHART_FOCUS_RANGE.1,
        )));
    }

    /// Restores the full domain and clears the brush.
    pub fn reset(&self) {
        self.viewport.set(None);
        self.selection.set(None);
    }

    /// Re-seeds the signal so the chart animates to a new curve.
    pub fn update_data(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    pub fn set_selection(&self, selection: Option<ChartRange>) {
        self.selection.set(selection);
    }

    /// The plotted samples: a slow sine plus a faster cosine, phase-shifted by
    /// the revision so every update produces a visibly different curve.
    pub fn samples(&self) -> Vec<ChartDatum> {
        (0..CHART_SAMPLE_COUNT)
            .map(|index| {
                let x = index as f32;
                let y = 25.0
                    + (x * 0.2 + self.revision.get() as f32).sin() * 14.0
                    + (x * 0.7).cos() * 3.0;
                ChartDatum::new(x, y)
            })
            .collect()
    }

    /// The caption under the chart while a brush is active.
    pub fn brushed_caption(&self) -> Option<String> {
        self.selection
            .get()
            .map(|range| format!("Selected {:.1}–{:.1}", range.lower, range.upper))
    }

    /// The chart the section shows: an area signal on a fixed value domain, with
    /// the viewport, brush and data-change animation wired to this story's state.
    fn chart(&self) -> Chart {
        let series = vec![ChartSeries::points(
            self.samples(),
            SemanticColorRef::accent(),
            ChartMark::Area,
            "Signal",
            ChartValueAxis::Left,
            ChartSeriesStyle {
                line_width: Some(2.0),
                ..ChartSeriesStyle::default()
            },
        )];
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Explore signal".to_string();
        configuration.viewport.range = Some(self.viewport.binding());
        configuration.selection.range = Some(self.selection.binding());
        configuration.animation = Some(cove_core::scene::Animation::ease_in_out(0.45));
        Chart::new(
            series,
            ChartXDomain::Numeric,
            ChartGuides::default(),
            ChartStyle::new(
                Some(CHART_VALUE_DOMAIN.0),
                Some(CHART_VALUE_DOMAIN.1),
                true,
                2.0,
                2.0,
                4.0,
                None,
                SemanticColorRef::divider(),
                ChartStacking::None,
                0.6,
                ChartOrientation::Vertical,
            ),
            configuration,
        )
    }
}

impl View for ChartExplorationStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let focus = self.clone();
        let reset = self.clone();
        let update = self.clone();

        let controls = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Button::title("Focus", move || focus.focus()).debug_name("chart-zoom-focus"),
                ),
                AnyView::new(
                    Button::title("Reset", move || reset.reset()).debug_name("chart-zoom-reset"),
                ),
                AnyView::new(
                    Button::title("Update data", move || update.update_data())
                        .debug_name("chart-animate-data"),
                ),
            ])),
        ));

        let mut stack = vec![
            controls,
            AnyView::new(
                AnyView::new(self.chart())
                    .frame_height(200.0)
                    .debug_name("chart-exploration"),
            ),
        ];
        if let Some(caption) = self.brushed_caption() {
            stack.push(AnyView::new(
                AnyView::new(
                    Text::new(caption)
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                )
                .debug_name("chart-brushed-range"),
            ));
        }

        AnyView::new(StorySection::new(
            "Explore",
            "Scroll to zoom, drag to brush, Alt-drag to pan.",
            AnyView::new(TupleView::new(stack)),
        ))
    }

    fn tag(&self) -> &'static str {
        "ChartExplorationStory"
    }
}

/// `ChartStory`: the chart vocabulary, one section per mark family.
#[derive(Clone)]
pub struct ChartStory {
    stacked: State<bool>,
    percent: State<bool>,
    selection: State<Option<ChartPoint>>,
    hidden: State<std::collections::HashSet<String>>,
    exploration: ChartExplorationStory,
}

const CHART_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const CHART_REVENUE: [f32; 12] = [
    12.0, 19.0, 16.0, 28.0, 21.0, 35.0, 24.0, 31.0, 40.0, 34.0, 47.0, 42.0,
];
const CHART_SIGNUPS: [f32; 12] = [
    8.0, 12.0, 10.0, 17.0, 14.0, 20.0, 18.0, 23.0, 27.0, 24.0, 30.0, 28.0,
];
const CHART_LATENCY: [f32; 10] = [
    120.0, 180.0, 240.0, 310.0, 360.0, 420.0, 470.0, 520.0, 610.0, 700.0,
];
const CHART_ERRORS: [f32; 10] = [1.2, 1.6, 2.1, 2.0, 2.9, 3.4, 3.1, 4.2, 4.8, 5.6];
const CHART_GAINS: [f32; 6] = [18.0, -6.0, 22.0, -9.0, 14.0, -11.0];
const CHART_VISITS: [f32; 12] = [
    40.0, 95.0, 60.0, 180.0, 420.0, 1300.0, 900.0, 2600.0, 5400.0, 3100.0, 9000.0, 7200.0,
];
const CHART_STATES: [&str; 4] = [
    "Northern Territory",
    "Western Australia",
    "New South Wales",
    "Tasmania",
];
const CHART_SESSIONS: [f32; 4] = [42.0, 31.0, 18.0, 9.0];
const CHART_SESSIONS_LAST_YEAR: [f32; 4] = [38.0, 26.0, 21.0, 5.0];
const CHART_DURATIONS: [f32; 26] = [
    42.0, 55.0, 61.0, 63.0, 68.0, 70.0, 72.0, 74.0, 75.0, 77.0, 78.0, 79.0, 80.0, 81.0, 82.0, 84.0,
    85.0, 86.0, 88.0, 90.0, 92.0, 95.0, 98.0, 104.0, 112.0, 118.0,
];
const CHART_ONLINE: [f32; 6] = [14.0, 18.0, 12.0, 22.0, 19.0, 25.0];
const CHART_RETAIL: [f32; 6] = [9.0, 7.0, 11.0, 8.0, 12.0, 10.0];
const CHART_LOSSES: [f32; 6] = [-7.0, -12.0, -5.0, -8.0, -9.0, -6.0];

/// Two thousand samples across a narrow plot: the line keeps its envelope while
/// the stroke only covers what the pixels can resolve.
fn chart_signal() -> Vec<f32> {
    (0..2_000)
        .map(|index| {
            let t = index as f32 / 2_000.0;
            (t * 60.0).sin() * 12.0 + (t * 7.0).sin() * 6.0 + 20.0
        })
        .collect()
}

impl Default for ChartStory {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartStory {
    pub fn new() -> Self {
        Self {
            stacked: State::new(false),
            percent: State::new(false),
            selection: State::new(None),
            hidden: State::new(std::collections::HashSet::new()),
            exploration: ChartExplorationStory::new(),
        }
    }

    pub fn is_stacked(&self) -> bool {
        self.stacked.get()
    }

    pub fn selection(&self) -> Option<ChartPoint> {
        self.selection.get()
    }

    fn months() -> ChartXDomain {
        ChartXDomain::Categorical(CHART_MONTHS.iter().map(|month| month.to_string()).collect())
    }

    /// The first six month labels, for the six-point datasets.
    fn months_six() -> ChartXDomain {
        ChartXDomain::Categorical(
            CHART_MONTHS[..6]
                .iter()
                .map(|month| month.to_string())
                .collect(),
        )
    }

    fn base_style(stacking: ChartStacking) -> ChartStyle {
        ChartStyle::new(
            None,
            None,
            true,
            2.0,
            2.0,
            4.0,
            None,
            SemanticColorRef::divider(),
            stacking,
            0.6,
            ChartOrientation::Vertical,
        )
    }

    fn section(title: &'static str, detail: &'static str, body: AnyView) -> AnyView {
        let name = match title {
            "Trend" => "chart-trend",
            "Grouped and stacked bars" => "chart-bars",
            "Below the baseline" => "chart-stacking",
            "Dual axis" => "chart-dual-axis",
            "Log axis" => "chart-log-axis",
            "Histogram" => "chart-histogram",
            "Large series" => "chart-large-series",
            "Scatter" => "chart-scatter",
            "Donut" => "chart-donut",
            "Sparkline" => "chart-sparkline",
            "Horizontal bars" => "chart-horizontal-bars",
            "Data table" => "chart-data-table",
            other => panic!("no chart section named {other}"),
        };
        AnyView::new(StorySection::new(title, detail, body).debug_name(name))
    }

    fn trend(&self) -> AnyView {
        let mut guides = ChartGuides::default();
        let mut threshold = ChartThreshold::new(30.0);
        threshold.label = "Target".to_string();
        guides.thresholds.push(threshold);
        let mut band = ChartBand::new(10.0, 20.0);
        band.color = SemanticColorRef::info();
        band.label = "Slow".to_string();
        guides.bands.push(band);
        guides
            .annotations
            .push(ChartAnnotation::new("Launch", 0, 3));

        let mut configuration = ChartConfiguration::default();
        configuration.label = "Monthly revenue and signups".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.axis.y.title = "Revenue".to_string();
        configuration.selection.point = Some(self.selection.binding());
        configuration.legend.hidden_series = Some(self.hidden.binding());
        configuration.data_labels.format = Some(std::sync::Arc::new(|value: f32| {
            format!("{}", value.round() as i64)
        }));

        let series = vec![
            ChartSeries::values(
                CHART_REVENUE.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Area,
                "Revenue",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_SIGNUPS.to_vec(),
                SemanticColorRef::success(),
                ChartMark::Line,
                "Signups",
                ChartValueAxis::Left,
                ChartSeriesStyle::new(Some(2.0), Vec::new(), false, 3.5, 0.2, true),
            ),
        ];
        let chart = Chart::new(
            series,
            Self::months(),
            guides,
            Self::base_style(ChartStacking::None),
            configuration,
        );
        // What the pointer is over reads back in words, so a keyboard or a
        // screenshot tells the same story the callout does.
        let readout = match self.selection.get() {
            Some(point) => {
                let series = if point.series == 0 {
                    "Revenue"
                } else {
                    "Signups"
                };
                let month = CHART_MONTHS.get(point.index).copied().unwrap_or("?");
                format!("Selected: {series} · {month}")
            }
            None => "Nothing selected".to_string(),
        };
        Self::section(
            "Trend",
            "Area and line marks with a labelled target, a shaded corridor and \
             a pinned note. The value axis fits the data to round ticks, so \
             grid lines and labels agree. Hover or drag to inspect a value.",
            AnyView::new(TupleView::new(vec![
                AnyView::new(AnyView::new(chart).frame_height(200.0)),
                AnyView::new(
                    AnyView::new(
                        Text::new(readout)
                            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                    )
                    .debug_name("chart-trend-selection"),
                ),
            ])),
        )
    }

    fn bars(&self) -> AnyView {
        let stacking = if self.stacked.get() {
            ChartStacking::Stacked
        } else {
            ChartStacking::None
        };
        let stacked = self.stacked.binding();
        let toggle = Checkbox::labeled_from_boolean("Stacked", stacked).debug_name("chart-stacked");

        let mut configuration = ChartConfiguration::default();
        configuration.label = "Sales by channel".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.selection.point = Some(self.selection.binding());
        let series = vec![
            ChartSeries::values(
                CHART_ONLINE.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Bar,
                "Online",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_RETAIL.to_vec(),
                SemanticColorRef::warning(),
                ChartMark::Bar,
                "Retail",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
        ];
        let chart = Chart::new(
            series,
            Self::months(),
            ChartGuides::default(),
            Self::base_style(stacking),
            configuration,
        );
        Self::section(
            "Grouped and stacked bars",
            "Bar series sit side by side per category, or accumulate when \
             stacked. Category labels drop out on their own when the columns \
             get narrow.",
            AnyView::new(TupleView::new(vec![
                AnyView::new(toggle),
                AnyView::new(AnyView::new(chart).frame_height(180.0)),
            ])),
        )
    }

    fn baseline(&self) -> AnyView {
        let stacking = if self.percent.get() {
            ChartStacking::Percent
        } else {
            ChartStacking::Stacked
        };
        let percent = self.percent.binding();
        let toggle = Checkbox::labeled_from_boolean("Percent", percent).debug_name("chart-percent");

        let mut configuration = ChartConfiguration::default();
        configuration.label = "Monthly gain and loss".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.selection.point = Some(self.selection.binding());
        configuration.axis.y.title = if self.percent.get() {
            "Share".to_string()
        } else {
            "Amount".to_string()
        };
        let series = vec![
            ChartSeries::values(
                CHART_GAINS.to_vec(),
                SemanticColorRef::success(),
                ChartMark::Bar,
                "Gain",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_LOSSES.to_vec(),
                SemanticColorRef::error(),
                ChartMark::Bar,
                "Loss",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
        ];
        let chart = Chart::new(
            series,
            Self::months(),
            ChartGuides::default(),
            Self::base_style(stacking),
            configuration,
        );
        Self::section(
            "Below the baseline",
            "Values under zero stack downward instead of cancelling the surplus \
             above them, so one column carries both a gain and a loss. Percent \
             mode scales every column to 100, and the axis reads as shares.",
            AnyView::new(TupleView::new(vec![
                AnyView::new(toggle),
                AnyView::new(AnyView::new(chart).frame_height(180.0)),
            ])),
        )
    }

    fn dual_axis(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Latency and error rate".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.axis.y.title = "Sessions".to_string();
        configuration.axis.secondary.title = "Error %".to_string();
        configuration.axis.secondary.format =
            Some(std::sync::Arc::new(|value: f32| format!("{value:.1}")));
        let series = vec![
            ChartSeries::values(
                CHART_ONLINE.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Bar,
                "Sessions",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_ERRORS[..6].to_vec(),
                SemanticColorRef::error(),
                ChartMark::Line,
                "Error rate",
                ChartValueAxis::Right,
                ChartSeriesStyle::default(),
            ),
        ];
        let chart = Chart::new(
            series,
            Self::months_six(),
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Dual axis",
            "A series can read the right-hand axis, which fits only its own              data: a count and a rate share one plot without flattening each              other. Grid lines follow the left axis.",
            AnyView::new(AnyView::new(chart).frame_height(180.0)),
        )
    }

    fn log_axis(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Visits on a log axis".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.axis.y.kind = ChartScaleKind::Log;
        configuration.axis.y.title = "Visits".to_string();
        configuration.selection.point = Some(self.selection.binding());
        let series = vec![ChartSeries::values(
            CHART_VISITS.to_vec(),
            SemanticColorRef::accent_secondary(),
            ChartMark::Line,
            "Visits",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )];
        let chart = Chart::new(
            series,
            Self::months(),
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Log axis",
            "When values span orders of magnitude a log axis spaces them by \
             power of ten. Ticks land on 1, 2 and 5 x 10^n, so a small early \
             value still reads next to a large late one.",
            AnyView::new(AnyView::new(chart).frame_height(180.0)),
        )
    }

    fn histogram(&self) -> AnyView {
        let result = ChartHistogram::new(8, Some(ChartRange::new(40.0, 120.0))).build(
            &CHART_DURATIONS,
            SemanticColorRef::accent_secondary(),
            "Requests",
        );
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Latency distribution".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        configuration.axis.y.title = "Requests".to_string();
        configuration.axis.x.title = "Latency (ms)".to_string();
        // Binning is a data step, so the chart only draws bars: no grid, and
        // bars touch with no spacing.
        let mut style = Self::base_style(ChartStacking::None);
        style.shows_grid = false;
        style.bar_spacing = 0.0;
        let chart = Chart::new(
            vec![result.series],
            ChartXDomain::Categorical(result.labels),
            ChartGuides::default(),
            style,
            configuration,
        );
        Self::section(
            "Histogram",
            "Binning is a data step, so the chart only draws bars: \
             ChartHistogram counts the samples and labels each bin's edges. \
             Bars touch with no bar spacing.",
            AnyView::new(AnyView::new(chart).frame_height(160.0)),
        )
    }

    fn large_series(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "A two-thousand-sample signal".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        configuration.axis.y.title = "Level".to_string();
        configuration.selection.point = Some(self.selection.binding());
        let series = vec![ChartSeries::values(
            chart_signal(),
            SemanticColorRef::accent_secondary(),
            ChartMark::Line,
            "Signal",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )];
        let chart = Chart::new(
            series,
            ChartXDomain::Numeric,
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Large series",
            "Two thousand samples across a narrow plot: each pixel column keeps \
             its highest and lowest sample, so the envelope survives while the \
             stroke only covers what the pixels can resolve.",
            AnyView::new(AnyView::new(chart).frame_height(150.0)),
        )
    }

    fn scatter(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Error rate by latency".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        configuration.selection.point = Some(self.selection.binding());
        configuration.selection.value_text =
            std::sync::Arc::new(|value: f32| format!("{value:.1}%"));
        configuration.axis.x.title = "Latency (ms)".to_string();
        configuration.axis.y.title = "Error rate %".to_string();
        let points = CHART_LATENCY
            .iter()
            .zip(CHART_ERRORS.iter())
            .map(|(latency, errors)| ChartDatum::new(*latency, *errors))
            .collect();
        let series = vec![ChartSeries::points(
            points,
            SemanticColorRef::error(),
            ChartMark::Point,
            "Error rate",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )];
        let chart = Chart::new(
            series,
            ChartXDomain::Numeric,
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Scatter",
            "A numeric x domain puts every point on one value axis.",
            AnyView::new(AnyView::new(chart).frame_height(160.0)),
        )
    }

    fn donut(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Sessions by device".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.selection.point = Some(self.selection.binding());
        configuration.pie.sorts_by_value = true;
        configuration.pie.shows_labels = true;
        configuration.pie.center = Some(PieCenter::new("100", "Sessions"));
        let slices = vec![
            ChartSlice::new("Desktop", 58.0, None),
            ChartSlice::new("Mobile", 31.0, None),
            ChartSlice::new("Tablet", 8.0, None),
            ChartSlice::new("Other", 3.0, None),
        ];
        let mut style = Self::base_style(ChartStacking::None);
        style.inner_radius_ratio = 0.6;
        let chart = Chart::pie(slices, style, configuration);
        Self::section(
            "Donut",
            "Slices take the categorical palette, largest first.",
            AnyView::new(AnyView::new(chart).frame_height(220.0)),
        )
    }

    fn sparkline(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Revenue trend".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        configuration.axis.x.is_labeled = false;
        configuration.axis.y.is_labeled = false;
        let series = vec![ChartSeries::values(
            CHART_REVENUE.to_vec(),
            SemanticColorRef::success(),
            ChartMark::Area,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )];
        let chart = Chart::new(
            series,
            ChartXDomain::Categorical(Vec::new()),
            ChartGuides::default(),
            ChartStyle::sparkline(),
            configuration,
        );
        Self::section(
            "Sparkline",
            "A compact trend for cards and cells: no grid, no labels.",
            AnyView::new(AnyView::new(chart).frame_width(160.0).frame_height(40.0)),
        )
    }

    fn horizontal(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Sessions by state".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.selection.point = Some(self.selection.binding());
        configuration.axis.y.title = "Sessions".to_string();
        configuration.axis.x.title = "State".to_string();
        let mut style = Self::base_style(ChartStacking::None);
        style.orientation = ChartOrientation::Horizontal;
        let series = vec![
            ChartSeries::values(
                CHART_SESSIONS.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Bar,
                "Sessions",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_SESSIONS_LAST_YEAR.to_vec(),
                SemanticColorRef::accent_secondary(),
                ChartMark::Bar,
                "Last year",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
        ];
        let chart = Chart::new(
            series,
            ChartXDomain::Categorical(CHART_STATES.iter().map(|state| state.to_string()).collect()),
            ChartGuides::default(),
            style,
            configuration,
        );
        Self::section(
            "Horizontal bars",
            "Long category names have no room under a column, so the chart \
             turns: categories down the left, values along the bottom. \
             `axis.y` still configures the value axis — the title and ticks \
             just move to the bottom edge.",
            AnyView::new(AnyView::new(chart).frame_height(220.0)),
        )
    }

    fn data_table(&self) -> AnyView {
        let series = vec![
            ChartSeries::values(
                CHART_REVENUE.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Area,
                "Revenue",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_SIGNUPS.to_vec(),
                SemanticColorRef::success(),
                ChartMark::Line,
                "Signups",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
        ];
        let table = ChartDataTable::new(
            series,
            Self::months(),
            "Monthly revenue and signups",
            "Month",
            100,
            None,
        );
        Self::section(
            "Data table",
            "A plot is a picture of a table, and a picture is the one thing a \
             screen reader cannot describe. ChartDataTable is the table itself: \
             one row per month, one column per series, carrying the table, row \
             and cell roles that let a reader move by row and column.",
            AnyView::new(table),
        )
    }
}

impl View for ChartStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            24.0,
            AnyView::new(TupleView::new(vec![
                self.trend(),
                self.bars(),
                self.baseline(),
                self.dual_axis(),
                self.log_axis(),
                self.histogram(),
                self.large_series(),
                self.scatter(),
                self.donut(),
                self.sparkline(),
                self.horizontal(),
                AnyView::new(self.exploration.clone()),
                self.data_table(),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "ChartStory"
    }
}

/// The row count the adaptive-table sample lists.
pub const ADAPTIVE_TABLE_ROWS: usize = 30;

/// Builds the three authored columns the adaptive-table sample shows.
///
/// `quantity` and `category` are optional: a narrow table drops them rather than
/// clipping, which is what makes the sample "adaptive".
pub fn adaptive_table_columns() -> Vec<TableColumn<usize>> {
    let sized = |width: f32| {
        let mut layout = TableColumnLayout::default();
        layout.width = width;
        layout
    };

    let columns = vec![
        TableColumn::new("name", "Asset", move |row: &usize| {
            AnyView::new(Text::new(format!("Asset {row}")))
        })
        .layout(sized(160.0)),
        // `quantity` and `category` are optional: a narrow table drops them
        // rather than clipping, which is what makes the sample adaptive.
        TableColumn::new("quantity", "Quantity", move |row: &usize| {
            AnyView::new(Text::new(format!("{}", row * 7)))
        })
        .layout(sized(100.0))
        .visibility(ColumnVisibility::Optional),
        TableColumn::new("category", "Category", move |row: &usize| {
            AnyView::new(Text::new(if row.is_multiple_of(2) {
                "Texture"
            } else {
                "Material"
            }))
        })
        .layout(sized(130.0))
        .visibility(ColumnVisibility::Optional),
    ];
    columns
}

/// `AdaptiveTableSample`: the same columns rendered as either table flavour.
pub struct AdaptiveTableSample {
    pub options: GalleryOptions,
    /// `true` selects the editing `DataTable`; `false` the lighter `Table`.
    pub data: bool,
}

impl AdaptiveTableSample {
    pub fn new(options: GalleryOptions, data: bool) -> Self {
        Self { options, data }
    }

    pub fn row_count(&self) -> usize {
        ADAPTIVE_TABLE_ROWS
    }

    pub fn uses_data_table(&self) -> bool {
        self.data
    }
}

impl View for AdaptiveTableSample {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled = self.options.is_enabled;
        let rows: Vec<usize> = (0..ADAPTIVE_TABLE_ROWS).collect();
        // The same columns drive either flavour, which is the point of the
        // sample: only the data lifecycle and the weight differ.
        if self.data {
            let mut options = DataTableOptions::default();
            options.is_enabled = enabled;
            let table = DataTable::from_records(rows, |row: &usize| *row, adaptive_table_columns())
                .expect("the gallery table columns are static with unique ids")
                .options(options);
            return AnyView::new(table);
        }
        let mut options = TableOptions::default();
        options.is_enabled = enabled;
        let table = Table::new(rows, |row: &usize| *row, adaptive_table_columns())
            .expect("the gallery table columns are static with unique ids")
            .options(options);
        AnyView::new(table)
    }

    fn tag(&self) -> &'static str {
        "AdaptiveTableSample"
    }
}

/// Sorting a quantity column uses the numeric comparison, not the display text.
pub fn numeric_comparison(left: usize, right: usize) -> ComparisonResult {
    match (left * 7).cmp(&(right * 7)) {
        std::cmp::Ordering::Less => ComparisonResult::Ascending,
        std::cmp::Ordering::Greater => ComparisonResult::Descending,
        std::cmp::Ordering::Equal => ComparisonResult::Same,
    }
}
