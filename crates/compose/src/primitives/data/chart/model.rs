//! Translation of `Primitives/Data/Chart/Model.swift`.
//!
//! Public chart vocabulary: data, series, slices, guides, style and
//! configuration. `ClosedRange<Float>` becomes [`ChartRange`] so the same
//! value can be stored in `cove_core::Binding` (which requires `Send + Sync`).
use cove_core::Binding;

use crate::theme::tokens::SemanticColorRef;

/// Inclusive numeric interval. Mirrors `ClosedRange<Float>` where the chart
/// uses it (domains, viewport ranges, brush ranges, histogram spans).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartRange {
    pub lower: f32,
    pub upper: f32,
}
impl ChartRange {
    pub fn new(lower: f32, upper: f32) -> Self {
        Self { lower, upper }
    }
    pub fn span(self) -> f32 {
        self.upper - self.lower
    }
    pub fn contains(self, value: f32) -> bool {
        value >= self.lower && value <= self.upper
    }
    pub fn clamped(self, value: f32) -> f32 {
        value.clamp(self.lower, self.upper)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartMark {
    Line,
    /// A line over a translucent fill down to the baseline (or the series below
    /// when stacked).
    Area,
    Bar,
    /// Unconnected points; with a numeric x domain this is a scatter plot.
    Point,
}

/// One plotted value and the x position it belongs to. A non-finite `y` is a
/// gap: the line breaks there instead of drawing a value never measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartDatum {
    pub x: f32,
    pub y: f32,
}
impl ChartDatum {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChartXDomain {
    /// Evenly spaced slots, one per datum, with an optional label each.
    Categorical(Vec<String>),
    /// Every datum's own `x`, shared across series and ticked on round values.
    Numeric,
}
impl ChartXDomain {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Numeric)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartValueAxis {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartScaleKind {
    Linear,
    /// Even spacing per power of ten; ticks land on 1, 2 and 5 x 10^n.
    /// Values at or below zero clamp to the bottom edge.
    Log,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartSeriesStyle {
    pub line_width: Option<f32>,
    pub dash: Vec<f32>,
    pub curve: bool,
    pub point_radius: f32,
    pub fill_opacity: f32,
    pub shows_values: bool,
}
impl Default for ChartSeriesStyle {
    fn default() -> Self {
        Self::new(None, Vec::new(), false, 3.5, 0.2, false)
    }
}
impl ChartSeriesStyle {
    pub fn new(
        line_width: Option<f32>,
        dash: Vec<f32>,
        curve: bool,
        point_radius: f32,
        fill_opacity: f32,
        shows_values: bool,
    ) -> Self {
        Self {
            line_width: line_width.map(|value| value.max(0.5)),
            dash: dash.into_iter().map(|value| value.max(0.0)).collect(),
            curve,
            point_radius: point_radius.max(0.5),
            fill_opacity: fill_opacity.clamp(0.0, 1.0),
            shows_values,
        }
    }
}

#[derive(Clone)]
pub struct ChartSeries {
    /// Plotted values, in x order.
    pub data: Vec<ChartDatum>,
    pub color: SemanticColorRef,
    pub mark: ChartMark,
    /// Legend and selection name. Empty keeps the series out of the legend.
    pub title: String,
    pub axis: ChartValueAxis,
    pub style: ChartSeriesStyle,
}
impl ChartSeries {
    /// A series on evenly spaced slots; each datum's x is its index.
    pub fn values(
        values: Vec<f32>,
        color: SemanticColorRef,
        mark: ChartMark,
        title: impl Into<String>,
        axis: ChartValueAxis,
        style: ChartSeriesStyle,
    ) -> Self {
        Self {
            data: values
                .into_iter()
                .enumerate()
                .map(|(index, value)| ChartDatum::new(index as f32, value))
                .collect(),
            color,
            mark,
            title: title.into(),
            axis,
            style,
        }
    }
    pub fn points(
        points: Vec<ChartDatum>,
        color: SemanticColorRef,
        mark: ChartMark,
        title: impl Into<String>,
        axis: ChartValueAxis,
        style: ChartSeriesStyle,
    ) -> Self {
        Self {
            data: points,
            color,
            mark,
            title: title.into(),
            axis,
            style,
        }
    }
    pub fn line(values: Vec<f32>) -> Self {
        Self::values(
            values,
            SemanticColorRef::accent(),
            ChartMark::Line,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )
    }
    pub fn area(values: Vec<f32>) -> Self {
        Self::values(
            values,
            SemanticColorRef::accent(),
            ChartMark::Area,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )
    }
    pub fn bar(values: Vec<f32>) -> Self {
        Self::values(
            values,
            SemanticColorRef::accent(),
            ChartMark::Bar,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )
    }
    /// Scatter series; `x` and `y` are paired by index and truncated to the
    /// shorter one.
    pub fn scatter(x: Vec<f32>, y: Vec<f32>) -> Self {
        let count = x.len().min(y.len());
        let points = (0..count)
            .map(|index| ChartDatum::new(x[index], y[index]))
            .collect();
        Self::points(
            points,
            SemanticColorRef::accent(),
            ChartMark::Point,
            "",
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        )
    }
}

/// One part of a pie or donut. Without a color, slices take the categorical
/// palette in order.
#[derive(Clone)]
pub struct ChartSlice {
    pub value: f32,
    pub title: String,
    pub color: Option<SemanticColorRef>,
}
impl ChartSlice {
    pub fn new(title: impl Into<String>, value: f32, color: Option<SemanticColorRef>) -> Self {
        Self {
            value,
            title: title.into(),
            color,
        }
    }
    /// Distinct hues first; `info` is a second blue in the default themes, so
    /// it comes last.
    pub fn palette() -> [SemanticColorRef; 6] {
        [
            SemanticColorRef::accent(),
            SemanticColorRef::success(),
            SemanticColorRef::warning(),
            SemanticColorRef::accent_secondary(),
            SemanticColorRef::error(),
            SemanticColorRef::info(),
        ]
    }
    pub fn resolved_color(&self, index: usize) -> SemanticColorRef {
        self.color.clone().unwrap_or_else(|| {
            let palette = Self::palette();
            palette[index % palette.len()].clone()
        })
    }
}

#[derive(Clone)]
pub struct ChartThreshold {
    pub value: f32,
    pub color: SemanticColorRef,
    /// Drawn at the right end of the rule. Empty draws no text.
    pub label: String,
    pub axis: ChartValueAxis,
}
impl ChartThreshold {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            color: SemanticColorRef::warning(),
            label: String::new(),
            axis: ChartValueAxis::Left,
        }
    }
}

/// A shaded range on the value axis: a target corridor, a healthy band.
#[derive(Clone)]
pub struct ChartBand {
    pub from: f32,
    pub to: f32,
    pub color: SemanticColorRef,
    pub label: String,
    pub axis: ChartValueAxis,
}
impl ChartBand {
    pub fn new(from: f32, to: f32) -> Self {
        Self {
            from: from.min(to),
            to: from.max(to),
            color: SemanticColorRef::accent(),
            label: String::new(),
            axis: ChartValueAxis::Left,
        }
    }
}

/// A short note pinned to one value, drawn as a pill above the mark.
#[derive(Clone)]
pub struct ChartAnnotation {
    pub text: String,
    pub series: usize,
    pub index: usize,
    pub color: SemanticColorRef,
}
impl ChartAnnotation {
    pub fn new(text: impl Into<String>, series: usize, index: usize) -> Self {
        Self {
            text: text.into(),
            series,
            index,
            color: SemanticColorRef::accent(),
        }
    }
}

#[derive(Clone)]
pub struct ChartMarker {
    pub index: usize,
    pub color: SemanticColorRef,
    pub width: f32,
}
impl ChartMarker {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            color: SemanticColorRef::accent(),
            width: 1.0,
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(1.0);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartStacking {
    None,
    /// Positives rise from zero, negatives fall from it.
    Stacked,
    /// Each index is scaled so its taller side reaches 100.
    Percent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartOrientation {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
pub struct ChartStyle {
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
    pub shows_grid: bool,
    pub line_width: f32,
    pub bar_spacing: f32,
    pub content_inset: f32,
    pub background: Option<SemanticColorRef>,
    pub grid_color: SemanticColorRef,
    pub stacking: ChartStacking,
    pub inner_radius_ratio: f32,
    pub orientation: ChartOrientation,
}
impl Default for ChartStyle {
    fn default() -> Self {
        Self::new(
            None,
            None,
            true,
            2.0,
            2.0,
            4.0,
            None,
            SemanticColorRef::divider(),
            ChartStacking::None,
            0.6,
            ChartOrientation::Vertical,
        )
    }
}
impl ChartStyle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        min_value: Option<f32>,
        max_value: Option<f32>,
        shows_grid: bool,
        line_width: f32,
        bar_spacing: f32,
        content_inset: f32,
        background: Option<SemanticColorRef>,
        grid_color: SemanticColorRef,
        stacking: ChartStacking,
        inner_radius_ratio: f32,
        orientation: ChartOrientation,
    ) -> Self {
        Self {
            min_value,
            max_value,
            shows_grid,
            line_width: line_width.max(0.5),
            bar_spacing: bar_spacing.max(0.0),
            content_inset: content_inset.max(0.0),
            background,
            grid_color,
            stacking,
            inner_radius_ratio: if inner_radius_ratio.is_finite() {
                inner_radius_ratio.clamp(0.0, 0.9)
            } else {
                0.0
            },
            orientation,
        }
    }
    /// Compact trend line: no grid, thin stroke, small inset. Pair with
    /// unlabeled axes.
    pub fn sparkline() -> Self {
        Self::new(
            None,
            None,
            false,
            1.5,
            2.0,
            3.0,
            None,
            SemanticColorRef::divider(),
            ChartStacking::None,
            0.6,
            ChartOrientation::Vertical,
        )
    }
}

/// A selected value: series index (or 0 for a pie) and value index (or slice).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChartPoint {
    pub series: usize,
    pub index: usize,
}
impl ChartPoint {
    pub fn new(series: usize, index: usize) -> Self {
        Self { series, index }
    }
}

#[derive(Clone)]
pub struct ChartCalloutRow {
    pub title: String,
    pub value: String,
    pub color: Option<SemanticColorRef>,
}
impl ChartCalloutRow {
    pub fn new(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            color: None,
        }
    }
}

#[derive(Clone)]
pub struct ChartCallout {
    pub point: ChartPoint,
    pub title: String,
    pub rows: Vec<ChartCalloutRow>,
}
impl ChartCallout {
    pub fn new(point: ChartPoint, title: impl Into<String>, rows: Vec<ChartCalloutRow>) -> Self {
        Self {
            point,
            title: title.into(),
            rows,
        }
    }
}

#[derive(Clone)]
pub struct ChartConfiguration {
    pub selection: Selection,
    pub legend: Legend,
    pub axis: Axis,
    pub data_labels: DataLabels,
    pub pie: Pie,
    pub accessibility: Accessibility,
    pub viewport: Viewport,
    pub animation: Option<cove_core::scene::Animation>,
    pub label: String,
}
impl Default for ChartConfiguration {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            legend: Legend::default(),
            axis: Axis::default(),
            data_labels: DataLabels::default(),
            pie: Pie::default(),
            accessibility: Accessibility::default(),
            viewport: Viewport::default(),
            animation: None,
            label: "Chart".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Selection {
    pub point: Option<Binding<Option<ChartPoint>>>,
    /// Numeric charts return x values; categorical charts zero-based indices.
    pub range: Option<Binding<Option<ChartRange>>>,
    pub value_text: std::sync::Arc<dyn Fn(f32) -> String + Send + Sync>,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            point: None,
            range: None,
            value_text: std::sync::Arc::new(Self::default_text),
        }
    }
}
impl Selection {
    /// Whole numbers without decimals, the rest to two places.
    pub fn default_text(value: f32) -> String {
        if !value.is_finite() {
            return String::new();
        }
        if value.round() == value && value.abs() < 1e9 {
            format!("{}", value as i64)
        } else {
            format!("{value:.2}")
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChartLegendPlacement {
    #[default]
    Hidden,
    Bottom,
}

#[derive(Clone, Default)]
pub struct Legend {
    pub placement: ChartLegendPlacementState,
    pub hidden_series: Option<Binding<std::collections::HashSet<String>>>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChartLegendPlacementState(pub ChartLegendPlacement);
impl ChartLegendPlacementState {
    pub fn hidden() -> Self {
        Self(ChartLegendPlacement::Hidden)
    }
    pub fn bottom() -> Self {
        Self(ChartLegendPlacement::Bottom)
    }
}

#[derive(Clone, Default)]
pub struct DataLabels {
    pub format: Option<std::sync::Arc<dyn Fn(f32) -> String + Send + Sync>>,
}

#[derive(Clone, Default)]
pub struct Pie {
    pub start_angle: f32,
    pub sorts_by_value: bool,
    pub shows_labels: bool,
    pub center: Option<PieCenter>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PieCenter {
    pub value: String,
    pub label: String,
}
impl PieCenter {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct Axis {
    pub x: XAxis,
    pub y: YAxis,
    pub secondary: YAxis,
}

#[derive(Clone)]
pub struct XAxis {
    pub title: String,
    pub tick_count: i32,
    pub is_labeled: bool,
    pub kind: ChartScaleKind,
    pub format: Option<std::sync::Arc<dyn Fn(f32) -> String + Send + Sync>>,
}
impl Default for XAxis {
    fn default() -> Self {
        Self {
            title: String::new(),
            tick_count: 5,
            is_labeled: true,
            kind: ChartScaleKind::Linear,
            format: None,
        }
    }
}

#[derive(Clone)]
pub struct YAxis {
    pub title: String,
    pub tick_count: i32,
    pub is_labeled: bool,
    pub kind: ChartScaleKind,
    pub format: Option<std::sync::Arc<dyn Fn(f32) -> String + Send + Sync>>,
}
impl Default for YAxis {
    fn default() -> Self {
        Self {
            title: String::new(),
            tick_count: 4,
            is_labeled: true,
            kind: ChartScaleKind::Linear,
            format: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accessibility {
    pub exposes_values: bool,
}
impl Default for Accessibility {
    fn default() -> Self {
        Self {
            exposes_values: true,
        }
    }
}

#[derive(Clone, Default)]
pub struct Viewport {
    /// Visible interval on the independent axis; `None` fits all data.
    pub range: Option<Binding<Option<ChartRange>>>,
    pub minimum_fraction: f32,
}
impl Viewport {
    pub fn new() -> Self {
        Self {
            range: None,
            minimum_fraction: 0.01,
        }
    }
}
