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

    /// Fill for an editable input in the given interaction state.
    pub fn background_for(&self, state: InputChromeState) -> Color {
        if state.is_enabled {
            self.background
        } else {
            self.background_disabled
        }
    }

    /// Border tone for an editable input. Error beats focus, focus beats hover,
    /// in the order the whole field set shares.
    pub fn border_for(&self, state: InputChromeState) -> Color {
        if !state.is_enabled {
            self.border_disabled
        } else if state.is_error {
            self.border_error
        } else if state.is_focused {
            self.border_focused
        } else if state.is_hovered {
            self.border_hover
        } else {
            self.border_color
        }
    }

    /// Border thickness for an editable input: the focus ring replaces the hairline.
    pub fn border_width_for(&self, state: InputChromeState) -> f32 {
        if state.is_focused {
            self.focus_ring_width
        } else {
            self.border_width
        }
    }
}

/// Interaction snapshot every field shell resolves its chrome from. One type so
/// `TextField`, the numeric prefix, `Select`, `TimeInput` and the JSON editor all
/// draw the same fill, border and radius instead of re-deriving them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct InputChromeState {
    pub is_enabled: bool,
    pub is_focused: bool,
    pub is_hovered: bool,
    pub is_error: bool,
}

impl InputChromeState {
    pub fn new(is_enabled: bool) -> Self {
        Self {
            is_enabled,
            ..Default::default()
        }
    }

    pub fn focused(mut self, is_focused: bool) -> Self {
        self.is_focused = is_focused;
        self
    }

    pub fn hovered(mut self, is_hovered: bool) -> Self {
        self.is_hovered = is_hovered;
        self
    }

    pub fn error(mut self, is_error: bool) -> Self {
        self.is_error = is_error;
        self
    }
}

impl From<&super::text_field_style::TextFieldStyleConfiguration> for InputChromeState {
    fn from(configuration: &super::text_field_style::TextFieldStyleConfiguration) -> Self {
        Self {
            is_enabled: configuration.is_enabled,
            is_focused: configuration.is_focused,
            is_hovered: configuration.is_hovered,
            is_error: configuration.is_error,
        }
    }
}
