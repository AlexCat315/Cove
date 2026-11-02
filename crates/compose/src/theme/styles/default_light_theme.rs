//! Translation of Theme/Styles/DefaultLightTheme.swift.
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
            color(0xFF, 0xFF, 0xFF),
            color(0xF9, 0xF9, 0xFA),
            color(0xF0, 0xF0, 0xF2),
            Some(color(0xE6, 0xE6, 0xEA)),
            color(0xFF, 0xFF, 0xFF),
            color(0xFF, 0xFF, 0xFF),
            color(0xFF, 0xFF, 0xFF),
            color(0xFF, 0xFF, 0xFF),
            color(0x18, 0x18, 0x1B),
            color(0x18, 0x18, 0x1B),
            color(0x52, 0x52, 0x5B),
            color(0x71, 0x71, 0x7A),
            color(0x25, 0x63, 0xEB),
            color(0x1D, 0x4E, 0xD8),
            color(0x1E, 0x40, 0xAF),
            color(0xFF, 0xFF, 0xFF),
            alpha(0x25, 0x63, 0xEB, 0x24),
            Some(color(0x93, 0x33, 0xEA)),
            alpha(0x18, 0x18, 0x1B, 0x0A),
            alpha(0x18, 0x18, 0x1B, 0x14),
            alpha(0x25, 0x63, 0xEB, 0x29),
            color(0x16, 0xA3, 0x4A),
            color(0xD9, 0x77, 0x06),
            color(0xDC, 0x26, 0x26),
            color(0x25, 0x63, 0xEB),
            color(0xE7, 0xE7, 0xEB),
            color(0xC4, 0xC4, 0xCC),
            color(0xEE, 0xEE, 0xF1),
            alpha(0x25, 0x63, 0xEB, 0x8C),
            alpha(0x25, 0x63, 0xEB, 0x40),
            alpha(0x00, 0x00, 0x00, 0x66),
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
                color: Some(alpha(0, 0, 0, 0x1F)),
                offset_x: 0.0,
                offset_y: 8.0,
                blur: 28.0,
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
