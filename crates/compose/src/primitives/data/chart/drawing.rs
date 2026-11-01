//! Translation of `Primitives/Data/Chart/Drawing.swift`.
//!
//! Draws one resolved chart into a draw list. Pure: everything it needs is
//! passed in, so repaint identity stays explicit. Text drawing is deferred to
//! the future Text host; `axis_label_layouts` returns the same gutter
//! positions the Swift renderer uses.
use std::collections::HashSet;

use cove_core::scene::{Color, DrawList, Point, Rect};

use crate::theme::Theme;

use super::axis::{ChartFrame, ChartText};
use super::coordinates::{ChartGeometry, Mark, PieGeometry};
use super::model::{
    ChartBand, ChartDatum, ChartMarker, ChartOrientation, ChartPoint, ChartSeries,
    ChartSeriesStyle, ChartSlice, ChartStyle, ChartThreshold, ChartValueAxis,
};
use super::viewport::ChartWindow;

/// One sampled point of a filled area: the curve value and the baseline it
/// fills down to.
#[derive(Clone, Copy)]
struct FillPoint {
    value: Point,
    base: Point,
}

pub struct ChartPaint<'a> {
    pub theme: &'a Theme,
    pub style: &'a ChartStyle,
    pub selection: Option<ChartPoint>,
    pub frame: &'a ChartFrame,
    pub text: &'a ChartText,
}

impl<'a> ChartPaint<'a> {
    pub fn background(&self, bounds: Rect, list: &mut DrawList) {
        if let Some(background) = &self.style.background {
            let _ = list.add_rect(bounds, background.resolve(self.theme));
        }
    }

    pub fn brush(
        &self,
        range: super::model::ChartRange,
        window: &ChartWindow,
        list: &mut DrawList,
    ) {
        if !range.lower.is_finite() || !range.upper.is_finite() {
            return;
        }
        let a = window.position(range.lower, self.frame.plot, self.frame.orientation);
        let b = window.position(range.upper, self.frame.plot, self.frame.orientation);
        if !a.is_finite() || !b.is_finite() {
            return;
        }
        let rect = if self.frame.orientation == ChartOrientation::Vertical {
            Rect::new(
                a.min(b),
                self.frame.plot.y,
                (b - a).abs(),
                self.frame.plot.height,
            )
        } else {
            Rect::new(
                self.frame.plot.x,
                a.min(b),
                self.frame.plot.width,
                (b - a).abs(),
            )
        };
        let _ = list.add_rect(rect, self.theme.colors.accent.multiplied_alpha(0.14));
        let _ = list.add_rounded_rect_stroke(rect, 0.0, 1.0, self.theme.colors.accent);
    }

    /// One rule per value-axis tick, so the grid reads the same numbers as the
    /// labels instead of dividing the domain into arbitrary fractions.
    pub fn grid(&self, list: &mut DrawList) {
        if !self.style.shows_grid || self.frame.value_ticks.is_empty() {
            return;
        }
        let plot = self.frame.plot;
        let color = self
            .style
            .grid_color
            .resolve(self.theme)
            .multiplied_alpha(0.55);
        for tick in &self.frame.value_ticks {
            match self.frame.orientation {
                ChartOrientation::Vertical => {
                    let _ = list.add_line(
                        Point::new(plot.x, tick.position),
                        Point::new(plot.x + plot.width, tick.position),
                        1.0,
                        color,
                    );
                }
                ChartOrientation::Horizontal => {
                    let _ = list.add_line(
                        Point::new(tick.position, plot.y),
                        Point::new(tick.position, plot.y + plot.height),
                        1.0,
                        color,
                    );
                }
            }
        }
        let _ = list.add_line(
            Point::new(plot.x, plot.y + plot.height),
            Point::new(plot.x + plot.width, plot.y + plot.height),
            1.0,
            self.style
                .grid_color
                .resolve(self.theme)
                .multiplied_alpha(0.9),
        );
    }

    /// Gutter placements for every tick label and axis title. The text host
    /// consumes these; the draw list itself cannot rasterize glyphs yet.
    pub fn axis_label_layouts(&self) -> Vec<(String, Point)> {
        let color_slot = ();
        color_slot;
        let mut result = Vec::new();
        let bounds = self.frame.bounds;
        let plot = self.frame.plot;
        let vertical = self.frame.orientation == ChartOrientation::Vertical;
        let size = |text: &str| self.text.size(text);
        for tick in self
            .frame
            .value_ticks
            .iter()
            .filter(|tick| !tick.label.is_empty())
        {
            let measured = size(&tick.label);
            if vertical {
                result.push((
                    tick.label.clone(),
                    Point::new(
                        bounds
                            .x
                            .max(plot.x - ChartFrame::LABEL_GAP - measured.width),
                        bounds
                            .y
                            .max(tick.position - measured.height / 2.0)
                            .min(bounds.y + bounds.height - measured.height),
                    ),
                ));
            } else {
                result.push((
                    tick.label.clone(),
                    Point::new(
                        bounds
                            .x
                            .max(tick.position - measured.width / 2.0)
                            .min(bounds.x + bounds.width - measured.width),
                        plot.y + plot.height + 4.0,
                    ),
                ));
            }
        }
        for tick in self
            .frame
            .category_ticks
            .iter()
            .filter(|tick| !tick.label.is_empty())
        {
            let measured = size(&tick.label);
            if vertical {
                result.push((
                    tick.label.clone(),
                    Point::new(
                        bounds
                            .x
                            .max(tick.position - measured.width / 2.0)
                            .min(bounds.x + bounds.width - measured.width),
                        plot.y + plot.height + 4.0,
                    ),
                ));
            } else {
                result.push((
                    tick.label.clone(),
                    Point::new(
                        bounds
                            .x
                            .max(plot.x - ChartFrame::LABEL_GAP - measured.width),
                        bounds
                            .y
                            .max(tick.position - measured.height / 2.0)
                            .min(bounds.y + bounds.height - measured.height),
                    ),
                ));
            }
        }
        for tick in self
            .frame
            .secondary_ticks
            .iter()
            .filter(|tick| !tick.label.is_empty())
        {
            let measured = size(&tick.label);
            if vertical {
                result.push((
                    tick.label.clone(),
                    Point::new(
                        (plot.x + plot.width + ChartFrame::LABEL_GAP)
                            .min(bounds.x + bounds.width - measured.width),
                        bounds
                            .y
                            .max(tick.position - measured.height / 2.0)
                            .min(bounds.y + bounds.height - measured.height),
                    ),
                ));
            } else {
                result.push((
                    tick.label.clone(),
                    Point::new(
                        bounds
                            .x
                            .max(tick.position - measured.width / 2.0)
                            .min(bounds.x + bounds.width - measured.width),
                        bounds.y,
                    ),
                ));
            }
        }
        if !self.frame.value_title.is_empty() {
            let measured = size(&self.frame.value_title);
            result.push((
                self.frame.value_title.clone(),
                if vertical {
                    Point::new(bounds.x, bounds.y)
                } else {
                    Point::new(
                        plot.x + (plot.width - measured.width) / 2.0,
                        bounds.y + bounds.height - measured.height,
                    )
                },
            ));
        }
        if !self.frame.secondary_title.is_empty() {
            let measured = size(&self.frame.secondary_title);
            result.push((
                self.frame.secondary_title.clone(),
                Point::new(bounds.x + bounds.width - measured.width, bounds.y),
            ));
        }
        if !self.frame.category_title.is_empty() {
            let measured = size(&self.frame.category_title);
            result.push((
                self.frame.category_title.clone(),
                if vertical {
                    Point::new(
                        plot.x + (plot.width - measured.width) / 2.0,
                        bounds.y + bounds.height - measured.height,
                    )
                } else {
                    Point::new(bounds.x, bounds.y)
                },
            ));
        }
        result
    }

    pub fn thresholds(
        &self,
        thresholds: &[ChartThreshold],
        geometry: &ChartGeometry,
        list: &mut DrawList,
    ) {
        let plot = geometry.plot;
        for threshold in thresholds.iter().filter(|item| item.value.is_finite()) {
            let at = geometry.along_value(threshold.value, threshold.axis);
            let color = threshold.color.resolve(self.theme).multiplied_alpha(0.8);
            match geometry.orientation {
                ChartOrientation::Vertical => {
                    let _ = list.add_line(
                        Point::new(plot.x, at),
                        Point::new(plot.x + plot.width, at),
                        1.0,
                        color,
                    );
                }
                ChartOrientation::Horizontal => {
                    let _ = list.add_line(
                        Point::new(at, plot.y),
                        Point::new(at, plot.y + plot.height),
                        1.0,
                        color,
                    );
                }
            }
        }
    }

    /// A shaded corridor, under the grid so the grid stays the top rule.
    pub fn bands(&self, bands: &[ChartBand], geometry: &ChartGeometry, list: &mut DrawList) {
        let plot = geometry.plot;
        for band in bands
            .iter()
            .filter(|band| band.from.is_finite() && band.to.is_finite())
        {
            let a = geometry.along_value(band.to, band.axis);
            let b = geometry.along_value(band.from, band.axis);
            let low = a.min(b);
            let high = a.max(b);
            let rect = if geometry.orientation == ChartOrientation::Vertical {
                Rect::new(plot.x, low, plot.width, high - low)
            } else {
                Rect::new(low, plot.y, high - low, plot.height)
            };
            if rect.width <= 0.5 || rect.height <= 0.5 {
                continue;
            }
            let _ = list.add_rect(rect, band.color.resolve(self.theme).multiplied_alpha(0.12));
        }
    }

    pub fn pie_leaders(&self, slices: &[ChartSlice], geometry: &PieGeometry, list: &mut DrawList) {
        if geometry.radius <= 2.0 {
            return;
        }
        let color = self.theme.colors.on_surface_muted.multiplied_alpha(0.4);
        for wedge in geometry.wedges.iter().filter(|wedge| {
            slices
                .get(wedge.index)
                .map(|slice| !slice.title.is_empty())
                .unwrap_or(false)
        }) {
            let from = geometry.ring_point(wedge);
            let to = geometry.label_anchor(wedge);
            let _ = list.add_line(from, to, 1.0, color);
        }
    }

    pub fn series(&self, series: &[ChartSeries], geometry: &ChartGeometry, list: &mut DrawList) {
        // Fills first, so lines and points of every series stay on top.
        for (index, item) in series
            .iter()
            .enumerate()
            .filter(|(_, item)| item.mark == super::model::ChartMark::Area)
        {
            self.area(
                &geometry.marks[index],
                item.color.resolve(self.theme),
                &item.style,
                geometry,
                list,
            );
        }
        for (index, item) in series
            .iter()
            .enumerate()
            .filter(|(_, item)| item.mark == super::model::ChartMark::Bar)
        {
            self.bars(&geometry.marks[index], item.color.resolve(self.theme), list);
        }
        for (index, item) in series.iter().enumerate().filter(|(_, item)| {
            matches!(
                item.mark,
                super::model::ChartMark::Line | super::model::ChartMark::Area
            )
        }) {
            self.line(
                &geometry.marks[index],
                item.color.resolve(self.theme),
                &item.style,
                geometry,
                list,
            );
        }
        for (index, item) in series
            .iter()
            .enumerate()
            .filter(|(_, item)| item.mark == super::model::ChartMark::Point)
        {
            let mut drawn = HashSet::new();
            for mark in geometry.marks[index]
                .iter()
                .filter(|mark| mark.y.is_finite())
            {
                if !drawn.insert(Self::pixel(mark.x, mark.y)) {
                    continue;
                }
                self.dot(
                    mark.x,
                    mark.y,
                    item.style.point_radius,
                    item.color.resolve(self.theme).multiplied_alpha(0.85),
                    list,
                );
            }
        }
    }

    pub fn markers(&self, markers: &[ChartMarker], geometry: &ChartGeometry, list: &mut DrawList) {
        if geometry.sample_count == 0 {
            return;
        }
        let plot = geometry.plot;
        for marker in markers {
            let index = marker.index.min(geometry.sample_count - 1);
            let Some(at) = geometry.across_at(index) else {
                continue;
            };
            match geometry.orientation {
                ChartOrientation::Vertical => {
                    let _ = list.add_line(
                        Point::new(at, plot.y),
                        Point::new(at, plot.y + plot.height),
                        marker.width,
                        marker.color.resolve(self.theme),
                    );
                }
                ChartOrientation::Horizontal => {
                    let _ = list.add_line(
                        Point::new(plot.x, at),
                        Point::new(plot.x + plot.width, at),
                        marker.width,
                        marker.color.resolve(self.theme),
                    );
                }
            }
        }
    }

    /// A rule through the selected column and a ringed dot on the value.
    pub fn selection(&self, series: &[ChartSeries], geometry: &ChartGeometry, list: &mut DrawList) {
        let Some(selection) = self.selection else {
            return;
        };
        let Some(mark) = geometry.mark(selection) else {
            return;
        };
        let plot = geometry.plot;
        let rule = self.theme.colors.on_surface_muted.multiplied_alpha(0.35);
        let at = geometry.across_mark(mark);
        match geometry.orientation {
            ChartOrientation::Vertical => {
                let _ = list.add_line(
                    Point::new(at, plot.y),
                    Point::new(at, plot.y + plot.height),
                    1.0,
                    rule,
                );
            }
            ChartOrientation::Horizontal => {
                let _ = list.add_line(
                    Point::new(plot.x, at),
                    Point::new(plot.x + plot.width, at),
                    1.0,
                    rule,
                );
            }
        }
        if mark.bar.is_some() || !mark.y.is_finite() {
            return;
        }
        self.dot(mark.x, mark.y, 6.0, self.theme.colors.surface, list);
        self.dot(
            mark.x,
            mark.y,
            4.0,
            series[mark.series].color.resolve(self.theme),
            list,
        );
    }

    pub fn pie(&self, slices: &[ChartSlice], geometry: &PieGeometry, list: &mut DrawList) {
        if geometry.radius <= 2.0 {
            return;
        }
        let gap = if geometry.wedges.len() > 1 { 1.5 } else { 0.0 };
        for wedge in &geometry.wedges {
            let selected = self
                .selection
                .map(|selection| selection.index == wedge.index)
                .unwrap_or(false);
            let dimmed = self.selection.is_some() && !selected;
            let mut color = slices[wedge.index]
                .resolved_color(wedge.index)
                .resolve(self.theme);
            if dimmed {
                color = color.multiplied_alpha(0.55);
            }
            self.ring(
                geometry,
                wedge,
                geometry.radius + if selected { 4.0 } else { 0.0 },
                gap,
                color,
                list,
            );
        }
    }

    fn line(
        &self,
        marks: &[Mark],
        color: Color,
        series_style: &ChartSeriesStyle,
        geometry: &ChartGeometry,
        list: &mut DrawList,
    ) {
        let width = series_style.line_width.unwrap_or(self.style.line_width);
        let length = geometry.orientation.breadth(geometry.plot);
        for run in Self::runs(marks) {
            let points = self.sampled(
                &ChartGeometry::decimated(
                    &geometry.visible_run(&run),
                    geometry.orientation,
                    length,
                ),
                series_style.curve,
                geometry,
            );
            self.stroke(&points, color, width, &series_style.dash, list);
        }
    }

    fn area(
        &self,
        marks: &[Mark],
        color: Color,
        series_style: &ChartSeriesStyle,
        geometry: &ChartGeometry,
        list: &mut DrawList,
    ) {
        let fill = color.multiplied_alpha(series_style.fill_opacity);
        let length = geometry.orientation.breadth(geometry.plot);
        for run in Self::runs(marks) {
            let points = self.sampled_area(
                &ChartGeometry::decimated(
                    &geometry.visible_run(&run),
                    geometry.orientation,
                    length,
                ),
                series_style.curve,
                geometry,
            );
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let _ = list.add_triangle(a.value, b.value, b.base, fill);
                let _ = list.add_triangle(a.value, b.base, a.base, fill);
            }
        }
    }

    /// Contiguous stretches of measured values. A gap ends a run.
    fn runs(marks: &[Mark]) -> Vec<Vec<Mark>> {
        let mut runs = Vec::new();
        let mut current = Vec::new();
        for mark in marks {
            if mark.y.is_finite() {
                current.push(mark.clone());
            } else if !current.is_empty() {
                runs.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            runs.push(current);
        }
        runs
    }

    /// Emits a polyline, split into on/off runs when a dash pattern is set.
    fn stroke(
        &self,
        points: &[Point],
        color: Color,
        width: f32,
        dash: &[f32],
        list: &mut DrawList,
    ) {
        if points.len() <= 1 {
            if let Some(point) = points.first() {
                self.dot(point.x, point.y, width, color, list);
            }
            return;
        }
        let pattern: Vec<f32> = dash.iter().copied().filter(|value| *value > 0.0).collect();
        if pattern.len() < 2 || pattern.iter().sum::<f32>() <= 0.5 {
            for pair in points.windows(2) {
                let _ = list.add_line(pair[0], pair[1], width, color);
            }
            return;
        }
        let mut index = 0;
        let mut remaining = pattern[0];
        let mut is_on = true;
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let length = dx.hypot(dy);
            if length <= 0.0001 {
                continue;
            }
            let mut travelled = 0.0;
            while travelled < length - 0.0001 {
                let step = remaining.min(length - travelled);
                if is_on && step > 0.0 {
                    let t0 = travelled / length;
                    let t1 = (travelled + step) / length;
                    let _ = list.add_line(
                        Point::new(a.x + dx * t0, a.y + dy * t0),
                        Point::new(a.x + dx * t1, a.y + dy * t1),
                        width,
                        color,
                    );
                }
                travelled += step;
                remaining -= step;
                if remaining <= 0.0001 {
                    index = (index + 1) % pattern.len();
                    remaining = pattern[index];
                    is_on = !is_on;
                }
            }
        }
    }

    fn sampled(&self, marks: &[Mark], curved: bool, geometry: &ChartGeometry) -> Vec<Point> {
        let orientation = geometry.orientation;
        let logical: Vec<Point> = marks
            .iter()
            .map(|mark| {
                let (across, along) = orientation.axes(Point::new(mark.x, mark.y));
                Point::new(across, along)
            })
            .collect();
        if !curved {
            return logical
                .iter()
                .map(|point| orientation.point(point.x, point.y))
                .collect();
        }
        Self::curve(&logical)
            .iter()
            .map(|point| orientation.point(point.x, point.y))
            .collect()
    }

    fn curve(points: &[Point]) -> Vec<Point> {
        if points.len() <= 2 || !Self::is_increasing(points) {
            return points.to_vec();
        }
        let tangents = Self::tangents(points);
        let mut result = vec![points[0]];
        for i in 0..points.len() - 1 {
            let x0 = points[i].x;
            let x1 = points[i + 1].x;
            let h = x1 - x0;
            let y0 = points[i].y;
            let y1 = points[i + 1].y;
            for step in 1..=Self::CURVE_STEPS {
                let t = step as f32 / Self::CURVE_STEPS as f32;
                let t2 = t * t;
                let t3 = t2 * t;
                let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                    + (t3 - 2.0 * t2 + t) * h * tangents[i]
                    + (-2.0 * t3 + 3.0 * t2) * y1
                    + (t3 - t2) * h * tangents[i + 1];
                result.push(Point::new(x0 + h * t, y));
            }
        }
        result
    }

    fn sampled_area(
        &self,
        marks: &[Mark],
        curved: bool,
        geometry: &ChartGeometry,
    ) -> Vec<FillPoint> {
        let points = self.sampled(marks, curved, geometry);
        if points.len() <= marks.len() {
            return marks
                .iter()
                .zip(points)
                .map(|(mark, point)| FillPoint {
                    value: point,
                    base: geometry.base_point(mark),
                })
                .collect();
        }
        let orientation = geometry.orientation;
        let mut result = Vec::with_capacity(points.len());
        let mut segment = 0;
        for point in points {
            let (across, _) = orientation.axes(point);
            while segment < marks.len().saturating_sub(2)
                && across > geometry.across_mark(&marks[segment + 1])
            {
                segment += 1;
            }
            let a = &marks[segment];
            let b = &marks[(segment + 1).min(marks.len() - 1)];
            let span = geometry.across_mark(b) - geometry.across_mark(a);
            let t = if span > 0.0001 {
                ((across - geometry.across_mark(a)) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            result.push(FillPoint {
                value: point,
                base: orientation.point(across, a.base + (b.base - a.base) * t),
            });
        }
        result
    }

    const CURVE_STEPS: usize = 8;

    fn is_increasing(points: &[Point]) -> bool {
        (1..points.len()).all(|index| points[index].x > points[index - 1].x)
    }

    /// Fritsch-Carlson tangents: monotone, so the curve never overshoots.
    fn tangents(points: &[Point]) -> Vec<f32> {
        let count = points.len();
        let mut slopes = vec![0.0; count - 1];
        for i in 0..count - 1 {
            slopes[i] = (points[i + 1].y - points[i].y) / (points[i + 1].x - points[i].x);
        }
        let mut tangents = vec![0.0; count];
        tangents[0] = slopes[0];
        tangents[count - 1] = slopes[count - 2];
        for i in 1..count - 1 {
            tangents[i] = if slopes[i - 1] * slopes[i] <= 0.0 {
                0.0
            } else {
                (slopes[i - 1] + slopes[i]) / 2.0
            };
        }
        for i in 0..count - 1 {
            if slopes[i] == 0.0 {
                tangents[i] = 0.0;
                tangents[i + 1] = 0.0;
                continue;
            }
            let a = tangents[i] / slopes[i];
            let b = tangents[i + 1] / slopes[i];
            let magnitude = a * a + b * b;
            if magnitude > 9.0 {
                let scale = 3.0 / magnitude.sqrt();
                tangents[i] = scale * a * slopes[i];
                tangents[i + 1] = scale * b * slopes[i];
            }
        }
        tangents
    }

    fn bars(&self, marks: &[Mark], color: Color, list: &mut DrawList) {
        for mark in marks {
            let Some(rect) = mark.bar else { continue };
            let dimmed = self
                .selection
                .map(|selection| selection != ChartPoint::new(mark.series, mark.index))
                .unwrap_or(false);
            let fill = if dimmed {
                color.multiplied_alpha(0.45)
            } else {
                color
            };
            if !mark.is_outer_end {
                let _ = list.add_rect(rect, fill);
                continue;
            }
            let radius = 3.0_f32.min(rect.width / 2.0).min(rect.height / 2.0);
            let _ = list.add_rounded_rect(rect, radius, fill);
            if let Some(squared) = self.squared_end(rect, mark.base) {
                let _ = list.add_rect(squared, fill);
            }
        }
    }

    fn squared_end(&self, rect: Rect, base: f32) -> Option<Rect> {
        if !base.is_finite() {
            return None;
        }
        let radius = 3.0_f32.min(rect.width / 2.0).min(rect.height / 2.0);
        if radius <= 0.0 {
            return None;
        }
        match self.frame.orientation {
            ChartOrientation::Vertical => {
                let toward_base = base >= rect.y + rect.height / 2.0;
                Some(Rect::new(
                    rect.x,
                    if toward_base {
                        rect.y + rect.height - radius
                    } else {
                        rect.y
                    },
                    rect.width,
                    radius,
                ))
            }
            ChartOrientation::Horizontal => {
                let toward_base = base <= rect.x + rect.width / 2.0;
                Some(Rect::new(
                    if toward_base {
                        rect.x
                    } else {
                        rect.x + rect.width - radius
                    },
                    rect.y,
                    radius,
                    rect.height,
                ))
            }
        }
    }

    fn dot(&self, x: f32, y: f32, radius: f32, color: Color, list: &mut DrawList) {
        let _ = list.add_rounded_rect(
            Rect::new(x - radius, y - radius, radius * 2.0, radius * 2.0),
            radius,
            color,
        );
    }

    fn pixel(x: f32, y: f32) -> u64 {
        fn packed(value: f32) -> u64 {
            if !value.is_finite() {
                return 0;
            }
            let clamped = value.clamp(-1_000_000.0, 1_000_000.0).round() as i32;
            u64::from(clamped as u32)
        }
        packed(x) << 32 | packed(y)
    }

    fn ring(
        &self,
        geometry: &PieGeometry,
        wedge: &super::coordinates::PieWedge,
        outer: f32,
        gap: f32,
        color: Color,
        list: &mut DrawList,
    ) {
        let inner = geometry.inner_radius;
        let steps =
            (2).max((((wedge.end - wedge.start) / (2.0 * std::f32::consts::PI)) * 96.0) as usize);
        let point = |angle: f32, radius: f32| {
            Point::new(
                geometry.center.x + angle.cos() * radius,
                geometry.center.y + angle.sin() * radius,
            )
        };
        for step in 0..steps {
            let t0 = step as f32 / steps as f32;
            let t1 = (step + 1) as f32 / steps as f32;
            let a0 = wedge.start + (wedge.end - wedge.start) * t0;
            let a1 = wedge.start + (wedge.end - wedge.start) * t1;
            let outer_trim = gap / 2.0 / outer.max(1.0);
            let inner_trim = if inner > 0.0 { gap / 2.0 / inner } else { 0.0 };
            let o0 = point(a0.max(wedge.start + outer_trim), outer);
            let o1 = point(a1.min(wedge.end - outer_trim), outer);
            if inner <= 0.5 {
                let _ = list.add_triangle(geometry.center, o0, o1, color);
            } else {
                let i0 = point(a0.max(wedge.start + inner_trim), inner);
                let i1 = point(a1.min(wedge.end - inner_trim), inner);
                let _ = list.add_triangle(o0, o1, i1, color);
                let _ = list.add_triangle(o0, i1, i0, color);
            }
        }
    }
}

/// Values an external caller can feed back into the paint identity.
pub fn datum_identity(datum: &ChartDatum) -> (u32, u32) {
    (datum.x.to_bits(), datum.y.to_bits())
}

/// The value axis a threshold belongs to, exposed for hit guides.
pub fn threshold_axis(threshold: &ChartThreshold) -> ChartValueAxis {
    threshold.axis
}
