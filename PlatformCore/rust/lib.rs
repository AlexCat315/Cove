//! Platform-neutral input and native menu contracts. No window-system dependency.
use serde::{Deserialize, Serialize};
mod resources;
pub use resources::ResourceBundle;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u32);
impl WindowId {
    pub const MAIN: Self = Self(0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyModifiers(pub u16);
impl KeyModifiers {
    pub const LEFT_SHIFT: Self = Self(1);
    pub const RIGHT_SHIFT: Self = Self(2);
    pub const LEFT_CTRL: Self = Self(4);
    pub const RIGHT_CTRL: Self = Self(8);
    pub const LEFT_ALT: Self = Self(16);
    pub const RIGHT_ALT: Self = Self(32);
    pub const LEFT_GUI: Self = Self(64);
    pub const RIGHT_GUI: Self = Self(128);
    pub fn has_shift(self) -> bool {
        self.0 & 3 != 0
    }
    pub fn has_ctrl(self) -> bool {
        self.0 & 12 != 0
    }
    pub fn has_alt(self) -> bool {
        self.0 & 48 != 0
    }
    pub fn has_gui(self) -> bool {
        self.0 & 192 != 0
    }
}
impl std::ops::BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub scancode: u32,
    pub keycode: u32,
    pub modifiers: KeyModifiers,
    pub is_repeat: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextEditingEvent {
    pub text: String,
    pub start: i32,
    pub length: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextInputArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub cursor_x: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    X1,
    X2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerKind {
    #[default]
    Mouse,
    Touch,
    Pen,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseMotionEvent {
    pub x: f32,
    pub y: f32,
    pub delta_x: f32,
    pub delta_y: f32,
    #[serde(default)]
    pub kind: PointerKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseButtonEvent {
    pub button: MouseButton,
    pub x: f32,
    pub y: f32,
    pub clicks: u8,
    #[serde(default)]
    pub modifiers: KeyModifiers,
    #[serde(default)]
    pub kind: PointerKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollUnit {
    Points,
    #[default]
    Lines,
    Pages,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollDevice {
    #[default]
    Unknown,
    Wheel,
    Continuous,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollPhase {
    #[default]
    None,
    MayBegin,
    Began,
    Changed,
    Stationary,
    Ended,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollInput {
    pub unit: ScrollUnit,
    pub device: ScrollDevice,
    pub phase: ScrollPhase,
    pub momentum_phase: ScrollPhase,
    pub timestamp: f64,
}
impl ScrollInput {
    pub fn is_continuation(self) -> bool {
        self.momentum_phase != ScrollPhase::None
            || matches!(
                self.phase,
                ScrollPhase::Changed
                    | ScrollPhase::Stationary
                    | ScrollPhase::Ended
                    | ScrollPhase::Cancelled
            )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MouseWheelEvent {
    pub x: f32,
    pub y: f32,
    pub mouse_x: Option<f32>,
    pub mouse_y: Option<f32>,
    #[serde(default)]
    pub input: ScrollInput,
}
impl MouseWheelEvent {
    pub fn displacement(&self, line_step: f32, page_width: f32, page_height: f32) -> (f32, f32) {
        match self.input.unit {
            ScrollUnit::Points => (self.x, self.y),
            ScrollUnit::Lines => (self.x * line_step, self.y * line_step),
            ScrollUnit::Pages => (self.x * page_width, self.y * page_height),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GamepadButtonEvent {
    pub gamepad_id: u32,
    pub button: GamepadButton,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GamepadAxisEvent {
    pub gamepad_id: u32,
    pub axis: GamepadAxis,
    pub value: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
    TextInput(String),
    TextEditing(TextEditingEvent),
    MouseMotion(MouseMotionEvent),
    MouseButtonDown(MouseButtonEvent),
    MouseButtonUp(MouseButtonEvent),
    MouseWheel(MouseWheelEvent),
    GamepadButtonDown(GamepadButtonEvent),
    GamepadButtonUp(GamepadButtonEvent),
    GamepadAxisMotion(GamepadAxisEvent),
    GamepadAdded(u32),
    GamepadRemoved(u32),
    WindowFocusGained,
    WindowFocusLost,
    WindowMinimized,
    WindowRestored,
    WindowOccluded,
    WindowExposed,
    WindowResized { width: i32, height: i32 },
    WindowPixelSizeChanged { width: i32, height: i32 },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowInputEvent {
    pub window_id: WindowId,
    pub event: InputEvent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemCursor {
    #[default]
    Arrow,
    Pointer,
    Ibeam,
    Crosshair,
    Wait,
    Progress,
    NotAllowed,
    Move,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNwse,
    ResizeNesw,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NativeMenuSnapshot {
    pub items: Vec<NativeMenuItem>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NativeMenuKind {
    Regular,
    Separator,
    Submenu(NativeMenuSnapshot),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeMenuItem {
    pub id: i64,
    pub kind: NativeMenuKind,
    pub title: String,
    pub enabled: bool,
    pub checked: bool,
    pub shortcut: Option<NativeShortcut>,
    pub icon: Option<NativeMenuIcon>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NativeShortcut {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeMenuIcon {
    pub png: Vec<u8>,
    pub is_template: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn either_side_of_modifier_matches_chord() {
        for modifiers in [
            KeyModifiers::LEFT_GUI,
            KeyModifiers::RIGHT_GUI,
            KeyModifiers(192),
        ] {
            assert!(modifiers.has_gui());
            assert!(!modifiers.has_ctrl());
        }
    }
    #[test]
    fn scroll_units_and_momentum_are_not_density_dependent() {
        let mut wheel = MouseWheelEvent {
            x: -2.0,
            y: 3.0,
            ..Default::default()
        };
        assert_eq!(wheel.displacement(20.0, 100.0, 200.0), (-40.0, 60.0));
        wheel.input.unit = ScrollUnit::Points;
        assert_eq!(wheel.displacement(20.0, 100.0, 200.0), (-2.0, 3.0));
        wheel.input.unit = ScrollUnit::Pages;
        assert_eq!(wheel.displacement(20.0, 100.0, 200.0), (-200.0, 600.0));
        wheel.input.momentum_phase = ScrollPhase::Began;
        assert!(wheel.input.is_continuation());
    }
    #[test]
    fn input_round_trip_preserves_unicode_ime_and_window() {
        let event = WindowInputEvent {
            window_id: WindowId(7),
            event: InputEvent::TextEditing(TextEditingEvent {
                text: "你好🦀".into(),
                start: 2,
                length: 1,
            }),
        };
        let data = serde_json::to_vec(&event).unwrap();
        assert_eq!(
            serde_json::from_slice::<WindowInputEvent>(&data).unwrap(),
            event
        );
    }
}
