//! Pointer event encoding for programs that track the mouse.
//!
//! The tracking mode and wire format (X10, UTF-8, SGR, urxvt, SGR-pixels) come
//! from the terminal's live modes; Ghostty's encoder owns that mapping.

use crate::engine::TerminalEngine;
use crate::engine::ffi::{self, GhosttyMouseEncoder, GhosttyMouseEvent};
use crate::engine::key_encoder::KeyModifiers;
use std::ptr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

/// A pointer event as reported to the program (xterm mouse tracking).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TerminalMouseEvent {
    pub action: MouseAction,
    /// `None` for motion with no button held.
    pub button: Option<MouseButton>,
    /// Points from the grid's top-left corner.
    pub x: f32,
    pub y: f32,
    pub modifiers: KeyModifiers,
}

impl TerminalMouseEvent {
    pub fn new(action: MouseAction, button: Option<MouseButton>, x: f32, y: f32) -> Self {
        Self { action, button, x, y, modifiers: KeyModifiers::empty() }
    }

    fn ghostty_action(self) -> i32 {
        match self.action {
            MouseAction::Press => ffi::MOUSE_ACTION_PRESS,
            MouseAction::Release => ffi::MOUSE_ACTION_RELEASE,
            MouseAction::Motion => ffi::MOUSE_ACTION_MOTION,
        }
    }

    fn ghostty_button(button: MouseButton) -> i32 {
        match button {
            MouseButton::Left => ffi::MOUSE_BUTTON_LEFT,
            MouseButton::Middle => ffi::MOUSE_BUTTON_MIDDLE,
            MouseButton::Right => ffi::MOUSE_BUTTON_RIGHT,
            MouseButton::WheelUp => ffi::MOUSE_BUTTON_FOUR,
            MouseButton::WheelDown => ffi::MOUSE_BUTTON_FIVE,
            MouseButton::WheelLeft => ffi::MOUSE_BUTTON_SIX,
            MouseButton::WheelRight => ffi::MOUSE_BUTTON_SEVEN,
        }
    }

    fn ghostty_mods(self) -> ffi::GhosttyMods {
        let mut mods: ffi::GhosttyMods = 0;
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            mods |= ffi::MODS_SHIFT;
        }
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            mods |= ffi::MODS_CTRL;
        }
        if self.modifiers.contains(KeyModifiers::OPTION) {
            mods |= ffi::MODS_ALT;
        }
        mods
    }
}

/// Wraps Ghostty's mouse encoder.
pub(crate) struct MouseEncoder {
    encoder: GhosttyMouseEncoder,
    event: GhosttyMouseEvent,
}

impl MouseEncoder {
    pub(crate) fn new() -> Self {
        // SAFETY: both allocations are owned by this value and freed in `Drop`.
        unsafe {
            let mut encoder: GhosttyMouseEncoder = ptr::null_mut();
            let mut event: GhosttyMouseEvent = ptr::null_mut();
            let encoder_ok = ffi::ghostty_mouse_encoder_new(ptr::null(), &mut encoder) == ffi::GHOSTTY_SUCCESS;
            let event_ok = ffi::ghostty_mouse_event_new(ptr::null(), &mut event) == ffi::GHOSTTY_SUCCESS;
            Self { encoder: if encoder_ok { encoder } else { ptr::null_mut() }, event: if event_ok { event } else { ptr::null_mut() } }
        }
    }

    pub(crate) fn encode(
        &self,
        mouse: &TerminalMouseEvent,
        terminal: ffi::GhosttyTerminal,
        columns: usize,
        rows: usize,
        cell_width: usize,
        cell_height: usize,
    ) -> Vec<u8> {
        if self.encoder.is_null() || self.event.is_null() {
            return Vec::new();
        }
        // SAFETY: encoder and event are live; every pointer argument points at a
        // local of the documented type for the duration of the call.
        unsafe {
            ffi::ghostty_mouse_encoder_setopt_from_terminal(self.encoder, terminal);
            let mut size = ffi::sized_mouse_size();
            size.screen_width = (columns * cell_width) as u32;
            size.screen_height = (rows * cell_height) as u32;
            size.cell_width = cell_width as u32;
            size.cell_height = cell_height as u32;
            ffi::ghostty_mouse_encoder_setopt(
                self.encoder,
                ffi::MOUSE_ENCODER_OPT_SIZE,
                ptr::addr_of!(size).cast(),
            );

            ffi::ghostty_mouse_event_set_action(self.event, mouse.ghostty_action());
            match mouse.button {
                Some(button) => ffi::ghostty_mouse_event_set_button(self.event, TerminalMouseEvent::ghostty_button(button)),
                None => ffi::ghostty_mouse_event_clear_button(self.event),
            }
            ffi::ghostty_mouse_event_set_mods(self.event, mouse.ghostty_mods());
            ffi::ghostty_mouse_event_set_position(
                self.event,
                ffi::GhosttyMousePosition { x: mouse.x, y: mouse.y },
            );

            let mut buffer = vec![0u8; 64];
            let mut written = 0usize;
            if ffi::ghostty_mouse_encoder_encode(
                self.encoder,
                self.event,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut written,
            ) != ffi::GHOSTTY_SUCCESS
            {
                return Vec::new();
            }
            buffer.truncate(written);
            buffer
        }
    }
}

impl Drop for MouseEncoder {
    fn drop(&mut self) {
        // SAFETY: both handles were allocated by the library and are freed once.
        unsafe {
            if !self.event.is_null() {
                ffi::ghostty_mouse_event_free(self.event);
            }
            if !self.encoder.is_null() {
                ffi::ghostty_mouse_encoder_free(self.encoder);
            }
        }
    }
}

impl TerminalEngine {
    /// Whether the program asked for mouse reports (modes 9/1000/1002/1003).
    pub fn is_mouse_tracking(&self) -> bool {
        let mut tracking = false;
        // SAFETY: the terminal handle is live and `tracking` is a bool as documented.
        let result = unsafe {
            ffi::ghostty_terminal_get(self.terminal, ffi::DATA_MOUSE_TRACKING, ptr::addr_of_mut!(tracking).cast())
        };
        result == ffi::GHOSTTY_SUCCESS && tracking
    }

    pub fn encode_mouse(&self, event: &TerminalMouseEvent) -> Vec<u8> {
        let (width, height) = self.cell_size();
        self.mouse.encode(event, self.terminal, self.columns, self.rows, width, height)
    }
}
