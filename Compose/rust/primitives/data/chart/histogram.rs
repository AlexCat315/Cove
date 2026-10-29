//! Translation of `Primitives/Data/Chart/Histogram.swift`.
//!
//! Counts samples into even-width bins. Binning is a data concern, so it
//! happens here and the chart just draws the bars. Samples outside an explicit
//! range are ignored rather than folded into the tails.
use super::model::{ChartMark, ChartRange, ChartSeries, ChartValueAxis};
use super::model::ChartSeriesStyle;
use super::scale::ChartScale;
use crate::theme::tokens::SemanticColorRef;

#[derive(Clone)]
pub struct ChartHistogram {
    /// Number of bins, clamped to 1...64.
    pub bin_count: usize,
    /// Span to bin over; `None` fits the samples, so nothing is dropped.
    pub range: Option<ChartRange>,
}
impl Default for ChartHistogram {
    fn default() -> Self {
        Self { bin_count: 10, range: None }
    }
}
impl ChartHistogram {
    pub fn new(bin_count: usize, range: Option<ChartRange>) -> Self {
        let range = range.filter(|value| {
            value.lower.is_finite() && value.upper.is_finite() && value.lower < value.upper
        });
        Self { bin_count: bin_count.clamp(1, 64), range }
    }

    /// Bins the finite samples. Non-finite values never reach a bin.
    pub fn build(
        &self,
        values: &[f32],
        color: SemanticColorRef,
        title: impl Into<String>,
    ) -> ChartHistogramResult {
        let samples: Vec<f32> = values.iter().copied().filter(|value| value.is_finite()).collect();
        let domain = self.range.unwrap_or_else(|| Self::fitted(&samples));
        let count = self.bin_count.max(1);
        let width = domain.span() / count as f32;
        let mut counts = vec![0_usize; count];
        for value in samples {
            if value < domain.lower || value > domain.upper {
                continue;
            }
            let position = (value - domain.lower) / width;
            let index = (position.floor() as isize).clamp(0, count as isize - 1) as usize;
            counts[index] += 1;
        }
        let decimals = Self::decimals(width);
        let labels: Vec<String> = (0..count)
            .map(|index| {
                let lower = domain.lower + index as f32 * width;
                format!(
                    "{}\u{2013}{}",
                    ChartScale::fixed(lower, decimals),
                    ChartScale::fixed(lower + width, decimals)
                )
            })
            .collect();
        let series = ChartSeries::values(
            counts.iter().map(|value| *value as f32).collect(),
            color,
            ChartMark::Bar,
            title,
            ChartValueAxis::Left,
            ChartSeriesStyle::default(),
        );
        ChartHistogramResult { series, labels, counts }
    }

    /// The samples' own span. Identical samples still need a width to divide.
    fn fitted(samples: &[f32]) -> ChartRange {
        let Some(low) = samples.iter().copied().reduce(f32::min) else {
            return ChartRange::new(0.0, 1.0);
        };
        let high = samples.iter().copied().reduce(f32::max).unwrap_or(low);
        if high > low {
            ChartRange::new(low, high)
        } else {
            ChartRange::new(low - 0.5, low + 0.5)
        }
    }

    /// First precision in 0...3 that states the bin width without rounding.
    fn decimals(width: f32) -> i32 {
        if !width.is_finite() || width <= 0.0 {
            return 0;
        }
        for places in 0..=3 {
            let scaled = width * 10.0_f32.powi(places);
            if (scaled - scaled.round()).abs() < 0.001 * scaled.abs().max(1.0) {
                return places;
            }
        }
        3
    }
}

#[derive(Clone)]
pub struct ChartHistogramResult {
    /// Bars, one per bin, titled `title`.
    pub series: ChartSeries,
    /// Each bin's edges, e.g. "0-120".
    pub labels: Vec<String>,
    pub counts: Vec<usize>,
}
