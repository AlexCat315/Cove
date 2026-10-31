//! OSC 8 hyperlink lookup for viewport cells.

use crate::engine::TerminalEngine;
use crate::engine::ffi;
use std::ptr;

impl TerminalEngine {
    /// OSC 8 target of the viewport cell, if any.
    pub fn hyperlink(&self, column: usize, row: usize) -> Option<String> {
        if column >= self.columns || row >= self.rows {
            return None;
        }
        let mut reference = ffi::sized_grid_ref();
        let point = ffi::GhosttyPoint::viewport(column as u16, row as u32);
        // SAFETY: the terminal is live, `reference` is a GhosttyGridRef, and the
        // library writes at most `buffer.len()` bytes into `buffer`.
        unsafe {
            if ffi::ghostty_terminal_grid_ref(self.terminal, point, &mut reference)
                != ffi::GHOSTTY_SUCCESS
            {
                return None;
            }
            let mut length = 0usize;
            ffi::ghostty_grid_ref_hyperlink_uri(&reference, ptr::null_mut(), 0, &mut length);
            if length == 0 {
                return None;
            }
            let mut buffer = vec![0u8; length];
            if ffi::ghostty_grid_ref_hyperlink_uri(
                &reference,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut length,
            ) != ffi::GHOSTTY_SUCCESS
            {
                return None;
            }
            buffer.truncate(length);
            Some(String::from_utf8_lossy(&buffer).into_owned())
        }
    }
}
