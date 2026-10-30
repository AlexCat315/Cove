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
pub mod event_buffer;
pub mod files;
pub mod file_lock;
pub mod io_state;
mod native;
pub mod native_process;
pub mod paths;
pub mod permissions;
pub mod pipe_read;
pub mod platform;
pub mod process;
pub mod replacement;
pub mod run;
pub mod session;
pub mod tool_locator;
pub mod trash;
#[cfg(windows)]
pub mod windows_launch;
