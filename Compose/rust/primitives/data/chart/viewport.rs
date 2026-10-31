//! Translation of `Primitives/Data/Chart/Viewport.swift`.
//!
//! One independent-axis transform used by ticks, marks, brushing and
//! navigation. Works in the axis' own space so log zoom is multiplicative.
use cove_core::scene::Rect;

use super::model::{ChartOrientation, ChartRange, ChartScaleKind};

#[derive(Clone, Debug, PartialEq)]
pub struct ChartWindow {
    pub full: ChartRange,
    pub range: ChartRange,
    pub kind: ChartScaleKind,
    pub minimum_fraction: f32,
    pub is_categorical: bool,
}

impl ChartWindow {
    pub fn new(
        full: ChartRange,
        range: Option<ChartRange>,
        kind: ChartScaleKind,
        minimum_fraction: f32,
        is_categorical: bool,
    ) -> Self {
        let minimum = if minimum_fraction.is_finite() {
            minimum_fraction.clamp(0.0001, 1.0)
        } else {
            0.01
        };
        let base = Self {
            full,
            range: full,
            kind,
            minimum_fraction: minimum,
            is_categorical,
        };
        base.replacing(range)
    }

    pub fn replacing(&self, candidate: Option<ChartRange>) -> Self {
        let Some(candidate) = candidate else {
            return Self {
                range: self.full,
                ..self.clone()
            };
        };
        if !candidate.lower.is_finite()
            || !candidate.upper.is_finite()
            || (self.kind == ChartScaleKind::Log && candidate.lower <= 0.0)
        {
            return Self {
                range: self.full,
                ..self.clone()
            };
        }
        let lo = self.space(self.full.lower);
        let hi = self.space(self.full.upper);
        let width = hi - lo;
        let candidate_span = self.space(candidate.upper) - self.space(candidate.lower);
        let span = width.min((width * f64::from(self.minimum_fraction)).max(candidate_span));
        let start = lo.max(self.space(candidate.lower).min(hi - span));
        Self {
            range: ChartRange::new(self.value(start), self.value(start + span)),
            ..self.clone()
        }
    }

    pub fn fraction(&self, value: f32) -> f32 {
        let low = self.space(self.range.lower);
        let high = self.space(self.range.upper);
        ((self.space(value) - low) / (high - low)) as f32
    }

    pub fn value_at(&self, fraction: f32) -> f32 {
        let low = self.space(self.range.lower);
        let high = self.space(self.range.upper);
        self.value(low + f64::from(fraction) * (high - low))
    }

    pub fn zoom(&self, factor: f32, anchor: f32) -> Self {
        if !factor.is_finite() || factor <= 0.0 || !anchor.is_finite() {
            return self.clone();
        }
        let a = f64::from(anchor.clamp(0.0, 1.0));
        let start = self.space(self.range.lower);
        let span = self.space(self.range.upper) - start;
        let full_span = self.space(self.full.upper) - self.space(self.full.lower);
        let next = (full_span * f64::from(self.minimum_fraction))
            .max(span * f64::from(factor))
            .min(full_span);
        let lower = start + (span - next) * a;
        self.replacing(Some(ChartRange::new(
            self.value(lower),
            self.value(lower + next),
        )))
    }

    pub fn pan(&self, fraction: f32) -> Self {
        if !fraction.is_finite() {
            return self.clone();
        }
        let delta =
            f64::from(fraction) * (self.space(self.range.upper) - self.space(self.range.lower));
        self.replacing(Some(ChartRange::new(
            self.value(self.space(self.range.lower) + delta),
            self.value(self.space(self.range.upper) + delta),
        )))
    }

    pub fn position(&self, value: f32, plot: Rect, orientation: ChartOrientation) -> f32 {
        let ratio = self.fraction(value);
        if orientation == ChartOrientation::Vertical {
            return plot.x + ratio * plot.width;
        }
        if self.is_categorical {
            plot.y + ratio * plot.height
        } else {
            plot.y + plot.height - ratio * plot.height
        }
    }

    pub fn fraction_at(&self, x: f32, y: f32, plot: Rect, orientation: ChartOrientation) -> f32 {
        if orientation == ChartOrientation::Vertical {
            return (x - plot.x) / plot.width.max(1.0);
        }
        if self.is_categorical {
            (y - plot.y) / plot.height.max(1.0)
        } else {
            (plot.y + plot.height - y) / plot.height.max(1.0)
        }
    }

    fn space(&self, value: f32) -> f64 {
        if self.kind == ChartScaleKind::Log {
            f64::from(value).ln()
        } else {
            f64::from(value)
        }
    }

    fn value(&self, value: f64) -> f32 {
        if self.kind == ChartScaleKind::Log {
            value.exp() as f32
        } else {
            value as f32
        }
    }
}
