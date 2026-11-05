//! Native grouped dark palette: system dark canvas, elevated cards, blue accent.
//!
//! The dark counterpart of the grouped light theme. Surfaces are one elevation
//! step above the canvas, dividers are hairlines, and the system blue keeps
//! white labels above 4.5:1.
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
            color(0x1C, 0x1C, 0x1E),       // background
            color(0x2C, 0x2C, 0x2E),       // surface
            color(0x3A, 0x3A, 0x3C),       // surface variant
            Some(color(0x48, 0x48, 0x4A)), // pressed
            color(0x1C, 0x1C, 0x1E),       // sunken input
            color(0x2C, 0x2C, 0x2E),       // raised
            color(0x2C, 0x2C, 0x2E),       // floating
            color(0x2C, 0x2C, 0x2E),       // overlay surface
            color(0xF5, 0xF5, 0xF7),       // on background
            color(0xF5, 0xF5, 0xF7),       // on surface
            color(0xB0, 0xB0, 0xB5),       // secondary text
            color(0x9E, 0x9E, 0xA3),       // muted text (>=4.5:1 on cards)
            color(0x18, 0x73, 0xE0),       // accent
            color(0x2A, 0x82, 0xF0),       // accent hover
            color(0x10, 0x5C, 0xB8),       // accent pressed
            color(0xFF, 0xFF, 0xFF),       // on accent
            alpha(0x18, 0x73, 0xE0, 0x3D), // muted accent
            Some(color(0xBF, 0x5A, 0xF2)), // secondary accent
            alpha(0xFF, 0xFF, 0xFF, 0x0D), // hover layer
            alpha(0xFF, 0xFF, 0xFF, 0x1A), // pressed layer
            alpha(0x18, 0x73, 0xE0, 0x3D), // selected layer
            color(0x30, 0xD1, 0x58),       // success
            color(0xFF, 0xB3, 0x40),       // warning
            color(0xFF, 0x69, 0x61),       // error
            color(0x64, 0xB5, 0xFF),       // info
            color(0x3A, 0x3A, 0x3C),       // border
            color(0x63, 0x63, 0x66),       // strong border
            color(0x38, 0x38, 0x3A),       // divider
            alpha(0x64, 0xB5, 0xFF, 0x99), // focus ring
            alpha(0x18, 0x73, 0xE0, 0x66), // text selection
            alpha(0x00, 0x00, 0x00, 0x99), // scrim
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
        // Reference ramp: checks 4, controls 6, popups 8, dialogs 10.
        RadiusScale::new(0.0, 4.0, 6.0, 8.0, 10.0, 9999.0),
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
