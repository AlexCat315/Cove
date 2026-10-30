//! Translation of Primitives/Layout/Split/Constraints.swift.
//!
//! Pane bounds use logical points. Impossible minimums share space in proportion
//! to their minimums. Maximums constrain the divider whenever both panes can
//! fill the container; if their sum is too small, surplus is shared in proportion
//! to the maximums. This keeps a filling split deterministic without overflow.

/// Where the divider may rest, as a fraction of the available extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitConstraints {
    pub fraction_min: f32,
    pub fraction_max: f32,
    pub minimum_first: f32,
    pub minimum_second: f32,
    pub maximum_first: Option<f32>,
    pub maximum_second: Option<f32>,
}

impl Default for SplitConstraints {
    fn default() -> Self {
        Self {
            fraction_min: 0.1,
            fraction_max: 0.9,
            minimum_first: 0.0,
            minimum_second: 0.0,
            maximum_first: None,
            maximum_second: None,
        }
    }
}

impl SplitConstraints {
    pub fn new(fraction_min: f32, fraction_max: f32) -> Self {
        Self { fraction_min, fraction_max, ..Default::default() }
    }

    pub fn fraction_range(mut self, minimum: f32, maximum: f32) -> Self {
        self.fraction_min = minimum;
        self.fraction_max = maximum;
        self
    }

    pub fn minimums(mut self, first: f32, second: f32) -> Self {
        self.minimum_first = first;
        self.minimum_second = second;
        self
    }

    pub fn maximums(mut self, first: Option<f32>, second: Option<f32>) -> Self {
        self.maximum_first = first;
        self.maximum_second = second;
        self
    }

    /// Feasible fraction range for the given pane extent.
    pub fn range(&self, available: f32) -> (f32, f32) {
        let lower = clamp_unit(self.fraction_min, 0.1);
        let upper = clamp_unit(self.fraction_max, 0.9).max(lower);
        if !available.is_finite() || available <= 0.0 {
            return (lower, upper);
        }
        let first = finite_minimum(self.minimum_first);
        let second = finite_minimum(self.minimum_second);
        if first + second >= available && first + second > 0.0 {
            let share = first / (first + second);
            return (share, share);
        }
        let max_first = finite_maximum(self.maximum_first, first, available);
        let max_second = finite_maximum(self.maximum_second, second, available);
        if max_first + max_second < available {
            let proposed = if max_first + max_second > 0.0 {
                max_first / (max_first + max_second)
            } else {
                0.5
            };
            let share = (first / available).max((1.0 - second / available).min(proposed));
            return (share, share);
        }
        let feasible_low = (first / available).max(1.0 - max_second / available);
        let feasible_high = (1.0 - second / available).min(max_first / available);
        let constrained_low = lower.max(feasible_low);
        let constrained_high = upper.min(feasible_high);
        if constrained_low <= constrained_high {
            (constrained_low, constrained_high)
        } else {
            (feasible_low, feasible_high)
        }
    }

    /// Clamp a proposed fraction into this set of constraints.
    pub fn clamp(&self, fraction: f32, available: f32) -> f32 {
        let proposed = if fraction.is_finite() { fraction } else { 0.5 };
        let (low, high) = self.range(available);
        proposed.clamp(low, high)
    }
}

fn clamp_unit(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { fallback }
}

fn finite_minimum(value: f32) -> f32 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

fn finite_maximum(value: Option<f32>, minimum: f32, available: f32) -> f32 {
    match value {
        Some(value) if value.is_finite() => value.max(minimum),
        _ => available,
    }
}
