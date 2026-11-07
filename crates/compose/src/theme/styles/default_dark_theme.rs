//! macOS dark palette: graphite content, a lighter sidebar and groups, blue accent.
//!
//! Regions separate by one step of fill, the way the macOS window, sidebar and
//! grouped content do. Corners follow the 6/8/12/16 ramp and motion is
//! spring-driven. The accent keeps white labels above 4.5:1.
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
            color(0x1E, 0x1E, 0x1E),       // background
            color(0x2C, 0x2C, 0x2E),       // surface
            color(0x26, 0x26, 0x28),       // surface variant
            Some(color(0x3A, 0x3A, 0x3C)), // pressed
            color(0x1E, 0x1E, 0x1E),       // sunken input
            color(0x2C, 0x2C, 0x2E),       // raised
            color(0x2C, 0x2C, 0x2E),       // floating
            color(0x2C, 0x2C, 0x2E),       // overlay surface
            color(0xF5, 0xF5, 0xF7),       // on background
            color(0xF5, 0xF5, 0xF7),       // on surface
            color(0xAE, 0xAE, 0xB2),       // secondary text
            color(0x98, 0x98, 0x9D),       // muted text (>=4.5:1 on cards)
            color(0x0B, 0x70, 0xE0),       // accent
            color(0x1A, 0x7D, 0xEB),       // accent hover
            color(0x00, 0x62, 0xC4),       // accent pressed
            color(0xFF, 0xFF, 0xFF),       // on accent
            alpha(0x0A, 0x84, 0xFF, 0x33), // muted accent
            Some(color(0xBF, 0x5A, 0xF2)), // secondary accent
            alpha(0xFF, 0xFF, 0xFF, 0x0F), // hover layer
            alpha(0xFF, 0xFF, 0xFF, 0x1F), // pressed layer
            alpha(0x0A, 0x84, 0xFF, 0x40), // selected layer
            color(0x30, 0xD1, 0x58),       // success
            color(0xFF, 0x9F, 0x0A),       // warning
            color(0xFF, 0x45, 0x3A),       // error
            color(0x40, 0x9C, 0xFF),       // info
            color(0x38, 0x38, 0x3A),       // border
            color(0x63, 0x63, 0x66),       // strong border
            color(0x2E, 0x2E, 0x30),       // divider
            alpha(0x40, 0x9C, 0xFF, 0x99), // focus ring
            alpha(0x0A, 0x84, 0xFF, 0x59), // text selection
            alpha(0x00, 0x00, 0x00, 0x8C), // scrim
        ),
        // SF tracking tightens as the size grows; text at body size and below
        // keeps the font's own spacing.
        Typography::new(
            None,
            TextStyleToken::new(Font::system(34.0, FontWeight::Bold), 41.0, -0.9),
            TextStyleToken::new(Font::system(26.0, FontWeight::Bold), 32.0, -0.6),
            TextStyleToken::new(Font::system(17.0, FontWeight::Semibold), 22.0, -0.4),
            TextStyleToken::new(Font::system(14.0, FontWeight::Regular), 20.0, 0.0),
            TextStyleToken::new(Font::system(14.0, FontWeight::Semibold), 20.0, 0.0),
            TextStyleToken::new(Font::system(12.0, FontWeight::Regular), 16.0, 0.0),
            TextStyleToken::new(Font::system(13.0, FontWeight::Medium), 18.0, 0.0),
            TextStyleToken::new(Font::monospaced(13.0, FontWeight::Regular), 20.0, 0.0),
        ),
        SpacingScale::new(4.0, 6.0, 8.0, 12.0, 16.0, 24.0),
        // Continuous ramp: checks 6, controls 8, groups 12, cards and sheets 16.
        RadiusScale::new(0.0, 6.0, 8.0, 12.0, 16.0, 9999.0),
        ElevationScale::new(
            SurfaceShadow::NONE,
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x59)),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 3.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x4D)),
                offset_x: 0.0,
                offset_y: 4.0,
                blur: 14.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x66)),
                offset_x: 0.0,
                offset_y: 10.0,
                blur: 24.0,
            },
            LayeredShadow {
                ambient: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x59)),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur: 48.0,
                },
                key: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x8C)),
                    offset_x: 0.0,
                    offset_y: 10.0,
                    blur: 20.0,
                },
            },
        ),
        MotionScale::new(0.1, 0.22, 0.35, Easing::FLUID, Easing::RESPONSIVE),
        None,
    )
}
