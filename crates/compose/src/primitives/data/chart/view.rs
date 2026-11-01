//! Translation of Primitives/Data/Chart/Chart.swift.
//!
//! Line, area, bar, scatter and pie/donut charts with labelled axes, an
//! optional legend and point selection. The model, geometry, scale, legend and
//! plot layers live in the sibling modules; this is the composing view.
//!
//! Behaviour checklist:
//! - the x domain belongs to the chart, so a bar series and a scatter series in
//!   one chart read the same axis
//! - a selection is only drawn while its series is visible
//! - the legend lists titled series, or every slice, in declaration order
//! - the callout for a cartesian point lists every visible series at that x
//! - the summary describes the selection when there is one, else each visible
//!   series' range
//! - a non-finite value is a gap, not a zero

use crate::Scope;
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::data::chart::{
    ChartCallout, ChartCalloutRow, ChartConfiguration, ChartContent, ChartGuides, ChartLegend,
    ChartLegendItem, ChartLegendPlacement, ChartPlot, ChartPlotInput, ChartPoint, ChartSeries,
    ChartSlice, ChartStyle, ChartXDomain,
};
use crate::{AnyView, BoxView, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, State};

/// Line, area, bar, scatter and pie/donut charts.
pub struct Chart {
    content: ChartContent,
    style: ChartStyle,
    configuration: ChartConfiguration,
    own_hidden: State<std::collections::HashSet<String>>,
}

impl Chart {
    /// Cartesian series over a shared x domain.
    pub fn new(
        series: Vec<ChartSeries>,
        x: ChartXDomain,
        guides: ChartGuides,
        style: ChartStyle,
        configuration: ChartConfiguration,
    ) -> Self {
        Self {
            content: ChartContent::Cartesian { series, guides, x },
            style,
            configuration,
            own_hidden: State::new(std::collections::HashSet::new()),
        }
    }

    /// Pie and donut slices.
    pub fn pie(
        slices: Vec<ChartSlice>,
        style: ChartStyle,
        configuration: ChartConfiguration,
    ) -> Self {
        Self {
            content: ChartContent::Pie(slices),
            style,
            configuration,
            own_hidden: State::new(std::collections::HashSet::new()),
        }
    }

    pub fn content(&self) -> &ChartContent {
        &self.content
    }

    pub fn style(&self) -> &ChartStyle {
        &self.style
    }

    pub fn configuration(&self) -> &ChartConfiguration {
        &self.configuration
    }

    fn hidden(&self) -> Binding<std::collections::HashSet<String>> {
        match &self.configuration.legend.hidden_series {
            Some(binding) => binding.clone(),
            None => self.own_hidden.binding(),
        }
    }

    /// Which content index a selection belongs to: its series, or its slice.
    pub fn visible_index(&self, point: ChartPoint) -> usize {
        match &self.content {
            ChartContent::Pie(_) => point.index,
            ChartContent::Cartesian { .. } => point.series,
        }
    }

    fn is_visible(&self, index: usize) -> bool {
        let hidden = self.hidden().get();
        match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .get(index)
                .map(|series| series.title.is_empty() || !hidden.contains(&series.title))
                .unwrap_or(false),
            ChartContent::Pie(slices) => slices
                .get(index)
                .map(|slice| slice.title.is_empty() || !hidden.contains(&slice.title))
                .unwrap_or(false),
        }
    }

    /// The legend's items, in declaration order.
    pub fn legend_items(&self) -> Vec<ChartLegendItem> {
        match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .iter()
                .filter(|series| !series.title.is_empty())
                .map(|series| ChartLegendItem {
                    title: series.title.clone(),
                    color: series.color.clone(),
                })
                .collect(),
            ChartContent::Pie(slices) => slices
                .iter()
                .enumerate()
                .map(|(index, slice)| ChartLegendItem {
                    title: slice.title.clone(),
                    color: slice.resolved_color(index),
                })
                .collect(),
        }
    }

    /// The format used for data labels, falling back to the selection text.
    fn value_format(&self) -> Rc<dyn Fn(f32) -> String> {
        match &self.configuration.data_labels.format {
            Some(format) => {
                let format = format.clone();
                Rc::new(move |value| format(value))
            }
            None => Rc::new(crate::primitives::data::chart::Selection::default_text),
        }
    }

    /// What the x axis calls this position.
    pub fn category_label(&self, index: usize) -> String {
        let format = self.value_format();
        match &self.content {
            ChartContent::Cartesian { series, x, .. } => match x {
                ChartXDomain::Categorical(labels) => labels
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("Point {}", index + 1)),
                ChartXDomain::Numeric => series
                    .first()
                    .and_then(|series| series.data.get(index))
                    .map(|datum| match &self.configuration.axis.x.format {
                        Some(format) => format(datum.x),
                        None => format(datum.x),
                    })
                    .unwrap_or_else(|| format!("Point {}", index + 1)),
            },
            ChartContent::Pie(_) => format!("Point {}", index + 1),
        }
    }

    /// The callout for one point, listing every visible series at that x.
    pub fn callout(&self, point: ChartPoint) -> Option<ChartCallout> {
        let format = self.value_format();
        match &self.content {
            ChartContent::Cartesian { series, .. } => {
                series.get(point.series)?.data.get(point.index)?;
                let rows: Vec<ChartCalloutRow> = series
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| self.is_visible(*index))
                    .filter_map(|(index, other)| {
                        let datum = other.data.get(point.index)?;
                        if !datum.y.is_finite() {
                            return None;
                        }
                        let name = if other.title.is_empty() {
                            format!("Series {}", index + 1)
                        } else {
                            other.title.clone()
                        };
                        Some(ChartCalloutRow {
                            title: name,
                            value: format(datum.y),
                            color: Some(other.color.clone()),
                        })
                    })
                    .collect();
                if rows.is_empty() {
                    return None;
                }
                Some(ChartCallout::new(
                    point,
                    self.category_label(point.index),
                    rows,
                ))
            }
            ChartContent::Pie(slices) => {
                let slice = slices.get(point.index)?;
                let total: f32 = slices.iter().map(|slice| slice.value.max(0.0)).sum();
                let mut detail = format(slice.value);
                if total > 0.0 {
                    detail.push_str(&format!(
                        " · {}%",
                        (slice.value / total * 100.0).round() as i64
                    ));
                }
                let title = if slice.title.is_empty() {
                    format!("Slice {}", point.index + 1)
                } else {
                    slice.title.clone()
                };
                Some(ChartCallout::new(
                    point,
                    title,
                    vec![ChartCalloutRow {
                        title: String::new(),
                        value: detail,
                        color: Some(slice.resolved_color(point.index)),
                    }],
                ))
            }
        }
    }

    /// The accessible summary: the selection's detail, else each visible
    /// series' range.
    pub fn summary(&self, selection: Option<ChartPoint>) -> (String, String) {
        let format = self.value_format();
        if let Some(selection) = selection
            && let Some(callout) = self.callout(selection)
        {
            let detail = callout
                .rows
                .iter()
                .map(|row| {
                    if row.title.is_empty() {
                        row.value.clone()
                    } else {
                        format!("{} {}", row.title, row.value)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            return (
                self.configuration.label.clone(),
                if callout.title.is_empty() {
                    detail
                } else {
                    format!("{}: {detail}", callout.title)
                },
            );
        }

        let parts: Vec<String> = match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .iter()
                .enumerate()
                .filter(|(index, series)| self.is_visible(*index) && !series.data.is_empty())
                .map(|(index, series)| {
                    let values: Vec<f32> = series
                        .data
                        .iter()
                        .map(|datum| datum.y)
                        .filter(|value| value.is_finite())
                        .collect();
                    let name = if series.title.is_empty() {
                        format!("Series {}", index + 1)
                    } else {
                        series.title.clone()
                    };
                    let low = values.iter().copied().fold(f32::INFINITY, f32::min);
                    let high = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    format!(
                        "{name} {} to {}",
                        format(if low.is_finite() { low } else { 0.0 }),
                        format(if high.is_finite() { high } else { 0.0 })
                    )
                })
                .collect(),
            ChartContent::Pie(slices) => slices
                .iter()
                .enumerate()
                .filter(|(index, _)| self.is_visible(*index))
                .map(|(_, slice)| format!("{} {}", slice.title, format(slice.value)))
                .collect(),
        };
        (
            self.configuration.label.clone(),
            if parts.is_empty() {
                "No data".to_string()
            } else {
                parts.join("; ")
            },
        )
    }
}

impl View for Chart {
    fn body(&self, _cx: &Scope) -> AnyView {
        let hidden = self.hidden();
        let visible: Vec<bool> = (0..self.content.count())
            .map(|index| self.is_visible(index))
            .collect();

        let requested = self
            .configuration
            .selection
            .point
            .as_ref()
            .and_then(|binding| binding.get());
        let selection = requested.filter(|point| {
            let index = self.visible_index(*point);
            visible.get(index).copied().unwrap_or(false)
        });

        let format = self.value_format();
        let summary = self.summary(selection);

        let mut plot = ChartPlot::new(self.content.clone());
        plot.visible = visible.clone();
        plot.style = self.style.clone();
        plot.axis = self.configuration.axis.clone();
        plot.pie = self.configuration.pie.clone();
        plot.format = format.clone();
        plot.selection = selection;
        plot.summary = summary;
        plot.callout = {
            let chart = self.clone_for_callout();
            Rc::new(move |point: ChartPoint| chart.callout(point))
        };
        plot.accessibility = self.configuration.accessibility.clone();
        plot.viewport = self.configuration.viewport.clone();
        plot.visible_range = self
            .configuration
            .viewport
            .range
            .as_ref()
            .and_then(|binding| binding.get());
        plot.brush = self.configuration.selection.range.clone();
        plot.brushed_range = self
            .configuration
            .selection
            .range
            .as_ref()
            .and_then(|binding| binding.get());
        plot.animation = self.configuration.animation.clone();
        plot.input = ChartPlotInput {
            is_enabled: self.configuration.selection.point.is_some(),
            select: {
                let binding = self.configuration.selection.point.clone();
                Rc::new(move |next: Option<ChartPoint>| {
                    if let Some(binding) = &binding
                        && binding.get() != next
                    {
                        binding.set(next);
                    }
                })
            },
        };

        let mut children = vec![AnyView::new(AnyView::new(plot).flex_with(
            1.0,
            1.0,
            Some(0.0),
        ))];
        if self.configuration.legend.placement.0 == ChartLegendPlacement::Bottom {
            children.push(AnyView::new(
                AnyView::new(ChartLegend {
                    items: self.legend_items(),
                    hidden,
                })
                .flex_with(0.0, 0.0, None),
            ));
        }

        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(TupleView::new(children)),
            ))
            .frame(None, None, Some(0.0), None, None, None),
        )
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        crate::dynamic_fields!(self; own_hidden)
    }
}

impl Chart {
    /// A clone the callout closure can own; it shares the plot state, so the
    /// closure and the view always agree.
    fn clone_for_callout(&self) -> ChartCalloutSource {
        ChartCalloutSource {
            content: self.content.clone(),
            configuration: self.configuration.clone(),
            hidden: self.hidden(),
        }
    }
}

/// The subset of a chart the callout closure needs, without the view's state.
#[derive(Clone)]
struct ChartCalloutSource {
    content: ChartContent,
    configuration: ChartConfiguration,
    hidden: Binding<std::collections::HashSet<String>>,
}

impl ChartCalloutSource {
    fn is_visible(&self, index: usize) -> bool {
        let hidden = self.hidden.get();
        match &self.content {
            ChartContent::Cartesian { series, .. } => series
                .get(index)
                .map(|series| series.title.is_empty() || !hidden.contains(&series.title))
                .unwrap_or(false),
            ChartContent::Pie(slices) => slices
                .get(index)
                .map(|slice| slice.title.is_empty() || !hidden.contains(&slice.title))
                .unwrap_or(false),
        }
    }

    fn value_format(&self) -> Rc<dyn Fn(f32) -> String> {
        match &self.configuration.data_labels.format {
            Some(format) => {
                let format = format.clone();
                Rc::new(move |value| format(value))
            }
            None => Rc::new(crate::primitives::data::chart::Selection::default_text),
        }
    }

    fn callout(&self, point: ChartPoint) -> Option<ChartCallout> {
        let format = self.value_format();
        match &self.content {
            ChartContent::Cartesian { series, x, .. } => {
                series.get(point.series)?.data.get(point.index)?;
                let rows: Vec<ChartCalloutRow> = series
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| self.is_visible(*index))
                    .filter_map(|(index, other)| {
                        let datum = other.data.get(point.index)?;
                        if !datum.y.is_finite() {
                            return None;
                        }
                        let name = if other.title.is_empty() {
                            format!("Series {}", index + 1)
                        } else {
                            other.title.clone()
                        };
                        Some(ChartCalloutRow {
                            title: name,
                            value: format(datum.y),
                            color: Some(other.color.clone()),
                        })
                    })
                    .collect();
                if rows.is_empty() {
                    return None;
                }
                let title = match x {
                    ChartXDomain::Categorical(labels) => labels
                        .get(point.index)
                        .cloned()
                        .unwrap_or_else(|| format!("Point {}", point.index + 1)),
                    ChartXDomain::Numeric => series
                        .first()
                        .and_then(|series| series.data.get(point.index))
                        .map(|datum| match &self.configuration.axis.x.format {
                            Some(format) => format(datum.x),
                            None => format(datum.x),
                        })
                        .unwrap_or_else(|| format!("Point {}", point.index + 1)),
                };
                Some(ChartCallout::new(point, title, rows))
            }
            ChartContent::Pie(slices) => {
                let slice = slices.get(point.index)?;
                let total: f32 = slices.iter().map(|slice| slice.value.max(0.0)).sum();
                let mut detail = format(slice.value);
                if total > 0.0 {
                    detail.push_str(&format!(
                        " · {}%",
                        (slice.value / total * 100.0).round() as i64
                    ));
                }
                let title = if slice.title.is_empty() {
                    format!("Slice {}", point.index + 1)
                } else {
                    slice.title.clone()
                };
                Some(ChartCallout::new(
                    point,
                    title,
                    vec![ChartCalloutRow {
                        title: String::new(),
                        value: detail,
                        color: Some(slice.resolved_color(point.index)),
                    }],
                ))
            }
        }
    }
}
