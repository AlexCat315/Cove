use std::ffi::{CStr, CString};
pub struct Sdl3Clipboard;
impl Sdl3Clipboard {
    pub fn read() -> Option<String> {
        // SAFETY: SDL returns an owned NUL terminated allocation, released once below.
        let raw = unsafe { crate::native::cove_shell_clipboard_read() };
        if raw.is_null() {
            return None;
        }
        // SAFETY: raw is the non-null clipboard allocation.
        let value = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: raw was allocated by SDL and has not been freed.
        unsafe { crate::native::cove_shell_free(raw.cast()) };
        (!value.is_empty()).then_some(value)
    }
    pub fn write(text: &str) -> bool {
        let Ok(text) = CString::new(text) else {
            return false;
        };
        // SAFETY: SDL copies this valid C string before returning.
        unsafe { crate::native::cove_shell_clipboard_write(text.as_ptr()) }
    }
}
