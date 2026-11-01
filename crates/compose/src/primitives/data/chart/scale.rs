//! Translation of `Primitives/Data/Chart/Scale.swift`.
//!
//! The value axis of a cartesian chart: the domain the plot maps and the round
//! values ticked across it. Fitting runs before layout: domain -> ticks ->
//! label widths -> plot.
use super::model::{ChartBand, ChartRange, ChartScaleKind, ChartStyle, ChartThreshold};
use super::stacking::ChartStack;

#[derive(Clone, Debug, PartialEq)]
pub struct ChartScale {
    pub domain: ChartRange,
    /// Ascending tick values inside `domain`, bounds included.
    pub ticks: Vec<f32>,
    /// Decimal places that render the tick step exactly.
    pub decimals: i32,
    pub kind: ChartScaleKind,
}

impl ChartScale {
    /// Fits a stack to round bounds at about `tick_count` intervals. Reference
    /// rules and bands widen the domain with the data. Explicit min/max are
    /// honoured as-is. A percent stack always fits linearly.
    pub fn fit_stack(
        stack: &ChartStack,
        thresholds: &[ChartThreshold],
        bands: &[ChartBand],
        style: &ChartStyle,
        tick_count: i32,
        kind: ChartScaleKind,
    ) -> Self {
        let mut extent = stack.extent.clone();
        extent.extend(thresholds.iter().map(|threshold| threshold.value));
        for band in bands {
            extent.push(band.from);
            extent.push(band.to);
        }
        Self::fit_extent(
            &extent,
            style.min_value,
            style.max_value,
            tick_count,
            if stack.is_percent {
                ChartScaleKind::Linear
            } else {
                kind
            },
        )
    }

    /// Fits raw values; the shared path for the value axis and numeric x.
    pub fn fit_extent(
        extent: &[f32],
        lower: Option<f32>,
        upper: Option<f32>,
        tick_count: i32,
        kind: ChartScaleKind,
    ) -> Self {
        match kind {
            ChartScaleKind::Linear => Self::fit_linear(extent, lower, upper, tick_count),
            ChartScaleKind::Log => Self::fit_log(extent, lower, upper, tick_count),
        }
    }

    fn fit_linear(
        extent: &[f32],
        explicit_min: Option<f32>,
        explicit_max: Option<f32>,
        tick_count: i32,
    ) -> Self {
        let explicit_min = explicit_min.filter(|value| value.is_finite());
        let explicit_max = explicit_max.filter(|value| value.is_finite());
        let values: Vec<f32> = extent
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .collect();
        let mut lower =
            explicit_min.unwrap_or_else(|| values.iter().copied().reduce(f32::min).unwrap_or(0.0));
        let mut upper =
            explicit_max.unwrap_or_else(|| values.iter().copied().reduce(f32::max).unwrap_or(1.0));
        if lower > upper {
            std::mem::swap(&mut lower, &mut upper);
        }
        if lower == upper {
            let pad = (lower.abs() * 0.1).max(1.0);
            lower -= pad;
            upper += pad;
        }
        let step = Self::nice_step((upper - lower) / tick_count.max(1) as f32);
        if explicit_min.is_none() {
            lower = (lower / step).floor() * step;
        }
        if explicit_max.is_none() {
            upper = (upper / step).ceil() * step;
        }
        let ticks = if tick_count > 0 {
            Self::ticks(lower, upper, step)
        } else {
            Vec::new()
        };
        Self {
            domain: ChartRange::new(lower, upper),
            ticks,
            decimals: Self::decimals(step),
            kind: ChartScaleKind::Linear,
        }
    }

    /// Fits in log space and snaps to whole decades. Non-positive values and
    /// bounds have no place on the axis and are ignored.
    fn fit_log(
        extent: &[f32],
        explicit_min: Option<f32>,
        explicit_max: Option<f32>,
        tick_count: i32,
    ) -> Self {
        let explicit_min = explicit_min.filter(|value| value.is_finite() && *value > 0.0);
        let explicit_max = explicit_max.filter(|value| value.is_finite() && *value > 0.0);
        let values: Vec<f32> = extent
            .iter()
            .copied()
            .filter(|value| value.is_finite() && *value > 0.0)
            .collect();
        let mut lower =
            explicit_min.unwrap_or_else(|| values.iter().copied().reduce(f32::min).unwrap_or(1.0));
        let mut upper =
            explicit_max.unwrap_or_else(|| values.iter().copied().reduce(f32::max).unwrap_or(10.0));
        if lower > upper {
            std::mem::swap(&mut lower, &mut upper);
        }
        if explicit_min.is_none() {
            lower = Self::decade(lower.log10().floor());
        }
        if explicit_max.is_none() {
            upper = Self::decade(upper.log10().ceil());
        }
        if upper <= lower {
            lower /= 10.0;
            upper *= 10.0;
        }
        let ticks = if tick_count > 0 {
            Self::log_ticks(lower, upper)
        } else {
            Vec::new()
        };
        let decimals = Self::decimals_for_log(&ticks);
        Self {
            domain: ChartRange::new(lower, upper),
            ticks,
            decimals,
            kind: ChartScaleKind::Log,
        }
    }

    /// The y for a value, clamped to the plot.
    pub fn y(&self, value: f32, plot: cove_core::scene::Rect) -> f32 {
        plot.y + plot.height - self.ratio(value) * plot.height
    }
    /// The x for a value, clamped to the plot.
    pub fn x(&self, value: f32, plot: cove_core::scene::Rect) -> f32 {
        plot.x + self.ratio(value) * plot.width
    }

    /// 0 at the lower bound and 1 at the upper. Off-axis values clamp; on a log
    /// axis anything at or below zero clamps to the bottom.
    pub fn ratio(&self, value: f32) -> f32 {
        if !value.is_finite() {
            return 0.0;
        }
        match self.kind {
            ChartScaleKind::Linear => {
                let span = self.domain.span().max(0.001);
                ((value - self.domain.lower) / span).clamp(0.0, 1.0)
            }
            ChartScaleKind::Log => {
                if value <= 0.0 || self.domain.lower <= 0.0 {
                    return 0.0;
                }
                let low = self.domain.lower.log10();
                let high = self.domain.upper.log10();
                let span = (high - low).max(0.0001);
                ((value.log10() - low) / span).clamp(0.0, 1.0)
            }
        }
    }

    pub fn label(&self, value: f32) -> String {
        Self::fixed(value, self.decimals)
    }

    /// Fixed precision with trailing zeros and a bare "-0" trimmed.
    pub fn fixed(value: f32, decimals: i32) -> String {
        if !value.is_finite() {
            return String::new();
        }
        let places = decimals.max(0) as usize;
        let mut text = format!("{value:.places$}");
        if decimals > 0 {
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.pop();
            }
        }
        if text == "-0" { "0".to_string() } else { text }
    }

    /// Smallest step of 1, 2, 2.5 or 5 x 10^n that is at least `raw`.
    pub fn nice_step(raw: f32) -> f32 {
        if !raw.is_finite() || raw <= 0.0 {
            return 1.0;
        }
        let magnitude = 10.0_f32.powf(raw.log10().floor());
        let fraction = raw / magnitude;
        let nice = if fraction < 1.5 {
            1.0
        } else if fraction < 2.25 {
            2.0
        } else if fraction < 3.5 {
            2.5
        } else if fraction < 7.5 {
            5.0
        } else {
            10.0
        };
        nice * magnitude
    }

    /// Ticks on multiples of `step` inside the domain.
    fn ticks(lower: f32, upper: f32, step: f32) -> Vec<f32> {
        let first = (lower / step).ceil() * step;
        let last = (upper / step).floor() * step;
        if last < first {
            return vec![lower, upper];
        }
        let count = ((last - first) / step).round() as i64;
        if !(0..=64).contains(&count) {
            return vec![lower, upper];
        }
        (0..=count)
            .map(|index| first + index as f32 * step)
            .collect()
    }

    /// Ticks at 1, 2 and 5 x 10^n, thinned to a countable set.
    fn log_ticks(lower: f32, upper: f32) -> Vec<f32> {
        let low = lower.log10().floor().clamp(-38.0, 38.0) as i32;
        let high = upper.log10().ceil().clamp(-38.0, 38.0) as i32;
        let mut ticks = Vec::new();
        for exponent in low..=high {
            for multiplier in [1.0_f32, 2.0, 5.0] {
                let value = multiplier * Self::decade(exponent as f32);
                if value >= lower * 0.9999 && value <= upper * 1.0001 {
                    ticks.push(value);
                }
            }
        }
        if ticks.len() <= 1 {
            return vec![lower, upper];
        }
        let mut chosen = if ticks.len() <= 9 {
            ticks
        } else {
            ticks
                .into_iter()
                .filter(|value| Self::is_decade(*value))
                .collect()
        };
        if chosen.len() > 9 {
            let stride = (chosen.len() as f32 / 9.0).ceil() as usize;
            chosen = chosen
                .into_iter()
                .enumerate()
                .filter(|(index, _)| index % stride == 0)
                .map(|(_, value)| value)
                .collect();
        }
        chosen
    }

    fn is_decade(value: f32) -> bool {
        let exponent = value.log10();
        (exponent - exponent.round()).abs() < 0.0001
    }

    fn decade(exponent: f32) -> f32 {
        10.0_f32.powf(exponent.clamp(-38.0, 38.0))
    }

    /// First decimal count in 0...4 that represents `step` without rounding.
    fn decimals(step: f32) -> i32 {
        if !step.is_finite() || step <= 0.0 {
            return 0;
        }
        for places in 0..=4 {
            let scaled = step * 10.0_f32.powi(places);
            if (scaled - scaled.round()).abs() < 0.0001 * scaled.abs().max(1.0) {
                return places;
            }
        }
        4
    }

    /// Log ticks span decades, so the smallest tick sets precision.
    fn decimals_for_log(ticks: &[f32]) -> i32 {
        let mut places = 0;
        for tick in ticks
            .iter()
            .copied()
            .filter(|tick| *tick > 0.0 && *tick < 1.0)
        {
            places = places.max((-tick.log10()).ceil() as i32);
        }
        places.min(4)
    }
}
