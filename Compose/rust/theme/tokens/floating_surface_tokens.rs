//! Translation of Theme/Tokens/FloatingSurfaceTokens.swift.
use super::ColorScheme;
use cove_core::Color;

/// The outer hairline of a floating panel. A light panel has no darker body to
/// separate it from a light page, so its edge is a crisp neutral line rather
/// than the palette's tinted border.
pub fn floating_outline(scheme: &ColorScheme) -> Color {
    if is_light_floating(scheme) {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.1,
        }
    } else {
        scheme.border
    }
}
fn is_light_floating(scheme: &ColorScheme) -> bool {
    let surface = scheme.surface_floating;
    0.2126 * surface.r + 0.7152 * surface.g + 0.0722 * surface.b > 0.5
}
