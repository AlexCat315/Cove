//! Translation of Theme/Styles/Components/DefaultSliderStyle.swift.
//!
//! Built-in style, matching the reference control: a 6-pt track with a neutral
//! empty half and an accent filled half, and a white 20x16 capsule knob with a
//! small key shadow. The knob stays white; the fill carries the accent, and the
//! track reacts to hover / press.
use cove_core::{Color, SurfaceShadow};

use crate::theme::Theme;

use super::slider_style::{SliderAppearance, SliderStyle, SliderStyleConfiguration};

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultSliderStyle;

impl SliderStyle for DefaultSliderStyle {
    fn resolve(&self, configuration: &SliderStyleConfiguration, theme: &Theme) -> SliderAppearance {
        let track = if configuration.is_enabled {
            theme.colors.border
        } else {
            theme.colors.border.multiplied_alpha(0.6)
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
            theme.colors.border_strong
        } else {
            track
        };
        SliderAppearance::new(
            6.0,
            track_color,
            fill,
            16.0,
            Some(20.0),
            knob,
            // Keyboard focus draws a 2-px ring around the white capsule knob.
            if configuration.is_focused {
                Some(theme.colors.focus_ring)
            } else {
                None
            },
            if configuration.is_focused { 2.0 } else { 0.0 },
            Some(SurfaceShadow {
                color: Some(Color::from_rgba8(0, 0, 0, 64)),
                offset_x: 0.0,
                offset_y: 1.0,
                blur: 3.0,
            }),
            3.0,
        )
    }
}
