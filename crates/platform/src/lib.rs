//! Platform-neutral input and native menu contracts. No window-system dependency.
use serde::{Deserialize, Serialize};
mod resources;
pub use resources::ResourceBundle;

/// One pixel read from the screen: sRGB components in `0…1` with straight
/// (unpremultiplied) alpha. This crate only defines the contract; reading a
/// screen pixel is a host/backend responsibility with one native implementation
/// per window system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ScreenColor {
    pub const CLEAR: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        )
    }

    pub fn rgba8(self) -> [u8; 4] {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [byte(self.r), byte(self.g), byte(self.b), byte(self.a)]
    }

    /// Backends may report out-of-gamut floats; keep the contract in `0…1`.
    pub fn clamped(self) -> Self {
        Self::new(
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
            self.a.clamp(0.0, 1.0),
        )
    }
}

/// The window-system backend that answers a screen-colour request.
///
/// This is a *description*, not an implementation: `cove-platform` has no
/// window-system dependency and never links a native library. The host picks
/// one backend per window system and reports it so UI affordances can be hidden
/// where sampling is impossible.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenColorBackend {
    /// macOS `NSColorSampler` (AppKit, macOS 10.15+).
    AppKit,
    /// Win32 screen-DC pixel read on a worker thread.
    Win32,
    /// X11 `XGetImage` screenshot-and-sample on a worker thread.
    X11,
    /// `org.freedesktop.portal.Screenshot.PickColor` on Wayland desktops.
    XdgPortal,
    /// The platform exposes no way to read a screen pixel.
    #[default]
    None,
}
impl ScreenColorBackend {
    pub fn is_available(self) -> bool {
        !matches!(self, Self::None)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::AppKit => "NSColorSampler",
            Self::Win32 => "Win32 screen DC",
            Self::X11 => "X11 XGetImage",
            Self::XdgPortal => "XDG PickColor portal",
            Self::None => "unsupported",
        }
    }
}

/// The single answer to a screen-colour request.
///
/// A request never blocks: the backend starts the picker and reports exactly
/// once, later, through the completion. Cancellation and "no backend" are
/// distinct outcomes instead of one opaque `None`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScreenColorSample {
    Picked(ScreenColor),
    Cancelled,
    Unsupported(String),
    Failed(String),
}
impl ScreenColorSample {
    /// The colour to write, if the user actually picked one.
    pub fn color(&self) -> Option<ScreenColor> {
        match self {
            Self::Picked(color) => Some(color.clamped()),
            _ => None,
        }
    }
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// Delivers a screen-colour result. `FnOnce` because a request has one answer;
/// the backend must invoke it exactly once, never on the platform thread while
/// it is inside a blocking native call.
pub type ScreenColorCompletion = Box<dyn FnOnce(ScreenColorSample)>;

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
    /// No modifier is held.
    pub fn is_empty(self) -> bool {
        self.0 == 0
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
    /// Selection offsets in Unicode scalars, matching SDL's UTF-32 positions.
    pub start: i32,
    pub length: i32,
}

impl TextEditingEvent {
    /// Normalize native UTF-16 positions before delivery to scalar-based input
    /// handlers. Windows SDL uses WCHAR positions; Cocoa/X11/Wayland SDL
    /// already publish scalar positions.
    pub fn from_utf16(text: impl Into<String>, start: i32, length: i32) -> Self {
        let text = text.into();
        if start < 0 {
            return Self {
                text,
                start,
                length,
            };
        }
        let scalar = |target: i32, round_up: bool| {
            let mut units = 0;
            let mut index = 0;
            for character in text.chars() {
                let next = units + character.len_utf16() as i32;
                if target < next {
                    return index + i32::from(round_up && target > units);
                }
                units = next;
                index += 1;
            }
            index
        };
        let first = scalar(start, false);
        let length = if length < 0 {
            length
        } else if length == 0 {
            0
        } else {
            scalar(start.saturating_add(length), true) - first
        };
        Self {
            text,
            start: first,
            length,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextInputArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub cursor_x: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MouseButton {
    #[default]
    Left = 1,
    Middle = 2,
    Right = 3,
    X1 = 4,
    X2 = 5,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PointerKind {
    #[default]
    Mouse = 1,
    Touch = 2,
    Pen = 3,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MouseMotionEvent {
    pub x: f32,
    pub y: f32,
    pub delta_x: f32,
    pub delta_y: f32,
    #[serde(default)]
    pub kind: PointerKind,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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

/// One step of a drag that started outside the application.
///
/// The OS delivers drops as a sequence: `Begin` announces the drag, `File` and
/// `Text` carry each item, `Position` tracks the pointer while it moves over a
/// window, and `Complete` ends the sequence. A platform that cannot report a
/// position still reports the rest, so hosts read `position` as advisory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DropPhase {
    #[default]
    Begin,
    File,
    Text,
    Position,
    Complete,
}

/// A dropped item and where it landed.
///
/// `paths` holds filesystem paths for [`DropPhase::File`] and is empty
/// otherwise; `text` holds plain text for [`DropPhase::Text`]. Both stay empty
/// for `Begin`, `Position` and `Complete`. `x`/`y` are window coordinates in
/// logical points, the same space as pointer events.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExternalDropEvent {
    pub phase: DropPhase,
    pub paths: Vec<String>,
    pub text: Option<String>,
    pub x: f32,
    pub y: f32,
    /// The application that started the drag, when the platform reports one.
    pub source: Option<String>,
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
    /// A drag that started outside the application. Window identity travels in
    /// [`WindowInputEvent::window_id`], so it is not repeated here.
    ExternalDrop(ExternalDropEvent),
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
    WindowResized {
        width: i32,
        height: i32,
    },
    WindowPixelSizeChanged {
        width: i32,
        height: i32,
    },
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

/// Labels for the entries the platform owns in the application menu. An
/// application supplies only the leading items, such as About and Settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeApplicationMenuTitles {
    pub services: String,
    pub hide: String,
    pub hide_others: String,
    pub show_all: String,
    pub quit: String,
}

impl NativeApplicationMenuTitles {
    pub fn for_app_name(app_name: &str) -> Self {
        Self {
            services: "Services".to_owned(),
            hide: format!("Hide {app_name}"),
            hide_others: "Hide Others".to_owned(),
            show_all: "Show All".to_owned(),
            quit: format!("Quit {app_name}"),
        }
    }
}

/// Labels of the Edit and Window menus the platform appends so that key
/// equivalents reach native controls — a save panel's name field has no
/// Cut/Copy/Paste/Select All without an Edit menu in the bar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeStandardMenuTitles {
    pub edit: String,
    pub undo: String,
    pub redo: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub delete: String,
    pub select_all: String,
    pub window: String,
    pub minimize: String,
    pub zoom: String,
    pub bring_all_to_front: String,
}

impl Default for NativeStandardMenuTitles {
    fn default() -> Self {
        Self {
            edit: "Edit".to_owned(),
            undo: "Undo".to_owned(),
            redo: "Redo".to_owned(),
            cut: "Cut".to_owned(),
            copy: "Copy".to_owned(),
            paste: "Paste".to_owned(),
            delete: "Delete".to_owned(),
            select_all: "Select All".to_owned(),
            window: "Window".to_owned(),
            minimize: "Minimize".to_owned(),
            zoom: "Zoom".to_owned(),
            bring_all_to_front: "Bring All to Front".to_owned(),
        }
    }
}

/// A titled top-level menu of the application bar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeMenuBranch {
    pub title: String,
    pub items: NativeMenuSnapshot,
}

/// The whole application menu bar. Ids in `application_items` and `menus` share
/// one space so a chosen id resolves to a single authored action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeMenuBarSnapshot {
    pub app_name: String,
    pub application_titles: NativeApplicationMenuTitles,
    pub standard_titles: NativeStandardMenuTitles,
    pub application_items: NativeMenuSnapshot,
    pub menus: Vec<NativeMenuBranch>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_composition_positions_normalize_before_dispatch() {
        let event = TextEditingEvent::from_utf16("👩🏽‍💻e\u{301}中", 7, 2);
        assert_eq!((event.start, event.length), (4, 2));
        let caret = TextEditingEvent::from_utf16("😀x", 1, 0);
        assert_eq!((caret.start, caret.length), (0, 0));
        let selected = TextEditingEvent::from_utf16("😀x", 0, 1);
        assert_eq!((selected.start, selected.length), (0, 1));
        let unknown = TextEditingEvent::from_utf16("text", -1, -1);
        assert_eq!((unknown.start, unknown.length), (-1, -1));
    }
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
    fn screen_color_round_trips_as_srgba_bytes_and_clamps() {
        let color = ScreenColor::from_rgba8(0x12, 0x34, 0x56, 0x78);
        assert_eq!(color.rgba8(), [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(
            ScreenColor::new(-0.5, 1.5, 0.25, 1.0).clamped(),
            ScreenColor::new(0.0, 1.0, 0.25, 1.0)
        );
        let data = serde_json::to_vec(&color).unwrap();
        assert_eq!(serde_json::from_slice::<ScreenColor>(&data).unwrap(), color);
    }
    #[test]
    fn screen_color_sample_distinguishes_pick_cancel_and_unsupported() {
        let picked = ScreenColorSample::Picked(ScreenColor::new(-0.2, 0.5, 1.4, 1.0));
        assert_eq!(picked.color(), Some(ScreenColor::new(0.0, 0.5, 1.0, 1.0)));
        for sample in [
            ScreenColorSample::Cancelled,
            ScreenColorSample::Unsupported("no portal".into()),
            ScreenColorSample::Failed("XOpenDisplay failed".into()),
        ] {
            assert_eq!(sample.color(), None);
            let data = serde_json::to_vec(&sample).unwrap();
            assert_eq!(
                serde_json::from_slice::<ScreenColorSample>(&data).unwrap(),
                sample
            );
        }
        assert!(ScreenColorSample::Cancelled.is_cancelled());
        assert!(!picked.is_cancelled());
        assert!(ScreenColorBackend::AppKit.is_available());
        assert!(!ScreenColorBackend::None.is_available());
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
