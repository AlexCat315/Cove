//! Translation of Theme/Styles/Components/InputAppearance.swift.
use cove_core::Color;

/// Token group for editable text inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputAppearance {
    pub background: Color,
    pub background_disabled: Color,

    pub border_color: Color,
    pub border_hover: Color,
    pub border_focused: Color,
    pub border_error: Color,
    pub border_disabled: Color,

    pub border_width: f32,
    pub focus_ring_width: f32,

    pub addon_background: Color,
    pub addon_foreground: Color,
    pub divider_color: Color,

    pub radius: f32,
}
#[allow(clippy::too_many_arguments)]
impl InputAppearance {
    pub fn new(
        background: Color,
        background_disabled: Color,
        border_color: Color,
        border_hover: Color,
        border_focused: Color,
        border_error: Color,
        border_disabled: Color,
        border_width: f32,
        focus_ring_width: f32,
        addon_background: Color,
        addon_foreground: Color,
        divider_color: Color,
        radius: f32,
    ) -> Self {
        Self {
            background,
            background_disabled,
            border_color,
            border_hover,
            border_focused,
            border_error,
            border_disabled,
            border_width,
            focus_ring_width,
            addon_background,
            addon_foreground,
            divider_color,
            radius,
        }
    }
}
