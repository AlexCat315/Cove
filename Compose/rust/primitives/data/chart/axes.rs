//! Translation of `Primitives/Data/Chart/Axes.swift`.
//!
//! One stack and one scale per value side. Each side fits only its own series,
//! so a rate and a count can share a plot without flattening each other.
use super::model::{Axis as ChartAxisConfig, ChartSeries, ChartStyle, ChartValueAxis};
use super::plot::ChartGuides;
use super::scale::ChartScale;
use super::stacking::ChartStack;

#[derive(Clone)]
pub struct ChartValueAxes {
    pub primary: ChartStack,
    pub secondary: Option<ChartStack>,
    pub primary_scale: ChartScale,
    pub secondary_scale: Option<ChartScale>,
    /// x slots: widest series on either axis.
    pub sample_count: usize,
    /// Any visible series draws bars, so every mark centers on a bar slot.
    pub has_bars: bool,
    pub is_stacked: bool,
}

impl ChartValueAxes {
    pub fn resolve(
        series: &[ChartSeries],
        visible: &[bool],
        style: &ChartStyle,
        guides: &ChartGuides,
        axis: &ChartAxisConfig,
    ) -> Self {
        let primary = ChartStack::new(series, visible, style, ChartValueAxis::Left);
        let uses_secondary = series.iter().enumerate().any(|(index, item)| {
            visible.get(index).copied().unwrap_or(false) && item.axis == ChartValueAxis::Right
        });
        let secondary = if uses_secondary {
            Some(ChartStack::new(
                series,
                visible,
                style,
                ChartValueAxis::Right,
            ))
        } else {
            None
        };
        let primary_scale = ChartScale::fit_stack(
            &primary,
            &guides
                .thresholds
                .iter()
                .filter(|item| item.axis == ChartValueAxis::Left)
                .cloned()
                .collect::<Vec<_>>(),
            &guides
                .bands
                .iter()
                .filter(|item| item.axis == ChartValueAxis::Left)
                .cloned()
                .collect::<Vec<_>>(),
            style,
            axis.y.tick_count,
            axis.y.kind,
        );
        let secondary_scale = secondary.as_ref().map(|stack| {
            ChartScale::fit_stack(
                stack,
                &guides
                    .thresholds
                    .iter()
                    .filter(|item| item.axis == ChartValueAxis::Right)
                    .cloned()
                    .collect::<Vec<_>>(),
                &guides
                    .bands
                    .iter()
                    .filter(|item| item.axis == ChartValueAxis::Right)
                    .cloned()
                    .collect::<Vec<_>>(),
                style,
                axis.secondary.tick_count,
                axis.secondary.kind,
            )
        });
        Self {
            sample_count: primary.sample_count.max(
                secondary
                    .as_ref()
                    .map(|stack| stack.sample_count)
                    .unwrap_or(0),
            ),
            has_bars: primary.has_bars
                || secondary
                    .as_ref()
                    .map(|stack| stack.has_bars)
                    .unwrap_or(false),
            is_stacked: primary.is_stacked,
            primary,
            secondary,
            primary_scale,
            secondary_scale,
        }
    }

    pub fn uses_secondary(&self) -> bool {
        self.secondary.is_some()
    }

    /// Every visible series index on either axis, in series order.
    pub fn shown(&self) -> Vec<usize> {
        let mut shown = self.primary.shown.clone();
        if let Some(secondary) = &self.secondary {
            shown.extend(secondary.shown.iter().copied());
        }
        shown.sort_unstable();
        shown
    }

    /// A right-axis request on a chart without one falls back to the left.
    pub fn stack(&self, axis: ChartValueAxis) -> &ChartStack {
        if axis == ChartValueAxis::Right {
            self.secondary.as_ref().unwrap_or(&self.primary)
        } else {
            &self.primary
        }
    }

    pub fn scale(&self, axis: ChartValueAxis) -> &ChartScale {
        if axis == ChartValueAxis::Right {
            self.secondary_scale.as_ref().unwrap_or(&self.primary_scale)
        } else {
            &self.primary_scale
        }
    }
}
