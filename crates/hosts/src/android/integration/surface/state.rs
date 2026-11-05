//! Translation of Android/Integration/Surface/State.swift.

use cove_core::{
    Environment, Idiom, InputCapabilities, Insets, Keyboard, State, TextScale, Viewport,
};
use cove_ui_compose::theme::Theme;

/// Session geometry survives swapchain recreation. Native updates replace
/// device facts; frame preparation replaces only viewport facts.
pub struct SurfaceState {
    value: State<Environment>,
    system_bar_icon_style: i32,
}

impl SurfaceState {
    pub fn new(scale: f32) -> Self {
        let surface = Environment {
            idiom: Idiom::Phone,
            input: InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
            viewport: Viewport {
                width: 0.0,
                height: 0.0,
                scale: Self::valid_scale(scale),
            },
            ..Environment::default()
        };
        Self {
            value: State::new(surface),
            system_bar_icon_style: -1,
        }
    }

    pub fn value(&self) -> State<Environment> {
        self.value.clone()
    }

    pub fn environment(&self) -> Environment {
        self.value.get()
    }

    /// 1 = dark icons over a light surface, 0 = light icons over a dark
    /// surface, -1 = unknown. The Java host applies it to the system bars.
    pub fn system_bar_icon_style(&self) -> i32 {
        self.system_bar_icon_style
    }

    pub fn scale(&self) -> f32 {
        self.value.get().viewport.scale
    }

    pub fn logical_size(&self) -> (f32, f32) {
        let viewport = self.value.get().viewport;
        (viewport.width, viewport.height)
    }

    pub fn update(
        &self,
        idiom: Idiom,
        safe_area: Insets,
        keyboard: Keyboard,
        text_scale: TextScale,
    ) -> bool {
        let mut surface = self.value.get();
        surface.idiom = idiom;
        surface.safe_area = safe_area.normalized();
        surface.keyboard = Keyboard::new(keyboard.height);
        surface.text_scale = text_scale;
        self.replace(surface)
    }

    pub fn prepare(&self, pixels: (i32, i32), scale: f32) {
        let mut surface = self.value.get();
        surface.viewport.scale = Self::valid_scale(scale);
        surface.viewport.width = pixels.0.max(0) as f32 / surface.viewport.scale;
        surface.viewport.height = pixels.1.max(0) as f32 / surface.viewport.scale;
        let _ = self.replace(surface);
    }

    pub fn update_system_bars(&mut self, theme: Option<&Theme>) {
        let Some(background) = theme.map(|theme| theme.colors.background) else {
            return;
        };
        let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
        self.system_bar_icon_style = if luminance > 0.5 { 1 } else { 0 };
    }

    fn replace(&self, surface: Environment) -> bool {
        if surface == self.value.get() {
            return false;
        }
        self.value.set(surface);
        true
    }

    fn valid_scale(value: f32) -> f32 {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            1.0
        }
    }
}
