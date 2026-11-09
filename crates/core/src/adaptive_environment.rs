//! Translation of CoveUICore/Adaptive/Environment.swift.
use serde::{Deserialize, Serialize};

use crate::{
    DesignLanguage, DeviceClass, DisplayFeature, Platform, Posture, Rect, TextScale,
    display_features,
};

/// Material 3 window width classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidthClass {
    Compact,
    Medium,
    Expanded,
    Large,
    ExtraLarge,
}

impl WidthClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Expanded => "expanded",
            Self::Large => "large",
            Self::ExtraLarge => "extra-large",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeightClass {
    Compact,
    Medium,
    Expanded,
}

impl HeightClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Expanded => "expanded",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 720.0,
            scale: 1.0,
        }
    }
}
impl Viewport {
    pub const COMPACT_MAXIMUM_WIDTH: f32 = 600.0;
    pub const MEDIUM_MAXIMUM_WIDTH: f32 = 840.0;
    pub const EXPANDED_MAXIMUM_WIDTH: f32 = 1200.0;
    pub const LARGE_MAXIMUM_WIDTH: f32 = 1600.0;
    /// First width that fits two panes side by side.
    pub const EXPANDED_MINIMUM_WIDTH: f32 = Self::MEDIUM_MAXIMUM_WIDTH;
    pub const COMPACT_MAXIMUM_HEIGHT: f32 = 480.0;
    pub const EXPANDED_MINIMUM_HEIGHT: f32 = 900.0;

    pub fn width_class(self) -> WidthClass {
        let width = self.width;
        if width < Self::COMPACT_MAXIMUM_WIDTH {
            WidthClass::Compact
        } else if width < Self::MEDIUM_MAXIMUM_WIDTH {
            WidthClass::Medium
        } else if width < Self::EXPANDED_MAXIMUM_WIDTH {
            WidthClass::Expanded
        } else if width < Self::LARGE_MAXIMUM_WIDTH {
            WidthClass::Large
        } else {
            WidthClass::ExtraLarge
        }
    }
    pub fn height_class(self) -> HeightClass {
        if self.height < Self::COMPACT_MAXIMUM_HEIGHT {
            HeightClass::Compact
        } else if self.height < Self::EXPANDED_MINIMUM_HEIGHT {
            HeightClass::Medium
        } else {
            HeightClass::Expanded
        }
    }
    pub fn orientation(self) -> Orientation {
        if self.width >= self.height {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        }
    }
    pub fn bounds(self) -> Rect {
        Rect::new(0.0, 0.0, self.width.max(0.0), self.height.max(0.0))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Insets {
    pub top: f32,
    pub bottom: f32,
    pub leading: f32,
    pub trailing: f32,
}
impl Insets {
    pub fn normalized(self) -> Self {
        let normalize = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        Self {
            top: normalize(self.top),
            bottom: normalize(self.bottom),
            leading: normalize(self.leading),
            trailing: normalize(self.trailing),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keyboard {
    pub height: f32,
}
impl Keyboard {
    pub fn new(height: f32) -> Self {
        Self {
            height: if height.is_finite() {
                height.max(0.0)
            } else {
                0.0
            },
        }
    }
    pub fn is_visible(self) -> bool {
        self.height.is_finite() && self.height > 0.0
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InputCapabilities(pub u8);
impl InputCapabilities {
    pub const TOUCH: Self = Self(1);
    pub const POINTER: Self = Self(2);
    pub const KEYBOARD: Self = Self(4);
    pub const PEN: Self = Self(8);
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl std::ops::BitOr for InputCapabilities {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
/// The operating system's light/dark appearance, as the host reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SystemAppearance {
    /// The platform reports none; applications pick their own default.
    #[default]
    Unknown,
    Light,
    Dark,
}

impl SystemAppearance {
    pub fn is_dark(self) -> Option<bool> {
        match self {
            Self::Unknown => None,
            Self::Light => Some(false),
            Self::Dark => Some(true),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Environment {
    pub viewport: Viewport,
    pub safe_area: Insets,
    pub input: InputCapabilities,
    /// Conventions to follow; constant for a process unless overridden.
    pub platform: Platform,
    pub design_language: DesignLanguage,
    /// Hardware class; constant through folds and rotations.
    pub device_class: DeviceClass,
    pub posture: Posture,
    /// Folds and hinges in window coordinates.
    pub display_features: Vec<DisplayFeature>,
    pub text_scale: TextScale,
    pub keyboard: Keyboard,
    pub reduces_motion: bool,
    /// The window shows the platform's blurred material wherever the UI
    /// leaves it clear, so a sidebar can omit its fill.
    pub translucent_window: bool,
    /// Follows the system setting live; content decides whether to honor it.
    pub system_appearance: SystemAppearance,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            safe_area: Insets::default(),
            input: InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            platform: Platform::current(),
            design_language: Platform::current().design_language(),
            device_class: DeviceClass::Desktop,
            posture: Posture::Flat,
            display_features: Vec::new(),
            text_scale: TextScale::default(),
            keyboard: Keyboard::default(),
            reduces_motion: false,
            translucent_window: false,
            system_appearance: SystemAppearance::Unknown,
        }
    }
}
impl Environment {
    pub fn width_class(&self) -> WidthClass {
        self.viewport.width_class()
    }

    pub fn height_class(&self) -> HeightClass {
        self.viewport.height_class()
    }

    pub fn minimum_control_extent(&self) -> f32 {
        if self.input.contains(InputCapabilities::TOUCH) {
            self.design_language.touch_target()
        } else {
            28.0
        }
    }

    pub fn supports_hover(&self) -> bool {
        self.input.contains(InputCapabilities::POINTER)
    }

    /// Logical window regions separated by folds and hinges; one region when
    /// nothing divides the window.
    pub fn segments(&self) -> Vec<Rect> {
        display_features::window_segments(self.viewport.bounds(), &self.display_features)
    }

    /// Whether a fold or hinge divides the window into more than one segment.
    pub fn is_spanned(&self) -> bool {
        self.segments().len() > 1
    }

    /// The segment that should host `rect`, so overlays never straddle a fold.
    pub fn segment_for(&self, rect: Rect) -> Rect {
        display_features::segment_for(&self.segments(), rect).unwrap_or(self.viewport.bounds())
    }

    /// Regions where nothing can be drawn.
    pub fn occluded_regions(&self) -> impl Iterator<Item = Rect> + '_ {
        self.display_features
            .iter()
            .filter(|feature| feature.occludes)
            .map(|feature| feature.bounds)
    }
}
