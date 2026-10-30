//! Selection state and the gesture state machine.
//!
//! Ghostty owns the gesture semantics: a single click places an anchor, a drag
//! extends it, a double click selects a word and a triple click a line.

use crate::engine::TerminalEngine;
use crate::engine::ffi::{self, GhosttySelectionGesture, GhosttySelectionGestureEvent};
use std::ptr;

/// Grid position in the visible viewport plus the surface point it came from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerminalPointer {
    pub column: usize,
    pub row: usize,
    /// Points from the grid's top-left corner.
    pub x: f64,
    pub y: f64,
    /// Monotonic time; lets Ghostty recognise double and triple clicks.
    pub nanoseconds: u64,
}

impl TerminalPointer {
    pub fn new(column: usize, row: usize, x: f64, y: f64) -> Self {
        Self { column, row, x, y, nanoseconds: 0 }
    }
}

/// One phase of a pointer gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionPhase {
    Press,
    Drag,
    Release,
}

/// What the gesture produced.
pub(super) enum SelectionOutcome {
    Select(ffi::GhosttySelection),
    Clear,
}

/// Ghostty's selection gesture state machine.
pub(super) struct SelectionGesture {
    gesture: GhosttySelectionGesture,
    press: GhosttySelectionGestureEvent,
    drag: GhosttySelectionGestureEvent,
    release: GhosttySelectionGestureEvent,
}

impl SelectionGesture {
    pub(super) fn new() -> Self {
        // SAFETY: every handle is allocated here and freed in `free`, which the
        // engine calls before the terminal it built them against.
        unsafe {
            let mut gesture: GhosttySelectionGesture = ptr::null_mut();
            let gesture_ok = ffi::ghostty_selection_gesture_new(ptr::null(), &mut gesture) == ffi::GHOSTTY_SUCCESS;
            let press = Self::event(ffi::SELECTION_GESTURE_EVENT_TYPE_PRESS);
            let drag = Self::event(ffi::SELECTION_GESTURE_EVENT_TYPE_DRAG);
            let release = Self::event(ffi::SELECTION_GESTURE_EVENT_TYPE_RELEASE);
            if !press.is_null() {
                let mut interval: u64 = 500_000_000;
                ffi::ghostty_selection_gesture_event_set(
                    press,
                    ffi::SELECTION_EVENT_OPT_REPEAT_INTERVAL_NS,
                    ptr::addr_of!(interval).cast(),
                );
            }
            Self { gesture: if gesture_ok { gesture } else { ptr::null_mut() }, press, drag, release }
        }
    }

    unsafe fn event(event_type: i32) -> GhosttySelectionGestureEvent {
        let mut event: GhosttySelectionGestureEvent = ptr::null_mut();
        // SAFETY: caller frees the event through `free`; the type is a documented enum value.
        unsafe {
            ffi::ghostty_selection_gesture_event_new(ptr::null(), &mut event, event_type);
        }
        event
    }

    /// The gesture borrows terminal state, so the engine frees it first.
    pub(super) fn free(&self, terminal: ffi::GhosttyTerminal) {
        if self.gesture.is_null() && self.press.is_null() {
            return;
        }
        // SAFETY: each handle is freed exactly once, and the terminal outlives
        // the gesture because the engine drops it first.
        unsafe {
            for event in [self.release, self.drag, self.press] {
                if !event.is_null() {
                    ffi::ghostty_selection_gesture_event_free(event);
                }
            }
            if !self.gesture.is_null() {
                ffi::ghostty_selection_gesture_free(self.gesture, terminal);
            }
        }
    }

    /// Feeds one pointer phase.
    ///
    /// Returns the selection to install, `Clear` when the gesture removes it, or
    /// `None` when nothing changes. `rectangle` selects a block of columns
    /// instead of running text.
    pub(super) fn apply(
        &self,
        phase: SelectionPhase,
        pointer: &TerminalPointer,
        rectangle: bool,
        columns: usize,
        rows: usize,
        cell_width: usize,
        cell_height: usize,
        terminal: ffi::GhosttyTerminal,
    ) -> Option<SelectionOutcome> {
        if self.gesture.is_null() {
            return None;
        }
        let column = pointer.column.min(columns.saturating_sub(1)) as u16;
        let row = pointer.row.min(rows.saturating_sub(1)) as u32;
        let mut reference = ffi::sized_grid_ref();
        // SAFETY: every handle is live and every pointer argument points at a
        // live local whose type matches the option's documented input type.
        unsafe {
            if ffi::ghostty_terminal_grid_ref(terminal, ffi::GhosttyPoint::viewport(column, row), &mut reference)
                != ffi::GHOSTTY_SUCCESS
            {
                return None;
            }
            let mut position = ffi::GhosttySurfacePosition { x: pointer.x, y: pointer.y };
            let event = match phase {
                SelectionPhase::Press => {
                    let mut time = pointer.nanoseconds;
                    ffi::ghostty_selection_gesture_event_set(
                        self.press,
                        ffi::SELECTION_EVENT_OPT_TIME_NS,
                        ptr::addr_of!(time).cast(),
                    );
                    let mut distance = cell_width.max(cell_height) as f64;
                    ffi::ghostty_selection_gesture_event_set(
                        self.press,
                        ffi::SELECTION_EVENT_OPT_REPEAT_DISTANCE,
                        ptr::addr_of!(distance).cast(),
                    );
                    self.press
                }
                SelectionPhase::Drag => {
                    let mut geometry = ffi::GhosttySelectionGestureGeometry {
                        columns: columns as u32,
                        cell_width: cell_width as u32,
                        padding_left: 0,
                        screen_height: (rows * cell_height) as u32,
                    };
                    ffi::ghostty_selection_gesture_event_set(
                        self.drag,
                        ffi::SELECTION_EVENT_OPT_GEOMETRY,
                        ptr::addr_of!(geometry).cast(),
                    );
                    let mut block = rectangle;
                    ffi::ghostty_selection_gesture_event_set(
                        self.drag,
                        ffi::SELECTION_EVENT_OPT_RECTANGLE,
                        ptr::addr_of!(block).cast(),
                    );
                    self.drag
                }
                SelectionPhase::Release => self.release,
            };
            ffi::ghostty_selection_gesture_event_set(event, ffi::SELECTION_EVENT_OPT_REF, ptr::addr_of!(reference).cast());
            if phase != SelectionPhase::Release {
                ffi::ghostty_selection_gesture_event_set(
                    event,
                    ffi::SELECTION_EVENT_OPT_POSITION,
                    ptr::addr_of!(position).cast(),
                );
            }
            let mut selection = ffi::sized_selection();
            match ffi::ghostty_selection_gesture_event(self.gesture, terminal, event, &mut selection) {
                ffi::GHOSTTY_SUCCESS => Some(SelectionOutcome::Select(selection)),
                ffi::GHOSTTY_NO_VALUE => (phase == SelectionPhase::Press).then_some(SelectionOutcome::Clear),
                _ => None,
            }
        }
    }
}

impl TerminalEngine {
    /// Applies a pointer phase to the selection; returns whether it changed.
    pub fn select(&mut self, phase: SelectionPhase, pointer: &TerminalPointer, rectangle: bool) -> bool {
        let (width, height) = self.cell_size();
        let outcome = self.selection.apply(
            phase,
            pointer,
            rectangle,
            self.columns,
            self.rows,
            width,
            height,
            self.terminal,
        );
        match outcome {
            Some(SelectionOutcome::Select(range)) => {
                // SAFETY: the selection was produced for this terminal; the
                // library copies it and converts it to tracked state immediately.
                let result = unsafe {
                    ffi::ghostty_terminal_set(self.terminal, ffi::OPT_SELECTION, ptr::addr_of!(range).cast())
                };
                result == ffi::GHOSTTY_SUCCESS
            }
            Some(SelectionOutcome::Clear) => self.clear_selection(),
            None => false,
        }
    }

    pub fn clear_selection(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        // SAFETY: a null selection value clears it, as documented.
        unsafe {
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_SELECTION, ptr::null());
        }
        true
    }

    pub fn has_selection(&self) -> bool {
        let mut range = ffi::sized_selection();
        // SAFETY: the terminal is live and `range` is a GhosttySelection.
        let result = unsafe {
            ffi::ghostty_terminal_get(self.terminal, ffi::DATA_SELECTION, ptr::addr_of_mut!(range).cast())
        };
        result == ffi::GHOSTTY_SUCCESS
    }

    /// Plain text of the selection, soft wraps joined and trailing blanks
    /// trimmed.
    pub fn selected_text(&self) -> Option<String> {
        let mut range = ffi::sized_selection();
        // SAFETY: handles stay live for the duration of the call; the formatter
        // allocation is freed with `ghostty_free` below.
        unsafe {
            if ffi::ghostty_terminal_get(self.terminal, ffi::DATA_SELECTION, ptr::addr_of_mut!(range).cast())
                != ffi::GHOSTTY_SUCCESS
            {
                return None;
            }
            let mut options = ffi::sized_format_options();
            options.emit = ffi::GHOSTTY_FORMATTER_FORMAT_PLAIN;
            options.unwrap = true;
            options.trim = true;
            options.selection = ptr::addr_of!(range);
            let mut bytes: *mut u8 = ptr::null_mut();
            let mut length = 0usize;
            if ffi::ghostty_terminal_selection_format_alloc(self.terminal, ptr::null(), options, &mut bytes, &mut length)
                != ffi::GHOSTTY_SUCCESS
            {
                return None;
            }
            let text = if bytes.is_null() || length == 0 {
                String::new()
            } else {
                String::from_utf8_lossy(std::slice::from_raw_parts(bytes, length)).into_owned()
            };
            ffi::ghostty_free(ptr::null(), bytes, length);
            Some(text)
        }
    }
}
