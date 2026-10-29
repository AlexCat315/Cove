//! SDL3 native shell. Window handles are confined to the application thread.
mod native;
mod platform_shell;
mod sdl3_shell;
mod sdl3_clipboard;
mod application_entry;
mod cursor;
pub mod display_refresh;
pub mod diagnostics;
pub mod native_menu_popover;
pub mod scroll;
mod log;
pub use cove_platform::*;
pub use platform_shell::*;
pub use sdl3_shell::{Sdl3Shell, Sdl3WindowHandle};
pub use sdl3_clipboard::Sdl3Clipboard;
pub use application_entry::PlatformApplication;
pub use display_refresh::DisplayRefreshTick;
pub fn make_default_shell() -> Result<Sdl3Shell, ShellError> { Sdl3Shell::new() }
// The SDL Cocoa implementation provides the former AppKit shell's capabilities
// with the same native NSWindow/CAMetalLayer and full event/IME routing.
#[cfg(target_os = "macos")]
pub type AppKitShell = Sdl3Shell;
#[cfg(target_os = "macos")]
pub type MacShell = Sdl3Shell;
