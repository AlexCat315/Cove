//! Neutral desktop light palette with restrained graphite controls.
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
            color(0xFA, 0xFA, 0xF9),       // background
            color(0xF3, 0xF3, 0xF1),       // surface
            color(0xE8, 0xE8, 0xE5),       // surface variant
            Some(color(0xDD, 0xDD, 0xDA)), // pressed
            color(0xFF, 0xFF, 0xFF),       // sunken input
            color(0xFF, 0xFF, 0xFF),       // raised
            color(0xFF, 0xFF, 0xFF),       // floating
            color(0xFF, 0xFF, 0xFF),       // overlay surface
            color(0x20, 0x22, 0x24),       // on background
            color(0x20, 0x22, 0x24),       // on surface
            color(0x55, 0x5B, 0x61),       // secondary text
            color(0x6B, 0x71, 0x77),       // muted text
            color(0x33, 0x3B, 0x43),       // accent
            color(0x25, 0x2C, 0x33),       // accent hover
            color(0x17, 0x1D, 0x23),       // accent pressed
            color(0xFF, 0xFF, 0xFF),       // on accent
            alpha(0x33, 0x3B, 0x43, 0x14), // muted accent
            Some(color(0x60, 0x78, 0x85)), // secondary accent
            alpha(0x20, 0x22, 0x24, 0x09), // hover layer
            alpha(0x20, 0x22, 0x24, 0x12), // pressed layer
            alpha(0x33, 0x3B, 0x43, 0x1C), // selected layer
            color(0x27, 0x85, 0x57),       // success
            color(0xA9, 0x6E, 0x19),       // warning
            color(0xBD, 0x3D, 0x3A),       // error
            color(0x50, 0x6F, 0x8C),       // info
            color(0xD7, 0xD9, 0xD9),       // border
            color(0xA6, 0xAB, 0xB0),       // strong border
            color(0xE6, 0xE7, 0xE5),       // divider
            alpha(0x60, 0x6F, 0x7C, 0x88), // focus ring
            alpha(0x60, 0x7D, 0x9B, 0x45), // text selection
            alpha(0x00, 0x00, 0x00, 0x66), // scrim
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
