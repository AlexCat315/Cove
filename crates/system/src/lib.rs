//! Shared filesystem and process services for the CLI and application runtime.
//!
//! Module layout follows the Swift packages: `CoveFileSystem` supplies
//! `atomic_file`, `file_lock`, `paths`, `permissions`, `replacement` and
//! `trash`; `CoveProcess` supplies `command`, `event_buffer`, `io_state`,
//! `native_process`, `pipe_read`, `platform`, `run`, `session`, `tool_locator`
//! and `windows_launch`. `files` holds the native filesystem bridge these
//! modules build on, and `process` is the aggregated process surface.
pub mod atomic_file;
pub mod command;
#[cfg(any(unix, windows))]
pub mod event_buffer;
pub mod file_lock;
pub mod files;
pub mod io_state;
#[cfg(any(unix, windows))]
mod native;
#[cfg(any(unix, windows))]
pub mod native_process;
pub mod paths;
pub mod permissions;
pub mod pipe_read;
#[cfg(any(unix, windows))]
pub mod platform;
#[cfg(any(unix, windows))]
pub mod process;
pub mod replacement;
#[cfg(any(unix, windows))]
pub mod run;
#[cfg(any(unix, windows))]
pub mod session;
pub mod tool_locator;
pub mod trash;
#[cfg(windows)]
pub mod windows_launch;
