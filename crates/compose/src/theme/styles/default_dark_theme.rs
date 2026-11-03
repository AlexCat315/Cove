//! Neutral desktop dark palette with layered charcoal surfaces.
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
            color(0x16, 0x18, 0x19),       // background
            color(0x1E, 0x21, 0x23),       // surface
            color(0x29, 0x2D, 0x30),       // surface variant
            Some(color(0x34, 0x3A, 0x3F)), // pressed
            color(0x19, 0x1B, 0x1D),       // sunken input
            color(0x24, 0x28, 0x2B),       // raised
            color(0x25, 0x29, 0x2D),       // floating
            color(0x22, 0x26, 0x29),       // overlay surface
            color(0xE9, 0xEB, 0xED),       // on background
            color(0xE9, 0xEB, 0xED),       // on surface
            color(0xB4, 0xBA, 0xC0),       // secondary text
            color(0x96, 0x9F, 0xA7),       // muted text
            color(0xD3, 0xDA, 0xDF),       // accent
            color(0xE5, 0xEA, 0xEE),       // accent hover
            color(0xBC, 0xC5, 0xCD),       // accent pressed
            color(0x1B, 0x22, 0x28),       // on accent
            alpha(0xD3, 0xDA, 0xDF, 0x20), // muted accent
            Some(color(0x9C, 0xAE, 0xB9)), // secondary accent
            alpha(0xFF, 0xFF, 0xFF, 0x0D), // hover layer
            alpha(0xFF, 0xFF, 0xFF, 0x18), // pressed layer
            alpha(0xD3, 0xDA, 0xDF, 0x24), // selected layer
            color(0x6A, 0xB5, 0x8B),       // success
            color(0xD3, 0xA3, 0x56),       // warning
            color(0xE7, 0x83, 0x7A),       // error
            color(0x94, 0xAD, 0xBF),       // info
            color(0x3A, 0x41, 0x46),       // border
            color(0x60, 0x6A, 0x73),       // strong border
            color(0x2C, 0x32, 0x36),       // divider
            alpha(0xA7, 0xB8, 0xC7, 0x80), // focus ring
            alpha(0x9B, 0xB3, 0xC0, 0x5C), // text selection
            alpha(0x00, 0x00, 0x00, 0x80), // scrim
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
        RadiusScale::new(0.0, 4.0, 6.0, 8.0, 12.0, 9999.0),
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
