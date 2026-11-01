//! Translation of Theme/Tokens/ColorScheme.swift.
use cove_core::Color;

/// Layered surface palette, foregrounds, accent ramp and structural colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
    pub background: Color,
    pub surface: Color,
    pub surface_variant: Color,
    pub surface_pressed: Color,
    pub surface_sunken: Color,
    pub surface_raised: Color,
    pub surface_floating: Color,
    pub surface_overlay: Color,

    pub on_background: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    pub on_surface_muted: Color,

    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub on_accent: Color,
    pub accent_muted: Color,
    /// Secondary accent for content classification (scene / audio assets,
    /// special syntax, badges) — the purple of the editor palette. Not part
    /// of the interactive accent ramp; never used for hover/press states.
    pub accent_secondary: Color,

    pub state_layer_hover: Color,
    pub state_layer_pressed: Color,
    pub state_layer_selected: Color,

    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    pub border: Color,
    pub border_strong: Color,
    pub divider: Color,
    pub focus_ring: Color,
    pub selection: Color,
    pub overlay: Color,
}
#[allow(clippy::too_many_arguments)]
impl ColorScheme {
    pub fn new(
        background: Color,
        surface: Color,
        surface_variant: Color,
        surface_pressed: Option<Color>,
        surface_sunken: Color,
        surface_raised: Color,
        surface_floating: Color,
        surface_overlay: Color,
        on_background: Color,
        on_surface: Color,
        on_surface_variant: Color,
        on_surface_muted: Color,
        accent: Color,
        accent_hover: Color,
        accent_pressed: Color,
        on_accent: Color,
        accent_muted: Color,
        accent_secondary: Option<Color>,
        state_layer_hover: Color,
        state_layer_pressed: Color,
        state_layer_selected: Color,
        success: Color,
        warning: Color,
        error: Color,
        info: Color,
        border: Color,
        border_strong: Color,
        divider: Color,
        focus_ring: Color,
        selection: Color,
        overlay: Color,
    ) -> Self {
        Self {
            background,
            surface,
            surface_variant,
            surface_pressed: surface_pressed
                .unwrap_or_else(|| mix(surface_variant, on_surface, 0.12)),
            surface_sunken,
            surface_raised,
            surface_floating,
            surface_overlay,
            on_background,
            on_surface,
            on_surface_variant,
            on_surface_muted,
            accent,
            accent_hover,
            accent_pressed,
            on_accent,
            accent_muted,
            accent_secondary: accent_secondary.unwrap_or(accent),
            state_layer_hover,
            state_layer_pressed,
            state_layer_selected,
            success,
            warning,
            error,
            info,
            border,
            border_strong,
            divider,
            focus_ring,
            selection,
            overlay,
        }
    }
}

/// Straight-alpha linear interpolation, matching `Color.mixed(with:amount:)`.
pub fn mix(from: Color, to: Color, amount: f32) -> Color {
    let t = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}
