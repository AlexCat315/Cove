//! Native change notifications. Each backend only sends a wake; the watcher
//! compares snapshots to learn what changed.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::NativeWatch;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use linux::NativeWatch;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::NativeWatch;

/// iOS has no public recursive notification API for an app's folders and
/// other targets have none at all: the watcher polls.
#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "android",
    windows
)))]
pub struct NativeWatch;

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "android",
    windows
)))]
impl NativeWatch {
    pub fn start(
        _root: &std::path::Path,
        _wake: std::sync::mpsc::Sender<()>,
        _stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Option<Self> {
        None
    }
}
