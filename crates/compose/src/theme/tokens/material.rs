//! Translucent materials: frosted surfaces that let the canvas glow through.
use cove_core::Color;

/// One frosted surface: a translucent tint over a blurred backdrop, edged by
/// a light rim that catches the canvas behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassMaterial {
    pub tint: Color,
    /// Backdrop blur radius in points.
    pub blur: f32,
    pub rim: Color,
    pub rim_width: f32,
}

/// Theme-owned materials, from the most transparent to the most opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialScale {
    /// Sidebars, toolbars and other chrome that floats over content.
    pub chrome: GlassMaterial,
    /// Cards and grouped content, which keep more tint for legibility.
    pub content: GlassMaterial,
    /// Menus, popovers and dialogs: dense text over arbitrary content.
    pub floating: GlassMaterial,
}

impl MaterialScale {
    /// Derives the materials from the surface colours.
    pub fn from_surface(surface: Color, floating: Color) -> Self {
        let light = surface.r + surface.g + surface.b > 1.5;
        // A light pane needs a faint dark hairline to separate it from a light
        // canvas; a dark pane catches a faint highlight instead.
        let rim = if light {
            Color::from_rgba8(0x00, 0x00, 0x00, 0x14)
        } else {
            Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x1A)
        };
        Self {
            chrome: GlassMaterial {
                tint: surface.multiplied_alpha(if light { 0.8 } else { 0.72 }),
                blur: 40.0,
                rim,
                rim_width: 1.0,
            },
            content: GlassMaterial {
                tint: surface.multiplied_alpha(if light { 0.78 } else { 0.7 }),
                blur: 28.0,
                rim,
                rim_width: 1.0,
            },
            floating: GlassMaterial {
                tint: floating.multiplied_alpha(if light { 0.82 } else { 0.8 }),
                blur: 32.0,
                rim,
                rim_width: 1.0,
            },
        }
    }
}

/// The tint a translucent window lays over the platform's blurred material.
///
/// The light material already lifts what is behind it towards white, so any
/// light wash only hides the translucency; it gets none. The dark material
/// lets bright content behind the window through strongly, so a faint wash of
/// the chrome tone steadies the contrast of non-vibrant text.
pub fn window_backdrop_tint(colors: &crate::theme::ColorScheme) -> Option<Color> {
    let base = colors.surface_variant;
    let light = base.r + base.g + base.b > 1.5;
    (!light).then(|| base.multiplied_alpha(0.18))
}
