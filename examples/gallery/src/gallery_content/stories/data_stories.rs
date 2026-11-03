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
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::data::chart::{
    Chart, ChartAnnotation, ChartBand, ChartConfiguration, ChartContent, ChartDataTable,
    ChartDatum, ChartGuides, ChartHistogram, ChartLegendPlacementState, ChartMark,
    ChartOrientation, ChartPlot, ChartPoint, ChartScaleKind, ChartSeries, ChartSeriesStyle,
    ChartSlice, ChartStacking, ChartStyle, ChartThreshold, ChartValueAxis, ChartXDomain, PieCenter,
};
use cove_ui_compose::primitives::data::data_table::table_column::{
    ColumnVisibility, ComparisonResult, TableColumn, TableColumnLayout,
};
use cove_ui_compose::primitives::data::data_table::{DataTable, DataTableOptions};
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
#[derive(Clone, Default)]
pub struct ChartExplorationStory {
    /// Zoomed viewport, or `None` for the full domain.
    viewport: Rc<Cell<Option<(f32, f32)>>>,
    /// Brushed selection, or `None`.
    selection: Rc<Cell<Option<(f32, f32)>>>,
    /// Seeds the signal phase so "Update data" visibly changes the curve.
    revision: Rc<Cell<u32>>,
}

impl ChartExplorationStory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn viewport(&self) -> Option<(f32, f32)> {
        self.viewport.get()
    }

    pub fn selection(&self) -> Option<(f32, f32)> {
        self.selection.get()
    }

    pub fn revision(&self) -> u32 {
        self.revision.get()
    }

    /// Focuses the interesting part of the signal.
    pub fn focus(&self) {
        self.viewport.set(Some(CHART_FOCUS_RANGE));
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

    pub fn set_selection(&self, selection: Option<(f32, f32)>) {
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
            .map(|(lower, upper)| format!("Selected {lower:.1}–{upper:.1}"))
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

        let mut plot = ChartPlot::new(ChartContent::Cartesian {
            series: vec![ChartSeries::values(
                self.samples().into_iter().map(|datum| datum.y).collect(),
                SemanticColorRef::accent(),
                ChartMark::Area,
                "Signal",
                ChartValueAxis::Left,
                ChartSeriesStyle {
                    line_width: Some(2.0),
                    ..ChartSeriesStyle::default()
                },
            )],
            guides: ChartGuides::default(),
            x: ChartXDomain::Numeric,
        });
        plot.style = ChartStyle::new(
            Some(CHART_VALUE_DOMAIN.0),
            Some(CHART_VALUE_DOMAIN.1),
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

        let mut stack = vec![
            controls,
            AnyView::new(AnyView::new(plot.frame_height(200.0)).debug_name("chart-exploration")),
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
    selection: State<Option<ChartPoint>>,
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

impl Default for ChartStory {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartStory {
    pub fn new() -> Self {
        Self {
            stacked: State::new(false),
            selection: State::new(None),
            exploration: ChartExplorationStory::new(),
        }
    }

    pub fn is_stacked(&self) -> bool {
        self.stacked.get()
    }

    pub fn toggle_stacked(&self) {
        self.stacked.update(|value| *value = !*value);
    }

    pub fn selection(&self) -> Option<ChartPoint> {
        self.selection.get()
    }

    fn months() -> ChartXDomain {
        ChartXDomain::Categorical(CHART_MONTHS.iter().map(|month| month.to_string()).collect())
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
        AnyView::new(
            StorySection::new(title, detail, body).debug_name(match title {
                "Trend" => "chart-trend",
                "Grouped and stacked bars" => "chart-bars",
                "Below the baseline" => "chart-baseline",
                "Dual axis" => "chart-dual-axis",
                "Log axis" => "chart-log",
                "Histogram" => "chart-histogram",
                "Scatter" => "chart-scatter",
                "Donut" => "chart-donut",
                "Sparkline" => "chart-sparkline",
                "Horizontal bars" => "chart-horizontal",
                _ => "chart-data-table",
            }),
        )
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
            .push(ChartAnnotation::new("Launch", 1, 3));

        let mut configuration = ChartConfiguration::default();
        configuration.label = "Monthly revenue and signups".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.axis.y.title = "Revenue".to_string();
        configuration.axis.x.title = "Month".to_string();
        configuration.selection.point = Some(self.selection.binding());

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
        Self::section(
            "Trend",
            "Area and line marks with a target, a shaded corridor and a pinned note.",
            AnyView::new(AnyView::new(chart).frame_height(200.0)),
        )
    }

    fn bars(&self) -> AnyView {
        let stacking = if self.stacked.get() {
            ChartStacking::Stacked
        } else {
            ChartStacking::None
        };
        let toggle = Button::title(if self.stacked.get() { "Group" } else { "Stack" }, {
            let this = self.clone();
            move || this.toggle_stacked()
        })
        .debug_name("chart-bars-toggle");

        let mut configuration = ChartConfiguration::default();
        configuration.label = "Revenue and signups by month".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        let series = vec![
            ChartSeries::values(
                CHART_REVENUE.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Bar,
                "Revenue",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_SIGNUPS.to_vec(),
                SemanticColorRef::accent_secondary(),
                ChartMark::Bar,
                "Signups",
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
            "Bars pair per month until the toggle stacks them.",
            AnyView::new(TupleView::new(vec![
                AnyView::new(toggle),
                AnyView::new(AnyView::new(chart).frame_height(200.0)),
            ])),
        )
    }

    fn baseline(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Weekly change".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        let series = vec![ChartSeries::values(
            CHART_GAINS.to_vec(),
            SemanticColorRef::success(),
            ChartMark::Bar,
            "Change",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )];
        let chart = Chart::new(
            series,
            ChartXDomain::Categorical(
                ["W1", "W2", "W3", "W4", "W5", "W6"]
                    .iter()
                    .map(|week| week.to_string())
                    .collect(),
            ),
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Below the baseline",
            "Negatives fall from zero instead of being clipped.",
            AnyView::new(AnyView::new(chart).frame_height(180.0)),
        )
    }

    fn dual_axis(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Latency and error rate".to_string();
        configuration.legend.placement = ChartLegendPlacementState::bottom();
        configuration.axis.y.title = "Latency (ms)".to_string();
        configuration.axis.secondary.title = "Error rate %".to_string();
        let series = vec![
            ChartSeries::values(
                CHART_LATENCY.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Area,
                "Latency",
                ChartValueAxis::Left,
                ChartSeriesStyle::default(),
            ),
            ChartSeries::values(
                CHART_ERRORS.to_vec(),
                SemanticColorRef::error(),
                ChartMark::Line,
                "Errors",
                ChartValueAxis::Right,
                ChartSeriesStyle::default(),
            ),
        ];
        let chart = Chart::new(
            series,
            ChartXDomain::Numeric,
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Dual axis",
            "Latency on the left, error rate on the right.",
            AnyView::new(AnyView::new(chart).frame_height(200.0)),
        )
    }

    fn log_axis(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Visits by month".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        configuration.axis.y.kind = ChartScaleKind::Log;
        configuration.axis.y.title = "Visits".to_string();
        let series = vec![ChartSeries::values(
            CHART_VISITS.to_vec(),
            SemanticColorRef::warning(),
            ChartMark::Bar,
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
            "Even spacing per power of ten; ticks land on 1, 2 and 5.",
            AnyView::new(AnyView::new(chart).frame_height(200.0)),
        )
    }

    fn histogram(&self) -> AnyView {
        let result = ChartHistogram::new(10, None).build(
            &CHART_DURATIONS,
            SemanticColorRef::accent_secondary(),
            "Duration",
        );
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Session duration".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
        let chart = Chart::new(
            vec![result.series],
            ChartXDomain::Categorical(result.labels),
            ChartGuides::default(),
            Self::base_style(ChartStacking::None),
            configuration,
        );
        Self::section(
            "Histogram",
            "Samples counted into even bins; tails are never folded in.",
            AnyView::new(AnyView::new(chart).frame_height(180.0)),
        )
    }

    fn scatter(&self) -> AnyView {
        let mut configuration = ChartConfiguration::default();
        configuration.label = "Error rate by latency".to_string();
        configuration.legend.placement = ChartLegendPlacementState::hidden();
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
            "Long category names read down the left edge.",
            AnyView::new(AnyView::new(chart).frame_height(200.0)),
        )
    }

    fn data_table(&self) -> AnyView {
        let series = vec![
            ChartSeries::values(
                CHART_REVENUE.to_vec(),
                SemanticColorRef::accent(),
                ChartMark::Line,
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
            12,
            None,
        );
        let rows: Vec<AnyView> = table
            .rows()
            .iter()
            .map(|row| {
                AnyView::new(Text::new(format!(
                    "{} · {}",
                    row.label,
                    row.cells.join(" · ")
                )))
            })
            .collect();
        Self::section(
            "Data table",
            "The plotted series as rows, for assistive tech.",
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(rows)),
            )),
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
                AnyView::new(self.exploration.clone()),
                self.scatter(),
                self.donut(),
                self.sparkline(),
                self.horizontal(),
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
        let mut options = DataTableOptions::default();
        options.is_enabled = enabled;
        let table = DataTable::from_records(rows, |row: &usize| *row, adaptive_table_columns())
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
