//! SDL3 native shell. Window handles are confined to the application thread.
mod application_entry;
pub(crate) mod color_sampler;
mod cursor;
pub mod diagnostics;
pub mod display_refresh;
mod log;
mod native;
pub mod native_menu_bar;
pub mod native_menu_popover;
#[cfg(target_os = "macos")]
mod native_menu_wire;
mod open_documents;
mod platform_shell;
pub mod scroll;
mod sdl3_clipboard;
mod sdl3_audio;
mod sdl3_shell;
pub use application_entry::PlatformApplication;
pub use color_sampler::screen_color_backend;
pub use cove_platform::*;
pub use display_refresh::DisplayRefreshTick;
pub use platform_shell::*;
pub use sdl3_audio::{AudioDeviceFormat, AudioSampleFormat, AudioStreamSpec, Sdl3AudioDevice};
pub use sdl3_clipboard::Sdl3Clipboard;
pub use sdl3_shell::{Sdl3Shell, Sdl3WindowHandle, external_drop, wheel_event};
pub fn make_default_shell() -> Result<Sdl3Shell, ShellError> {
    Sdl3Shell::new()
}
// The SDL Cocoa implementation provides the former AppKit shell's capabilities
// with the same native NSWindow/CAMetalLayer and full event/IME routing.
#[cfg(target_os = "macos")]
pub type AppKitShell = Sdl3Shell;
#[cfg(target_os = "macos")]
pub type MacShell = Sdl3Shell;

/// Whether the caller runs on the thread that owns the platform event queue.
pub fn is_main_thread() -> bool {
    // SAFETY: SDL's main-thread check is thread-safe and requires no initialized subsystem.
    unsafe { native::cove_shell_is_main_thread() }
}

/// Monotonic host-clock seconds, aligned with native display-link timestamps.
pub fn monotonic_seconds() -> f64 {
    // SAFETY: SDL performance counters are thread-safe and require no initialized subsystem.
    unsafe { native::cove_shell_monotonic_seconds() }
}
