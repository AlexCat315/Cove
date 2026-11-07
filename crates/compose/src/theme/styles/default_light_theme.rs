//! macOS light palette: white content, light-gray sidebar and groups, system blue.
//!
//! The window content is white; the sidebar and grouped content share the
//! light-gray `surface_variant`, so regions separate by fill rather than by
//! outlines or shadows. Text uses Apple's label hierarchy and the accent keeps
//! white labels above 4.5:1.
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
            color(0xFF, 0xFF, 0xFF),       // background
            color(0xFF, 0xFF, 0xFF),       // surface
            color(0xF2, 0xF2, 0xF4),       // surface variant
            Some(color(0xE3, 0xE3, 0xE8)), // pressed
            color(0xFF, 0xFF, 0xFF),       // sunken input
            color(0xFF, 0xFF, 0xFF),       // raised
            color(0xFF, 0xFF, 0xFF),       // floating
            color(0xFF, 0xFF, 0xFF),       // overlay surface
            color(0x1D, 0x1D, 0x1F),       // on background
            color(0x1D, 0x1D, 0x1F),       // on surface
            color(0x42, 0x42, 0x45),       // secondary text
            color(0x6E, 0x6E, 0x73),       // muted text (>=4.5:1 on the canvas)
            color(0x00, 0x71, 0xE3),       // accent
            color(0x00, 0x77, 0xED),       // accent hover
            color(0x00, 0x62, 0xC4),       // accent pressed
            color(0xFF, 0xFF, 0xFF),       // on accent
            alpha(0x00, 0x71, 0xE3, 0x1F), // muted accent
            Some(color(0xAF, 0x52, 0xDE)), // secondary accent
            alpha(0x00, 0x00, 0x00, 0x0A), // hover layer
            alpha(0x00, 0x00, 0x00, 0x14), // pressed layer
            alpha(0x00, 0x71, 0xE3, 0x24), // selected layer
            color(0x24, 0x8A, 0x3D),       // success
            color(0xC9, 0x34, 0x00),       // warning
            color(0xD7, 0x00, 0x15),       // error
            color(0x00, 0x71, 0xE3),       // info
            color(0xD2, 0xD2, 0xD7),       // border
            color(0x86, 0x86, 0x8B),       // strong border
            color(0xE8, 0xE8, 0xED),       // divider
            alpha(0x00, 0x71, 0xE3, 0x80), // focus ring
            alpha(0x00, 0x71, 0xE3, 0x33), // text selection
            alpha(0x00, 0x00, 0x00, 0x4D), // scrim
        ),
        dark.typography.clone(),
        dark.spacing,
        dark.radius,
        ElevationScale::new(
            SurfaceShadow::NONE,
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x14)),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 3.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x12)),
                offset_x: 0.0,
                offset_y: 4.0,
                blur: 14.0,
            },
            SurfaceShadow {
                color: Some(alpha(0, 0, 0, 0x1F)),
                offset_x: 0.0,
                offset_y: 10.0,
                blur: 24.0,
            },
            LayeredShadow {
                ambient: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x12)),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur: 48.0,
                },
                key: SurfaceShadow {
                    color: Some(alpha(0, 0, 0, 0x29)),
                    offset_x: 0.0,
                    offset_y: 10.0,
                    blur: 20.0,
                },
            },
        ),
        dark.motion,
        None,
    )
}
