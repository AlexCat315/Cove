//! The platform thread and its one-way link to the UI thread.
//!
//! The main thread pumps the OS; the UI thread composes, lays out and records
//! frames. Neither calls across: the UI thread posts [`PlatformCommand`]s through
//! a [`MainThreadBridge`], reads window facts from a published [`PlatformState`]
//! and receives every result as a message. Nothing waits on the other side, so
//! the design cannot deadlock.
pub mod accessibility;
mod bridge;
mod link;
mod platform_loop;
mod proxy_shell;
mod state;

pub use accessibility::{AccessibilityItem, AccessibilitySnapshot};
pub use bridge::{DialogRequest, MainThreadBridge, PlatformCommand, SaveDialogRequest};
pub use link::{PlatformLink, PlatformMessage};
pub use platform_loop::{PlatformLoop, run_with_ui_thread};
pub use proxy_shell::ProxyShell;
pub use state::PlatformState;
