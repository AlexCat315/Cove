//! Translation of `Primitives/Data/Chart/Stacking.swift`.
//!
//! Per-series baselines and tops after stacking, plus what the scale needs to
//! know about them. A non-finite value stays non-finite: it is a gap, not zero.
use super::model::{ChartMark, ChartSeries, ChartStacking, ChartStyle, ChartValueAxis};

#[derive(Clone, Debug)]
pub struct ChartStack {
    /// Series indices on this axis that take part, in order.
    pub shown: Vec<usize>,
    pub bases: Vec<Vec<f32>>,
    pub tops: Vec<Vec<f32>>,
    /// Outermost nonzero bar at an index in each direction.
    pub outermost_up: Vec<Option<usize>>,
    pub outermost_down: Vec<Option<usize>>,
    pub has_bars: bool,
    pub is_stacked: bool,
    pub is_percent: bool,
    pub sample_count: usize,
    /// Values the domain must contain: finite tops plus zero for bars/areas.
    pub extent: Vec<f32>,
}

impl ChartStack {
    pub fn new(series: &[ChartSeries], visible: &[bool], style: &ChartStyle, axis: ChartValueAxis) -> Self {
        let shown: Vec<usize> = series
            .iter()
            .enumerate()
            .filter(|(index, item)| {
                visible.get(*index).copied().unwrap_or(false) && item.axis == axis
            })
            .map(|(index, _)| index)
            .collect();
        let has_bars = shown.iter().any(|index| series[*index].mark == ChartMark::Bar);
        let sample_count = shown
            .iter()
            .map(|index| series[*index].data.len())
            .max()
            .unwrap_or(0);
        let is_stacked = style.stacking != ChartStacking::None;
        let is_percent = style.stacking == ChartStacking::Percent;

        let mut up = vec![0.0_f32; sample_count];
        let mut down = vec![0.0_f32; sample_count];
        let mut tops: Vec<Vec<f32>> = vec![Vec::new(); series.len()];
        let mut bases: Vec<Vec<f32>> = vec![Vec::new(); series.len()];

        for &index in &shown {
            let data = &series[index].data;
            // Percent mode takes every series; otherwise only bars/areas stack.
            let stacks = is_percent
                || (is_stacked
                    && matches!(series[index].mark, ChartMark::Bar | ChartMark::Area));
            bases[index] = data
                .iter()
                .enumerate()
                .map(|(i, datum)| {
                    if !datum.y.is_finite() {
                        return f32::NAN;
                    }
                    if !stacks {
                        return 0.0;
                    }
                    if datum.y >= 0.0 { up[i] } else { down[i] }
                })
                .collect();
            tops[index] = data
                .iter()
                .enumerate()
                .map(|(i, datum)| {
                    if !datum.y.is_finite() {
                        return f32::NAN;
                    }
                    bases[index][i] + datum.y
                })
                .collect();
            if stacks {
                for (i, &top) in tops[index].iter().enumerate() {
                    if top.is_finite() {
                        if data[i].y >= 0.0 {
                            up[i] = top;
                        } else {
                            down[i] = top;
                        }
                    }
                }
            }
        }

        if is_percent {
            let mut factor = vec![1.0_f32; sample_count];
            for i in 0..sample_count {
                let span = up[i].max(-down[i]);
                if span > 0.0 {
                    factor[i] = 100.0 / span;
                }
            }
            for &index in &shown {
                for i in 0..bases[index].len() {
                    if bases[index][i].is_finite() {
                        bases[index][i] *= factor[i];
                        tops[index][i] *= factor[i];
                    }
                }
            }
        }

        let mut outermost_up = vec![None; sample_count];
        let mut outermost_down = vec![None; sample_count];
        if is_stacked {
            for &index in &shown {
                if series[index].mark != ChartMark::Bar {
                    continue;
                }
                for (i, datum) in series[index].data.iter().enumerate() {
                    if !datum.y.is_finite() || datum.y == 0.0 {
                        continue;
                    }
                    if datum.y > 0.0 {
                        outermost_up[i] = Some(index);
                    } else {
                        outermost_down[i] = Some(index);
                    }
                }
            }
        }

        let mut extent: Vec<f32> = shown
            .iter()
            .flat_map(|index| tops[*index].iter().copied())
            .filter(|value| value.is_finite())
            .collect();
        if shown.iter().any(|index| {
            matches!(series[*index].mark, ChartMark::Bar | ChartMark::Area)
        }) {
            extent.push(0.0);
        }

        Self {
            shown,
            bases,
            tops,
            outermost_up,
            outermost_down,
            has_bars,
            is_stacked,
            is_percent,
            sample_count,
            extent,
        }
    }

    /// Whether a bar's far end is exposed, so it can take a rounded corner.
    pub fn is_outer_end(&self, series: usize, position: usize, value: f32) -> bool {
        if !self.is_stacked {
            return true;
        }
        let outermost = if value >= 0.0 { &self.outermost_up } else { &self.outermost_down };
        outermost.get(position).copied().flatten() == Some(series)
    }
}
