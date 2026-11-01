//! Translation of Theme/Tokens/Metrics.swift.
use crate::theme::Theme;
use std::sync::Arc;

/// An open, late-bound numeric token. Application roles can read their own
/// `ThemeExtension` without adding cases to a framework enum.
#[derive(Clone)]
pub struct ThemeMetric {
    resolve: Arc<dyn Fn(&Theme) -> f32>,
}
impl ThemeMetric {
    pub fn new(resolve: impl Fn(&Theme) -> f32 + 'static) -> Self {
        Self {
            resolve: Arc::new(resolve),
        }
    }
    pub fn resolve(&self, theme: &Theme) -> f32 {
        (self.resolve)(theme)
    }
    pub fn content_spacing() -> Self {
        Self::new(|theme| theme.spacing.lg)
    }
    pub fn section_spacing() -> Self {
        Self::new(|theme| theme.spacing.xxl)
    }
    pub fn row_radius() -> Self {
        Self::new(|theme| theme.radius.sm)
    }
    pub fn input_radius() -> Self {
        Self::new(|theme| theme.inputs.radius)
    }
    pub fn control_radius() -> Self {
        Self::new(|theme| theme.radius.md)
    }
    pub fn group_radius() -> Self {
        Self::new(|theme| theme.radius.lg)
    }
    pub fn panel_radius() -> Self {
        Self::new(|theme| theme.radius.xl)
    }
}
