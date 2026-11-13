//! Translation of Android/Integration/Surface/State.swift.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::{
    DesignLanguage, DeviceClass, Environment, EnvironmentOverride, EnvironmentRequest,
    InputCapabilities, Insets, Keyboard, Platform, State, TextScale, Viewport,
};
use cove_ui_compose::theme::Theme;

use super::DeviceFacts;

/// Session geometry survives swapchain recreation. Native updates replace
/// device facts; frame preparation replaces only viewport facts. The published
/// environment is the reported one with any DevTools override applied.
#[derive(Clone)]
pub struct SurfaceState {
    shared: Rc<Shared>,
    system_bar_icon_style: i32,
}

struct Shared {
    value: State<Environment>,
    reported: RefCell<Environment>,
    environment_override: RefCell<EnvironmentOverride>,
    /// The surface's top-left in display pixels, as the Activity measured it.
    /// `None` until the first measurement, so nothing reports an assumed origin.
    origin: Cell<Option<(f32, f32)>>,
}

impl Shared {
    fn publish(&self) -> bool {
        let mut surface = self.reported.borrow().clone();
        self.environment_override.borrow().apply(&mut surface);
        if surface == self.value.get() {
            return false;
        }
        self.value.set(surface);
        true
    }
}

/// Replaces reported facts from DevTools. The system owns an Android window's
/// size, so a preset contributes only its facts.
#[derive(Clone)]
pub struct SurfaceOverrides(Rc<Shared>);

impl SurfaceOverrides {
    pub fn request(&self, request: &EnvironmentRequest) -> Result<Environment, String> {
        let (next, _size) = request.resolve(&self.0.environment_override.borrow())?;
        *self.0.environment_override.borrow_mut() = next;
        self.0.publish();
        Ok(self.0.value.get())
    }
}

impl SurfaceState {
    pub fn new(scale: f32) -> Self {
        let surface = Environment {
            platform: Platform::Android,
            design_language: DesignLanguage::Material,
            device_class: DeviceClass::Phone,
            input: InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
            viewport: Viewport {
                width: 0.0,
                height: 0.0,
                scale: Self::valid_scale(scale),
            },
            ..Environment::default()
        };
        Self {
            shared: Rc::new(Shared {
                value: State::new(surface.clone()),
                reported: RefCell::new(surface),
                environment_override: RefCell::new(EnvironmentOverride::default()),
                origin: Cell::new(None),
            }),
            system_bar_icon_style: -1,
        }
    }

    pub fn value(&self) -> State<Environment> {
        self.shared.value.clone()
    }

    pub fn environment(&self) -> Environment {
        self.shared.value.get()
    }

    pub fn overrides(&self) -> SurfaceOverrides {
        SurfaceOverrides(self.shared.clone())
    }

    /// 1 = dark icons over a light surface, 0 = light icons over a dark
    /// surface, -1 = unknown. The Java host applies it to the system bars.
    pub fn system_bar_icon_style(&self) -> i32 {
        self.system_bar_icon_style
    }

    pub fn scale(&self) -> f32 {
        self.shared.value.get().viewport.scale
    }

    pub fn logical_size(&self) -> (f32, f32) {
        let viewport = self.shared.value.get().viewport;
        (viewport.width, viewport.height)
    }

    pub fn update(
        &self,
        device: DeviceFacts,
        safe_area: Insets,
        keyboard: Keyboard,
        text_scale: TextScale,
    ) -> bool {
        {
            let mut surface = self.shared.reported.borrow_mut();
            surface.device_class = device.device_class;
            surface.posture = device.posture;
            surface.display_features = device.display_features;
            surface.safe_area = safe_area.normalized();
            surface.keyboard = Keyboard::new(keyboard.height);
            surface.text_scale = text_scale;
        }
        self.shared.publish()
    }

    /// Replaces the surface's top-left in display pixels. Keeping the last
    /// measured value means a query between layout passes still reports where
    /// the content was drawn rather than dropping to the display origin.
    pub fn set_origin(&self, origin: (f32, f32)) {
        if origin.0.is_finite() && origin.1.is_finite() {
            self.shared.origin.set(Some(origin));
        }
    }

    pub fn origin(&self) -> Option<(f32, f32)> {
        self.shared.origin.get()
    }

    pub fn prepare(&self, pixels: (i32, i32), scale: f32) {
        {
            let mut surface = self.shared.reported.borrow_mut();
            surface.viewport.scale = Self::valid_scale(scale);
            surface.viewport.width = pixels.0.max(0) as f32 / surface.viewport.scale;
            surface.viewport.height = pixels.1.max(0) as f32 / surface.viewport.scale;
        }
        let _ = self.shared.publish();
    }

    pub fn update_system_bars(&mut self, theme: Option<&Theme>) {
        let Some(background) = theme.map(|theme| theme.colors.background) else {
            return;
        };
        let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
        self.system_bar_icon_style = if luminance > 0.5 { 1 } else { 0 };
    }

    fn valid_scale(value: f32) -> f32 {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            1.0
        }
    }
}
