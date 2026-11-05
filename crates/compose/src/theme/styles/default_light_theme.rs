//! Native grouped light palette: system gray canvas, white cards, system blue.
//!
//! The window is the grouped-list canvas; surfaces are the white cards and
//! control faces that sit on it. Text uses Apple's label hierarchy and the
//! accent is a readable system blue that keeps white labels above 4.5:1.
use crate::theme::{ColorScheme, ElevationScale, Theme};
use cove_core::{Color, LayeredShadow, SurfaceShadow};

fn color(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 255)
}
fn alpha(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// Default light theme.
pub fn value() -> Theme {
    let dark = super::default_dark_theme::value();
    Theme::new(
        ColorScheme::new(
            color(0xF2, 0xF2, 0xF7),        // background
            color(0xFF, 0xFF, 0xFF),        // surface
            color(0xE9, 0xE9, 0xEE),        // surface variant
            Some(color(0xDC, 0xDC, 0xE2)),  // pressed
            color(0xFF, 0xFF, 0xFF),        // sunken input
            color(0xFF, 0xFF, 0xFF),        // raised
            color(0xFF, 0xFF, 0xFF),        // floating
            color(0xFF, 0xFF, 0xFF),        // overlay surface
            color(0x1D, 0x1D, 0x1F),        // on background
            color(0x1D, 0x1D, 0x1F),        // on surface
            color(0x55, 0x55, 0x5A),        // secondary text
            color(0x6E, 0x6E, 0x73),        // muted text (>=4.5:1 on the canvas)
            color(0x0A, 0x6C, 0xD8),        // accent
            color(0x0A, 0x5F, 0xBF),        // accent hover
            color(0x08, 0x4C, 0x99),        // accent pressed
            color(0xFF, 0xFF, 0xFF),        // on accent
            alpha(0x0A, 0x6C, 0xD8, 0x24),  // muted accent
            Some(color(0xAF, 0x52, 0xDE)),  // secondary accent
            alpha(0x00, 0x00, 0x00, 0x08),  // hover layer
            alpha(0x00, 0x00, 0x00, 0x14),  // pressed layer
            alpha(0x0A, 0x6C, 0xD8, 0x29),  // selected layer
            color(0x1B, 0x7F, 0x3B),        // success
            color(0xB2, 0x50, 0x00),        // warning
            color(0xD7, 0x00, 0x15),        // error
            color(0x0A, 0x6C, 0xD8),        // info
            color(0xD1, 0xD1, 0xD6),        // border
            color(0x8E, 0x8E, 0x93),        // strong border
            color(0xE5, 0xE5, 0xEA),        // divider
            alpha(0x0A, 0x6C, 0xD8, 0x8C),  // focus ring
            alpha(0x0A, 0x6C, 0xD8, 0x40),  // text selection
            alpha(0x00, 0x00, 0x00, 0x66),  // scrim
        ),
        dark.typography.clone(),
        dark.spacing,
        dark.radius,
        ElevationScale::new(
            SurfaceShadow::NONE,
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x1F)),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 2.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x0F)),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 3.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x2E)),
                offset_x: 0.0,
                offset_y: 6.0,
                blur: 20.0,
            },
            LayeredShadow {
                ambient: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x1A)),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur: 40.0,
                },
                key: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x4D)),
                    offset_x: 0.0,
                    offset_y: 10.0,
                    blur: 16.0,
                },
            },
        ),
        dark.motion,
        None,
    )
}
