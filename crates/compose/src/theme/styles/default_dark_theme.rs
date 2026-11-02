//! Translation of Theme/Styles/DefaultDarkTheme.swift.
use crate::theme::{
    ColorScheme, Easing, ElevationScale, MotionScale, RadiusScale, SpacingScale, TextStyleToken,
    Theme, Typography,
};
use cove_core::{Color, Font, FontWeight, LayeredShadow, SurfaceShadow};

fn color(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 255)
}
fn alpha(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// Default dark theme bundled with CoveUI.
pub fn value() -> Theme {
    Theme::new(
        ColorScheme::new(
            color(0x18, 0x18, 0x1B),
            color(0x1E, 0x1E, 0x21),
            color(0x2A, 0x2A, 0x2E),
            Some(color(0x35, 0x35, 0x3A)),
            color(0x18, 0x18, 0x1B),
            color(0x1D, 0x1D, 0x20),
            color(0x18, 0x18, 0x1B),
            color(0x18, 0x18, 0x1B),
            color(0xF4, 0xF4, 0xF5),
            color(0xF4, 0xF4, 0xF5),
            color(0xA1, 0xA1, 0xAA),
            color(0xA1, 0xA1, 0xAA),
            color(0x3B, 0x82, 0xF6),
            color(0x60, 0xA5, 0xFA),
            color(0x25, 0x63, 0xEB),
            color(0xFF, 0xFF, 0xFF),
            alpha(0x3B, 0x82, 0xF6, 0x33),
            Some(color(0xA8, 0x55, 0xF7)),
            alpha(0xFF, 0xFF, 0xFF, 0x0F),
            alpha(0xFF, 0xFF, 0xFF, 0x1A),
            alpha(0x3B, 0x82, 0xF6, 0x38),
            color(0x22, 0xC5, 0x5E),
            color(0xF5, 0x9E, 0x0B),
            color(0xEF, 0x44, 0x44),
            color(0x3B, 0x82, 0xF6),
            color(0x34, 0x34, 0x3A),
            color(0x56, 0x56, 0x5D),
            color(0x2A, 0x2A, 0x2E),
            alpha(0x60, 0xA5, 0xFA, 0x99),
            alpha(0x3B, 0x82, 0xF6, 0x66),
            alpha(0x00, 0x00, 0x00, 0x66),
        ),
        Typography::new(
            None,
            TextStyleToken::new(Font::system(32.0, FontWeight::Bold), 38.0, -0.6),
            TextStyleToken::new(Font::system(24.0, FontWeight::Bold), 30.0, -0.4),
            TextStyleToken::new(Font::system(17.0, FontWeight::Semibold), 23.0, -0.15),
            TextStyleToken::new(Font::system(14.0, FontWeight::Regular), 20.0, 0.0),
            TextStyleToken::new(Font::system(14.0, FontWeight::Semibold), 20.0, 0.0),
            TextStyleToken::new(Font::system(12.0, FontWeight::Regular), 17.0, 0.0),
            TextStyleToken::new(Font::system(13.0, FontWeight::Medium), 18.0, 0.0),
            TextStyleToken::new(Font::monospaced(13.0, FontWeight::Regular), 20.0, 0.0),
        ),
        SpacingScale::new(4.0, 6.0, 8.0, 12.0, 16.0, 24.0),
        RadiusScale::new(0.0, 6.0, 8.0, 12.0, 16.0, 9999.0),
        ElevationScale::new(
            SurfaceShadow::NONE,
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x4D)),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 2.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x33)),
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
                    color: Some(alpha(0, 0, 0, 0x66)),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur: 40.0,
                },
                key: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x8C)),
                    offset_x: 0.0,
                    offset_y: 10.0,
                    blur: 16.0,
                },
            },
        ),
        MotionScale::new(0.08, 0.18, 0.32, Easing::EMPHASIZED, Easing::STANDARD),
        None,
    )
}
