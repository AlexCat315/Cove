//! Pointer routing and per-surface state.
//!
//! Decides what a pointer gesture means on the terminal:
//!
//! - The program asked for mouse reports, so report it (hold Shift to select
//!   instead).
//! - Cmd-click on an OSC 8 hyperlink or a plain-text URL opens the link.
//! - Otherwise select text (drag, double-click word, triple-click line; hold
//!   Option to select a rectangular block).
//!
//! The wheel scrolls the scrollback, or is reported/turned into arrow keys for
//! full-screen programs.

use crate::engine::{MouseAction, MouseButton, SelectionPhase, TerminalMouseEvent, TerminalPointer};
use crate::session::TerminalSession;
use crate::view::url_detector::{self, UrlMatch};
use cove_platform::{MouseButton as PlatformButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gesture {
    None,
    Selecting { rectangle: bool },
    Reporting(MouseButton),
}

/// Decisions the router needs from the view: where a window point lands on the
/// grid, and whether a click was a Command-click.
pub struct PointerGeometry {
    /// Window position of the grid's top-left corner.
    pub origin: (f32, f32),
    /// Size of one cell in points.
    pub cell: (f32, f32),
    /// Cell size the engine measures pixels in, for its own pointer scale.
    pub engine_cell: (usize, usize),
    pub frame_columns: usize,
    pub frame_rows: usize,
    /// Whole window height, used to detect drags past the bottom edge.
    pub height: f32,
}

pub struct PointerRouter {
    gesture: std::cell::Cell<Gesture>,
    wheel_rows: std::cell::Cell<f32>,
    hovered: std::cell::RefCell<Option<UrlMatch>>,
}

impl Default for PointerRouter {
    fn default() -> Self {
        Self {
            gesture: std::cell::Cell::new(Gesture::None),
            wheel_rows: std::cell::Cell::new(0.0),
            hovered: std::cell::RefCell::new(None),
        }
    }
}

impl PointerRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&self) {
        self.gesture.set(Gesture::None);
        self.wheel_rows.set(0.0);
        *self.hovered.borrow_mut() = None;
    }

    pub fn hovered_link(&self) -> Option<UrlMatch> {
        self.hovered.borrow().clone()
    }

    /// Grid position under a window-space point, clamped to the grid.
    pub fn pointer(&self, geometry: &PointerGeometry, x: f32, y: f32, nanoseconds: u64) -> Option<TerminalPointer> {
        if geometry.cell.0 <= 0.0 || geometry.cell.1 <= 0.0 {
            return None;
        }
        let local_x = (x - geometry.origin.0) as f64;
        let local_y = (y - geometry.origin.1) as f64;
        let column = (local_x / geometry.cell.0 as f64).max(0.0) as usize;
        let row = (local_y / geometry.cell.1 as f64).max(0.0) as usize;
        let column = column.min(geometry.frame_columns.saturating_sub(1));
        let row = row.min(geometry.frame_rows.saturating_sub(1));
        // The engine measures in the integer cell size it was given.
        let scale_x = geometry.engine_cell.0 as f64 / geometry.cell.0.max(1.0) as f64;
        let scale_y = geometry.engine_cell.1 as f64 / geometry.cell.1.max(1.0) as f64;
        Some(TerminalPointer { column, row, x: local_x * scale_x, y: local_y * scale_y, nanoseconds })
    }

    /// A press. Returns whether the terminal handled it.
    pub fn down(
        &self,
        event: &MouseButtonEvent,
        session: &TerminalSession,
        geometry: &PointerGeometry,
    ) -> bool {
        let Some(pointer) = self.pointer(geometry, event.x, event.y, now_nanos()) else {
            return false;
        };
        let modifiers = modifiers(event.modifiers);
        if session.is_mouse_tracking()
            && !event.modifiers.has_shift()
            && let Some(button) = button(event.button)
        {
            self.gesture.set(Gesture::Reporting(button));
            let report = TerminalMouseEvent {
                action: MouseAction::Press,
                button: Some(button),
                x: pointer.x as f32,
                y: pointer.y as f32,
                modifiers,
            };
            session.report(&session.encode_mouse(&report));
            return true;
        }
        if event.button != PlatformButton::Left {
            return true;
        }
        self.gesture.set(Gesture::Selecting { rectangle: event.modifiers.has_alt() });
        session.select(SelectionPhase::Press, &pointer, event.modifiers.has_alt());
        true
    }

    /// Pointer motion. Returns whether the terminal handled it.
    pub fn motion(
        &self,
        event: &MouseMotionEvent,
        session: &TerminalSession,
        geometry: &PointerGeometry,
    ) -> bool {
        let Some(pointer) = self.pointer(geometry, event.x, event.y, now_nanos()) else {
            return false;
        };
        match self.gesture.get() {
            Gesture::Selecting { rectangle } => {
                // Dragging past the top or bottom edge scrolls the history.
                let edge = event.y;
                if edge < geometry.origin.1 {
                    session.scroll(-1);
                } else if edge > geometry.origin.1 + geometry.height {
                    session.scroll(1);
                }
                session.select(SelectionPhase::Drag, &pointer, rectangle);
                true
            }
            Gesture::Reporting(button) => {
                self.report(MouseAction::Motion, Some(button), &pointer, session);
                true
            }
            Gesture::None => {
                if session.is_mouse_tracking() {
                    self.report(MouseAction::Motion, None, &pointer, session);
                } else {
                    let frame = session.frame();
                    let marked = frame
                        .cell(pointer.row.min(frame.rows.saturating_sub(1)), pointer.column.min(frame.columns.saturating_sub(1)))
                        .is_hyperlink;
                    *self.hovered.borrow_mut() = if marked {
                        None
                    } else {
                        url_detector::find(&frame, pointer.row, pointer.column)
                    };
                }
                false
            }
        }
    }

    /// A release. Returns whether the terminal handled it.
    pub fn up(&self, event: &MouseButtonEvent, session: &TerminalSession, geometry: &PointerGeometry) -> bool {
        let gesture = self.gesture.replace(Gesture::None);
        let Some(pointer) = self.pointer(geometry, event.x, event.y, now_nanos()) else {
            return false;
        };
        match gesture {
            Gesture::Selecting { rectangle } => {
                session.select(SelectionPhase::Release, &pointer, rectangle);
                true
            }
            Gesture::Reporting(button) => {
                self.report(MouseAction::Release, Some(button), &pointer, session);
                true
            }
            Gesture::None => false,
        }
    }

    /// Wheel input. Returns whether the terminal handled it.
    pub fn wheel(&self, event: &MouseWheelEvent, session: &TerminalSession, line_step: f32) -> bool {
        if line_step <= 0.0 {
            return false;
        }
        let (_, dy) = event.displacement(line_step, 0.0, 0.0);
        self.wheel_rows.set(self.wheel_rows.get() + dy / line_step);
        let rows = self.wheel_rows.get().trunc();
        if rows == 0.0 {
            return true;
        }
        self.wheel_rows.set(self.wheel_rows.get() - rows);
        let rows = rows as i32;
        let steps = rows.unsigned_abs().min(50);
        if session.is_mouse_tracking() {
            let pointer = TerminalPointer::new(0, 0, event.mouse_x.unwrap_or(0.0) as f64, event.mouse_y.unwrap_or(0.0) as f64);
            for _ in 0..steps {
                let button = if rows > 0 { MouseButton::WheelUp } else { MouseButton::WheelDown };
                self.report(MouseAction::Press, Some(button), &pointer, session);
            }
        } else if session.is_alternate_screen() {
            let scancode = if rows > 0 { 82 } else { 81 };
            for _ in 0..steps {
                session.press(&crate::engine::TerminalKey::new(scancode));
            }
        } else {
            session.scroll(-rows);
        }
        true
    }

    /// OSC 8 target of the cell, else a plain-text URL printed there.
    pub fn link_at(&self, pointer: &TerminalPointer, session: &TerminalSession) -> Option<String> {
        if let Some(marked) = session.hyperlink(pointer.column, pointer.row) {
            return crate::view::link_opener::url(&marked);
        }
        let frame = session.frame();
        url_detector::find(&frame, pointer.row, pointer.column).map(|found| found.url)
    }

    fn report(
        &self,
        action: MouseAction,
        button: Option<MouseButton>,
        pointer: &TerminalPointer,
        session: &TerminalSession,
    ) {
        let event = TerminalMouseEvent {
            action,
            button,
            x: pointer.x as f32,
            y: pointer.y as f32,
            modifiers: crate::engine::KeyModifiers::empty(),
        };
        session.report(&session.encode_mouse(&event));
    }
}

fn button(button: PlatformButton) -> Option<MouseButton> {
    match button {
        PlatformButton::Left => Some(MouseButton::Left),
        PlatformButton::Middle => Some(MouseButton::Middle),
        PlatformButton::Right => Some(MouseButton::Right),
        PlatformButton::X1 | PlatformButton::X2 => None,
    }
}

fn modifiers(modifiers: cove_platform::KeyModifiers) -> crate::engine::KeyModifiers {
    let mut result = crate::engine::KeyModifiers::empty();
    if modifiers.has_shift() {
        result |= crate::engine::KeyModifiers::SHIFT;
    }
    if modifiers.has_ctrl() {
        result |= crate::engine::KeyModifiers::CONTROL;
    }
    if modifiers.has_alt() {
        result |= crate::engine::KeyModifiers::OPTION;
    }
    result
}

fn now_nanos() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}
