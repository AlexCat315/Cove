//! Translation of Theme/Styles/Components/DefaultSliderStyle.swift.
//!
//! Built-in style, matching the macOS control: a 4-pt track with a neutral
//! empty half and an accent filled half, and a white 26x18 capsule knob lifted
//! by a soft shadow. The knob stays white; the fill carries the accent, and the
//! track reacts to hover / press.
use cove_core::{Color, SurfaceShadow};

use crate::theme::Theme;

use super::slider_style::{SliderAppearance, SliderStyle, SliderStyleConfiguration};

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultSliderStyle;

impl SliderStyle for DefaultSliderStyle {
    fn resolve(&self, configuration: &SliderStyleConfiguration, theme: &Theme) -> SliderAppearance {
        let scale = match configuration.size {
            crate::theme::ControlSize::Mini => 0.75,
            crate::theme::ControlSize::Small => 0.875,
            crate::theme::ControlSize::Regular => 1.0,
            crate::theme::ControlSize::Large => 1.25,
        };
        let track = if configuration.is_enabled {
            theme.colors.surface_pressed
        } else {
            theme.colors.surface_pressed.multiplied_alpha(0.6)
        };
        let fill = if configuration.is_enabled {
            theme.colors.accent
        } else {
            theme.colors.border_strong.multiplied_alpha(0.6)
        };
        let knob = if configuration.is_enabled {
            Color::WHITE
        } else {
            Color::WHITE.multiplied_alpha(0.7)
        };
        // The empty track darkens slightly while the knob is held.
        let track_color = if configuration.is_pressed {
            theme.colors.border
        } else {
            track
        };
        SliderAppearance::new(
            4.0 * scale,
            track_color,
            fill,
            18.0 * scale,
            Some(26.0 * scale),
            knob,
            // Keyboard focus draws a 2-px ring; at rest a faint hairline keeps
            // the white knob legible on white cards.
            if configuration.is_focused {
                Some(theme.colors.focus_ring)
            } else {
                Some(Color::from_rgba8(0, 0, 0, 0x1A))
            },
            if configuration.is_focused { 2.0 } else { 0.5 },
            Some(SurfaceShadow {
                color: Some(Color::from_rgba8(0, 0, 0, 0x38)),
                offset_x: 0.0,
                offset_y: 2.0,
                blur: 5.0,
            }),
            2.0 * scale,
        )
    }
}
