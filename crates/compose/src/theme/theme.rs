//! Translation of Theme/Theme.swift.
use super::tokens::{
    ColorScheme, ElevationScale, IconTheme, MotionScale, RadiusScale, SpacingScale, TextEmphasis,
    ThemeExtensions, Typography,
};
use crate::theme::styles::InputAppearance;

/// Six-dimensional design token bundle consumed by Compose-layer styles.
///
/// `Theme` is a value type — switching themes means re-providing a new value
/// through `ThemeEnvironment`. Per-axis token containers are value types so a
/// call site can derive a variant without affecting other consumers.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub colors: ColorScheme,
    pub typography: Typography,
    pub spacing: SpacingScale,
    pub radius: RadiusScale,
    pub elevation: ElevationScale,
    pub motion: MotionScale,
    pub inputs: InputAppearance,
    pub text_emphasis: TextEmphasis,
    pub icons: IconTheme,
    pub extensions: ThemeExtensions,
}
impl Theme {
    pub fn new(
        colors: ColorScheme,
        typography: Typography,
        spacing: SpacingScale,
        radius: RadiusScale,
        elevation: ElevationScale,
        motion: MotionScale,
        inputs: Option<InputAppearance>,
    ) -> Self {
        let inputs = inputs.unwrap_or_else(|| {
            InputAppearance::new(
                colors.surface,
                colors.surface_variant,
                colors.border,
                colors.border_strong,
                colors.accent,
                colors.error,
                colors.border,
                1.0,
                1.0,
                colors.surface_variant,
                colors.on_surface_muted,
                colors.border,
                radius.md,
            )
        });
        Self {
            colors,
            typography,
            spacing,
            radius,
            elevation,
            motion,
            inputs,
            text_emphasis: TextEmphasis::default(),
            icons: IconTheme::default(),
            extensions: ThemeExtensions::new(),
        }
    }

    /// Project-wide dark theme. Value-stable across calls; safe to compare
    /// for identity within a frame.
    pub fn default_dark() -> Self {
        super::styles::default_dark_theme::value()
    }
    pub fn default_light() -> Self {
        super::styles::default_light_theme::value()
    }
}
