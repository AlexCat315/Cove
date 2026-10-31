//! Translation of `Primitives/Data/Chart/Coordinates.swift`.
//!
//! Everything is written once and projected through orientation helpers: a
//! mark has a position across the categories and one along the value axis, and
//! only `point(across, along)` decides which screen coordinate each lands on.
use cove_core::scene::{Point, Rect};

use super::axes::ChartValueAxes;
use super::model::{
    ChartDatum, ChartMark, ChartOrientation, ChartPoint, ChartSeries, ChartSlice, ChartValueAxis,
    ChartXDomain,
};
use super::scale::ChartScale;
use super::viewport::ChartWindow;

impl ChartOrientation {
    pub fn breadth(self, plot: Rect) -> f32 {
        if self == Self::Vertical {
            plot.width
        } else {
            plot.height
        }
    }
    pub fn length(self, plot: Rect) -> f32 {
        if self == Self::Vertical {
            plot.height
        } else {
            plot.width
        }
    }
    pub fn across_origin(self, plot: Rect) -> f32 {
        if self == Self::Vertical {
            plot.x
        } else {
            plot.y
        }
    }
    pub fn along(self, scale: &ChartScale, value: f32, plot: Rect) -> f32 {
        if self == Self::Vertical {
            scale.y(value, plot)
        } else {
            scale.x(value, plot)
        }
    }
    pub fn across(self, scale: &ChartScale, value: f32, plot: Rect) -> f32 {
        if self == Self::Vertical {
            scale.x(value, plot)
        } else {
            scale.y(value, plot)
        }
    }
    pub fn point(self, across: f32, along: f32) -> Point {
        if self == Self::Vertical {
            Point::new(across, along)
        } else {
            Point::new(along, across)
        }
    }
    pub fn axes(self, point: Point) -> (f32, f32) {
        if self == Self::Vertical {
            (point.x, point.y)
        } else {
            (point.y, point.x)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mark {
    pub series: usize,
    pub index: usize,
    pub value: f32,
    pub x: f32,
    pub y: f32,
    /// Screen coordinate along the value axis where the mark rests.
    pub base: f32,
    /// Bar rectangle in screen space; `None` for other marks.
    pub bar: Option<Rect>,
    pub is_outer_end: bool,
}

#[derive(Clone)]
pub struct ChartGeometry {
    pub plot: Rect,
    pub axes: ChartValueAxes,
    pub marks: Vec<Vec<Mark>>,
    pub has_bars: bool,
    pub sample_count: usize,
    pub is_scatter: bool,
    pub orientation: ChartOrientation,
    pub window: Option<ChartWindow>,
}

impl ChartGeometry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        series: &[ChartSeries],
        axes: ChartValueAxes,
        x_domain: &ChartXDomain,
        x_scale: Option<&ChartScale>,
        bar_spacing: f32,
        orientation: ChartOrientation,
        plot: Rect,
        window: Option<ChartWindow>,
    ) -> Self {
        let shown = axes.shown();
        let slot_count = axes.sample_count;
        let has_bars = axes.has_bars;
        let is_scatter = x_domain.is_numeric();
        let stacked = axes.is_stacked;
        let numeric_x = if x_domain.is_numeric() { x_scale } else { None };

        let bar_series: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|index| series[*index].mark == ChartMark::Bar)
            .collect();
        let groups = if stacked { 1 } else { bar_series.len().max(1) };
        let slot = window
            .as_ref()
            .and_then(|window| {
                if x_domain.is_numeric() {
                    None
                } else {
                    Some(
                        orientation.breadth(plot)
                            / (window.range.upper - window.range.lower).max(0.001),
                    )
                }
            })
            .or_else(|| {
                numeric_x.map(|scale| {
                    Self::numeric_slot(series, &shown, scale, plot, orientation, window.as_ref())
                })
            })
            .unwrap_or_else(|| orientation.breadth(plot) / slot_count.max(1) as f32);

        let category_gap = bar_spacing.max((slot * 0.25).min(24.0));
        let inner_gap = if groups > 1 { 2.0 } else { 0.0 };
        let thickness = (1.0_f32).max(
            (slot - category_gap - (groups.saturating_sub(1)) as f32 * inner_gap) / groups as f32,
        );
        let origin = orientation.across_origin(plot);

        let mut marks: Vec<Vec<Mark>> = vec![Vec::new(); series.len()];
        for &index in &shown {
            let item = &series[index];
            let stack = axes.stack(item.axis);
            let scale = axes.scale(item.axis);
            let group = if stacked {
                0
            } else {
                bar_series
                    .iter()
                    .position(|value| *value == index)
                    .unwrap_or(0)
            };
            let group_offset =
                (group as f32 - (groups.saturating_sub(1)) as f32 / 2.0) * (thickness + inner_gap);
            marks[index] = item
                .data
                .iter()
                .enumerate()
                .map(|(i, datum)| {
                    let center = if let Some(window) = &window {
                        window.position(
                            if x_domain.is_numeric() {
                                datum.x
                            } else {
                                i as f32
                            },
                            plot,
                            orientation,
                        )
                    } else if let Some(scale) = numeric_x {
                        orientation.across(scale, datum.x, plot)
                    } else if item.mark == ChartMark::Bar || stack.has_bars {
                        origin + (i as f32 + 0.5) * slot
                    } else {
                        Self::across_index(i, slot_count, false, plot, orientation)
                    };
                    let across = if item.mark == ChartMark::Bar {
                        center + group_offset
                    } else {
                        center
                    };
                    let x_visible = !x_domain.is_numeric()
                        || (datum.x.is_finite()
                            && (window.as_ref().map(|window| window.kind)
                                != Some(super::model::ChartScaleKind::Log)
                                || datum.x > 0.0));
                    if !datum.y.is_finite() || !x_visible {
                        return Mark {
                            series: index,
                            index: i,
                            value: datum.y,
                            x: f32::NAN,
                            y: f32::NAN,
                            base: f32::NAN,
                            bar: None,
                            is_outer_end: false,
                        };
                    }
                    let along = orientation.along(scale, stack.tops[index][i], plot);
                    let base = orientation.along(scale, stack.bases[index][i], plot);
                    let point = orientation.point(across, along);
                    let bar = if item.mark == ChartMark::Bar {
                        let a = orientation.point(across - thickness / 2.0, along.min(base));
                        let b = orientation.point(across + thickness / 2.0, along.max(base));
                        Some(Rect::new(
                            a.x.min(b.x),
                            a.y.min(b.y),
                            (b.x - a.x).abs().max(1.0),
                            (b.y - a.y).abs().max(1.0),
                        ))
                    } else {
                        None
                    };
                    Mark {
                        series: index,
                        index: i,
                        value: datum.y,
                        x: point.x,
                        y: point.y,
                        base,
                        bar,
                        is_outer_end: stack.is_outer_end(index, i, datum.y),
                    }
                })
                .collect();
        }

        Self {
            plot,
            axes,
            marks,
            has_bars,
            sample_count: slot_count,
            is_scatter,
            orientation,
            window,
        }
    }

    /// Bars on a numeric axis are as thick as the closest pair of x values.
    fn numeric_slot(
        series: &[ChartSeries],
        shown: &[usize],
        scale: &ChartScale,
        plot: Rect,
        orientation: ChartOrientation,
        window: Option<&ChartWindow>,
    ) -> f32 {
        let mut xs: Vec<f32> = shown
            .iter()
            .flat_map(|index| series[*index].data.iter().map(|datum| datum.x))
            .filter(|value| value.is_finite())
            .collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut gap = f32::MAX;
        if xs.len() > 1 {
            for i in 1..xs.len() {
                let a = window
                    .map(|window| window.position(xs[i], plot, orientation))
                    .unwrap_or_else(|| orientation.across(scale, xs[i], plot));
                let b = window
                    .map(|window| window.position(xs[i - 1], plot, orientation))
                    .unwrap_or_else(|| orientation.across(scale, xs[i - 1], plot));
                let delta = (a - b).abs();
                if delta > 0.001 {
                    gap = gap.min(delta);
                }
            }
        }
        if gap < f32::MAX {
            gap
        } else {
            orientation.breadth(plot) / xs.len().max(1) as f32
        }
    }

    pub fn across_index(
        index: usize,
        count: usize,
        has_bars: bool,
        plot: Rect,
        orientation: ChartOrientation,
    ) -> f32 {
        let origin = orientation.across_origin(plot);
        let length = orientation.breadth(plot);
        if has_bars {
            return origin + (index as f32 + 0.5) * length / count.max(1) as f32;
        }
        if count <= 1 {
            return origin + length / 2.0;
        }
        origin + index as f32 / (count - 1) as f32 * length
    }

    pub fn along_value(&self, value: f32, axis: ChartValueAxis) -> f32 {
        self.orientation
            .along(self.axes.scale(axis), value, self.plot)
    }

    pub fn across_mark(&self, mark: &Mark) -> f32 {
        self.orientation.axes(Point::new(mark.x, mark.y)).0
    }
    pub fn along_mark(&self, mark: &Mark) -> f32 {
        self.orientation.axes(Point::new(mark.x, mark.y)).1
    }
    pub fn base_point(&self, mark: &Mark) -> Point {
        self.orientation.point(self.across_mark(mark), mark.base)
    }

    pub fn across_at(&self, index: usize) -> Option<f32> {
        for marks in &self.marks {
            if let Some(mark) = marks.get(index) {
                let value = self.across_mark(mark);
                if value.is_finite() {
                    return Some(value);
                }
            }
        }
        None
    }

    /// Closest column, then the series whose value is closest along the value
    /// axis. A numeric axis uses true distance. Gaps take no part.
    pub fn nearest(&self, x: f32, y: f32) -> Option<Mark> {
        let origin = self.orientation.across_origin(self.plot);
        let end = origin + self.orientation.breadth(self.plot);
        let all: Vec<&Mark> = self
            .marks
            .iter()
            .flat_map(|marks| marks.iter())
            .filter(|mark| {
                mark.x.is_finite()
                    && mark.y.is_finite()
                    && self.across_mark(mark) >= origin
                    && self.across_mark(mark) <= end
            })
            .collect();
        if all.is_empty() {
            return None;
        }
        if self.is_scatter {
            return all
                .into_iter()
                .min_by(|a, b| {
                    let da = (a.x - x).hypot(a.y - y);
                    let db = (b.x - x).hypot(b.y - y);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned();
        }
        let (target_across, target_along) = self.orientation.axes(Point::new(x, y));
        let column = all
            .iter()
            .min_by(|a, b| {
                (self.across_mark(a) - target_across)
                    .abs()
                    .partial_cmp(&(self.across_mark(b) - target_across).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()?;
        let column_across = self.across_mark(column);
        all.into_iter()
            .filter(|mark| (self.across_mark(mark) - column_across).abs() < 0.5)
            .min_by(|a, b| {
                (self.along_mark(a) - target_along)
                    .abs()
                    .partial_cmp(&(self.along_mark(b) - target_along).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    pub fn mark(&self, point: ChartPoint) -> Option<&Mark> {
        self.marks
            .get(point.series)
            .and_then(|marks| marks.get(point.index))
    }

    /// Crop before downsampling so zooming reveals detail instead of keeping
    /// the full dataset's coarse buckets. Crossing segments retain both ends.
    pub fn visible_run(&self, run: &[Mark]) -> Vec<Mark> {
        let lower = self.orientation.across_origin(self.plot);
        let upper = lower + self.orientation.breadth(self.plot);
        let inside = |mark: &Mark| {
            let across = self.across_mark(mark);
            across >= lower && across <= upper
        };
        let mut result = Vec::new();
        for index in 0..run.len() {
            if inside(&run[index]) {
                result.push(run[index].clone());
                continue;
            }
            for adjacent in [index.wrapping_sub(1), index + 1] {
                if adjacent >= run.len() || adjacent == index {
                    continue;
                }
                let a = self.across_mark(&run[index]);
                let b = self.across_mark(&run[adjacent]);
                if a.min(b) <= upper && a.max(b) >= lower {
                    result.push(run[index].clone());
                    break;
                }
            }
        }
        result
    }

    /// Each pixel column across the categories keeps its lowest and highest
    /// value, in run order, so a spike cannot vanish between samples.
    pub fn decimated(marks: &[Mark], orientation: ChartOrientation, length: f32) -> Vec<Mark> {
        if marks.len() <= 2 || length < 1.0 {
            return marks.to_vec();
        }
        let columns = (length.ceil() as usize).max(1);
        if marks.len() <= columns * 2 {
            return marks.to_vec();
        }
        let bucket = (2).max((marks.len() as f32 / columns as f32).ceil() as usize);
        let value = |mark: &Mark| {
            if orientation == ChartOrientation::Vertical {
                mark.y
            } else {
                mark.x
            }
        };
        let mut result = Vec::with_capacity(columns * 2 + 2);
        result.push(marks[0].clone());
        let mut start = 1;
        while start < marks.len() - 1 {
            let end = (start + bucket).min(marks.len() - 1);
            let mut low = start;
            let mut high = start;
            for i in start..end {
                if value(&marks[i]) < value(&marks[low]) {
                    low = i;
                }
                if value(&marks[i]) > value(&marks[high]) {
                    high = i;
                }
            }
            result.push(marks[low.min(high)].clone());
            if low != high {
                result.push(marks[low.max(high)].clone());
            }
            start = end;
        }
        result.push(marks[marks.len() - 1].clone());
        result
    }
}

#[derive(Clone, Debug)]
pub struct PieWedge {
    pub index: usize,
    pub value: f32,
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Debug)]
pub struct PieGeometry {
    pub center: Point,
    pub radius: f32,
    pub inner_radius: f32,
    pub wedges: Vec<PieWedge>,
    pub total: f32,
}

impl PieGeometry {
    pub fn new(
        slices: &[ChartSlice],
        visible: &[bool],
        inner_ratio: f32,
        start_angle: f32,
        sorts_by_value: bool,
        plot: Rect,
    ) -> Self {
        let center = Point::new(plot.x + plot.width / 2.0, plot.y + plot.height / 2.0);
        let radius = (plot.width.min(plot.height) / 2.0 - 4.0).max(0.0);
        let inner_radius = radius * inner_ratio;
        let values: Vec<f32> = slices
            .iter()
            .enumerate()
            .map(|(index, slice)| {
                if visible.get(index).copied().unwrap_or(false) && slice.value.is_finite() {
                    slice.value.max(0.0)
                } else {
                    0.0
                }
            })
            .collect();
        let total: f32 = values.iter().sum();
        let mut order: Vec<usize> = values
            .iter()
            .enumerate()
            .filter(|(_, value)| **value > 0.0)
            .map(|(index, _)| index)
            .collect();
        if sorts_by_value {
            order.sort_by(|a, b| {
                values[*b]
                    .partial_cmp(&values[*a])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let mut angle = -std::f32::consts::PI / 2.0
            + if start_angle.is_finite() {
                start_angle
            } else {
                0.0
            };
        let mut wedges = Vec::new();
        if total > 0.0 {
            for index in order {
                let sweep = values[index] / total * 2.0 * std::f32::consts::PI;
                wedges.push(PieWedge {
                    index,
                    value: values[index],
                    start: angle,
                    end: angle + sweep,
                });
                angle += sweep;
            }
        }
        Self {
            center,
            radius,
            inner_radius,
            wedges,
            total,
        }
    }

    pub fn wedge_at(&self, x: f32, y: f32) -> Option<&PieWedge> {
        let dx = x - self.center.x;
        let dy = y - self.center.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > self.radius + 6.0 || distance < self.inner_radius - 2.0 {
            return None;
        }
        let mut angle = dy.atan2(dx);
        if angle < -std::f32::consts::PI / 2.0 {
            angle += 2.0 * std::f32::consts::PI;
        }
        self.wedges
            .iter()
            .find(|wedge| angle >= wedge.start && angle < wedge.end)
    }

    pub fn anchor(&self, wedge: &PieWedge) -> Point {
        let middle = (wedge.start + wedge.end) / 2.0;
        let radius = (self.radius + self.inner_radius) / 2.0;
        Point::new(
            self.center.x + middle.cos() * radius,
            self.center.y + middle.sin() * radius,
        )
    }

    pub fn ring_point(&self, wedge: &PieWedge) -> Point {
        let middle = (wedge.start + wedge.end) / 2.0;
        Point::new(
            self.center.x + middle.cos() * self.radius,
            self.center.y + middle.sin() * self.radius,
        )
    }

    pub fn label_anchor(&self, wedge: &PieWedge) -> Point {
        let middle = (wedge.start + wedge.end) / 2.0;
        let radius = self.radius + 8.0;
        Point::new(
            self.center.x + middle.cos() * radius,
            self.center.y + middle.sin() * radius,
        )
    }
}

/// Gaps are represented by non-finite values; this helper keeps the checks in
/// one place for series construction.
pub fn is_gap(datum: &ChartDatum) -> bool {
    !datum.y.is_finite()
}
