//! Translation of CoveUICore/Adaptive/Environment.swift.
use crate::TextScale;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SizeClass {
    Compact,
    Medium,
    Expanded,
}
pub type WidthClass = SizeClass;
pub type HeightClass = SizeClass;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Portrait,
    Landscape,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Idiom {
    Phone,
    Tablet,
    #[default]
    Desktop,
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
    pub const EXPANDED_MINIMUM_WIDTH: f32 = 1000.0;
    pub const COMPACT_MAXIMUM_HEIGHT: f32 = 600.0;
    pub const EXPANDED_MINIMUM_HEIGHT: f32 = 1000.0;
    fn classify(extent: f32) -> SizeClass {
        if extent < 600.0 {
            SizeClass::Compact
        } else if extent < 1000.0 {
            SizeClass::Medium
        } else {
            SizeClass::Expanded
        }
    }
    pub fn width_class(self) -> WidthClass {
        Self::classify(self.width)
    }
    pub fn height_class(self) -> HeightClass {
        Self::classify(self.height)
    }
    pub fn orientation(self) -> Orientation {
        if self.width >= self.height {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, PartialEq)]
pub struct Environment {
    pub viewport: Viewport,
    pub safe_area: Insets,
    pub input: InputCapabilities,
    pub idiom: Idiom,
    pub text_scale: TextScale,
    pub keyboard: Keyboard,
    pub reduces_motion: bool,
    /// The window shows the platform's blurred material wherever the UI
    /// leaves it clear, so a sidebar can omit its fill.
    pub translucent_window: bool,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            safe_area: Insets::default(),
            input: InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
            idiom: Idiom::Desktop,
            text_scale: TextScale::default(),
            keyboard: Keyboard::default(),
            reduces_motion: false,
            translucent_window: false,
        }
    }
}
impl Environment {
    pub fn minimum_control_extent(&self) -> f32 {
        if self.input.contains(InputCapabilities::TOUCH) {
            44.0
        } else {
            28.0
        }
    }
    pub fn supports_hover(&self) -> bool {
        self.input.contains(InputCapabilities::POINTER)
    }
}
