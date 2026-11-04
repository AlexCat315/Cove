//! Presentation shared by every numeric field.
//!
//! `NumberField` and `Vec3Field` draw the same value formatting, bounds, step
//! and enabled state, so they take one configuration type instead of each
//! re-declaring the same builders. Vector-only concerns (axis constraints,
//! mixed axes, the context menu) stay on `Vec3Field`.
use crate::primitives::text::text_field::TextFieldSize;

use super::text_format::NumericTextFormat;

/// Value formatting, bounds and interaction shared by numeric fields.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFieldOptions {
    pub format: NumericTextFormat,
    pub size: TextFieldSize,
    pub is_enabled: bool,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
    pub step: Option<f32>,
}

impl Default for NumberFieldOptions {
    fn default() -> Self {
        Self {
            format: NumericTextFormat::invariant(2),
            size: TextFieldSize::Automatic,
            is_enabled: true,
            min_value: None,
            max_value: None,
            step: None,
        }
    }
}

impl NumberFieldOptions {
    /// Locale-specific presentation.
    pub fn format(mut self, format: NumericTextFormat) -> Self {
        self.format = format;
        self
    }

    /// Shorthand for the invariant format at a fixed precision.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.format = NumericTextFormat::invariant(decimals.min(6));
        self
    }

    pub fn size(mut self, size: TextFieldSize) -> Self {
        self.size = size;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn range(mut self, min_value: Option<f32>, max_value: Option<f32>) -> Self {
        self.min_value = min_value;
        self.max_value = max_value;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }
}
