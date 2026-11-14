use std::ffi::{CStr, c_char, c_void};
#[repr(C)]
#[derive(Default)]
pub(crate) struct Event {
    pub kind: u32,
    pub window: u32,
    pub timestamp: f64,
    pub identity: u64,
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub d: i32,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub modifiers: u16,
    pub flags: u16,
    pub text: *mut c_char,
    pub aux: *mut c_char,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct WindowState {
    pub id: u32,
    pub width: i32,
    pub height: i32,
    pub pixel_width: i32,
    pub pixel_height: i32,
    pub x: i32,
    pub y: i32,
    pub scale: f32,
    pub focused: bool,
    pub visible: bool,
    pub minimized: bool,
    pub occluded: bool,
    pub maximized: bool,
    pub top: f32,
    pub bottom: f32,
    pub leading: f32,
    pub trailing: f32,
    pub surface_kind: u32,
    pub surface: usize,
    pub display: usize,
    pub instance: usize,
    pub native_window: usize,
    pub native_number: u64,
    pub refresh_rate: f64,
}
#[repr(C)]
pub(crate) struct Chrome {
    pub title_height: f32,
    pub leading: f32,
    pub trailing: f32,
    pub border: f32,
    pub explicit_drag: bool,
    pub drag: *const crate::ChromeRect,
    pub non_drag: *const crate::ChromeRect,
    pub drag_count: usize,
    pub non_drag_count: usize,
}
pub(crate) enum Window {}
pub(crate) enum AudioStream {}
pub(crate) type DialogCallback = unsafe extern "C" fn(*mut c_void, *const *const c_char, i32);
unsafe extern "C" {
    #[cfg(target_os = "windows")]
    pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> bool;
    pub fn cove_shell_is_main_thread() -> bool;
    pub fn cove_shell_init() -> bool;
    pub fn cove_shell_quit();
    pub fn cove_shell_error() -> *const c_char;
    pub fn cove_shell_create(
        title: *const c_char,
        width: i32,
        height: i32,
        inset: bool,
        dock: bool,
    ) -> *mut Window;
    pub fn cove_shell_destroy(window: *mut Window);
    /// Shows the platform material behind the window's clear pixels; false
    /// where the platform has none.
    #[cfg(not(target_os = "windows"))]
    pub fn cove_shell_backdrop(window: *mut Window, kind: i32) -> bool;
    pub fn cove_shell_appearance(window: *mut Window, dark: bool);
    pub fn cove_shell_state(window: *mut Window, state: *mut WindowState) -> bool;
    pub fn cove_shell_poll(event: *mut Event) -> bool;
    fn cove_shell_event_free(event: *mut Event);
    pub fn cove_shell_wait(timeout_ns: u64) -> bool;
    pub fn cove_shell_wake();
    pub fn cove_shell_monotonic_seconds() -> f64;
    pub fn cove_shell_command(window: *mut Window, command: i32, a: i32, b: i32);
    pub fn cove_shell_set_title(window: *mut Window, title: *const c_char);
    pub fn cove_shell_is_fullscreen(window: *mut Window) -> bool;
    pub fn cove_shell_chrome(window: *mut Window, chrome: *const Chrome);
    pub fn cove_shell_text_area(window: *mut Window, area: *const f32);
    pub fn cove_shell_keyboard(window: *mut Window, show: bool);
    pub fn cove_shell_keyboard_height() -> f32;
    pub fn cove_shell_device_form() -> i32;
    pub fn cove_shell_system_theme() -> i32;
    pub fn cove_shell_cursor(window: *mut Window, cursor: i32);
    pub fn cove_shell_global_pointer(x: *mut f32, y: *mut f32) -> bool;
    pub fn cove_shell_clipboard_read() -> *mut c_char;
    pub fn cove_shell_clipboard_write(text: *const c_char) -> bool;
    pub fn cove_shell_free(pointer: *mut c_void);
    pub fn cove_shell_audio_init() -> bool;
    pub fn cove_shell_audio_quit();
    pub fn cove_shell_audio_open_default(frequency: i32, channels: i32) -> *mut AudioStream;
    pub fn cove_shell_audio_format(
        stream: *mut AudioStream,
        format: *mut i32,
        channels: *mut i32,
        frequency: *mut i32,
    ) -> bool;
    pub fn cove_shell_audio_queued(stream: *mut AudioStream) -> i32;
    pub fn cove_shell_audio_available(stream: *mut AudioStream) -> i32;
    pub fn cove_shell_audio_put(
        stream: *mut AudioStream,
        bytes: *const c_void,
        length: i32,
    ) -> bool;
    pub fn cove_shell_audio_set_gain(stream: *mut AudioStream, gain: f32) -> bool;
    pub fn cove_shell_audio_set_frequency_ratio(stream: *mut AudioStream, ratio: f32) -> bool;
    pub fn cove_shell_audio_flush(stream: *mut AudioStream) -> bool;
    pub fn cove_shell_audio_pause(stream: *mut AudioStream) -> bool;
    pub fn cove_shell_audio_resume(stream: *mut AudioStream) -> bool;
    pub fn cove_shell_audio_close(stream: *mut AudioStream);
    pub fn cove_shell_dialog(
        window: *mut Window,
        mode: i32,
        path: *const c_char,
        names: *const *const c_char,
        patterns: *const *const c_char,
        count: usize,
        multiple: bool,
        callback: DialogCallback,
        context: *mut c_void,
    );
    /// Exports bytes through the iOS document picker; the OS copies them to the
    /// destination the user picks and reports status 3 without a path.
    #[cfg(target_os = "ios")]
    pub fn cove_shell_save_dialog(
        window: *mut Window,
        name: *const c_char,
        bytes: *const u8,
        size: usize,
        patterns: *const *const c_char,
        count: usize,
        callback: DialogCallback,
        context: *mut c_void,
    );
    pub fn cove_shell_refresh(window: *mut Window, enabled: bool) -> bool;
    pub fn cove_shell_refresh_take(
        window: *mut Window,
        timestamp: *mut f64,
        target: *mut f64,
    ) -> bool;
    pub fn cove_shell_log_install(callback: unsafe extern "C" fn(i32, i32, *const c_char));
    /// Mobile application entry: SDL owns `main` on iOS and Android.
    #[cfg(any(target_os = "ios", target_os = "android"))]
    pub fn cove_sdl_run_app(
        callback: unsafe extern "C" fn(*mut c_void) -> i32,
        context: *mut c_void,
    ) -> i32;
}
impl Event {
    pub fn string(&self) -> String {
        if self.text.is_null() {
            String::new()
        } else {
            // SAFETY: event text is a bridge-owned NUL terminated copy valid until Drop.
            unsafe { CStr::from_ptr(self.text) }
                .to_string_lossy()
                .into_owned()
        }
    }

    /// The secondary string, absent when the event has none (drop source).
    pub fn aux_string(&self) -> Option<String> {
        if self.aux.is_null() {
            return None;
        }
        // SAFETY: `aux` is a bridge-owned NUL terminated copy valid until Drop.
        Some(
            unsafe { CStr::from_ptr(self.aux) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: only this Event owns its copied text, and SDL accepts NULL.
        unsafe { cove_shell_event_free(self) };
    }
}
pub(crate) fn error() -> crate::ShellError {
    // SAFETY: SDL error strings remain valid on the calling application thread until the next SDL call.
    let raw = unsafe { cove_shell_error() };
    let message = if raw.is_null() {
        "SDL operation failed".into()
    } else {
        // SAFETY: SDL_GetError returns a NUL terminated string.
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    };
    crate::ShellError::InitializationFailed(message)
}
