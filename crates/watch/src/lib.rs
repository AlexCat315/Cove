//! Directory watching for every Cove host.
//!
//! A [`DirectoryWatcher`] tells an application which paths under a folder
//! were created, removed or modified — by another editor, a sync client or
//! the file manager. The answer always comes from comparing two snapshots of
//! the tree, so it is exact and identical on every platform. Native change
//! notifications only decide *when* to compare:
//!
//! | Platform | Wake source |
//! | --- | --- |
//! | macOS | FSEvents |
//! | Linux, Android | inotify |
//! | Windows | `FindFirstChangeNotificationW` |
//! | iOS, others | polling at [`WatchOptions::poll_interval`] |
//!
//! With a native source the tree is still compared every
//! [`WatchOptions::safety_interval`], so a missed notification only delays a
//! change. The callback runs on the watcher's thread; UI code forwards it with
//! `cove_core::ThreadPoster`.

mod native;
mod snapshot;

pub use snapshot::Snapshot;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

/// How changes are noticed on this platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchBackend {
    /// The operating system announces changes; snapshots confirm them.
    Native,
    /// Snapshots are compared on a timer.
    Polling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchOptions {
    /// Comparison period without native notifications.
    pub poll_interval: Duration,
    /// Comparison period with native notifications, as a safety net.
    pub safety_interval: Duration,
    /// Quiet time after a notification, so one save (temporary file, rename,
    /// metadata) is reported once.
    pub debounce: Duration,
    /// Skip names starting with `.` and everything inside them.
    pub ignore_hidden: bool,
    /// Use the platform's notifications where it has them. Off for folders on
    /// network volumes, which often deliver none.
    pub native: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            safety_interval: Duration::from_secs(30),
            debounce: Duration::from_millis(120),
            ignore_hidden: true,
            native: true,
        }
    }
}

/// Watches a folder tree until dropped.
pub struct DirectoryWatcher {
    root: PathBuf,
    backend: WatchBackend,
    stop: Arc<AtomicBool>,
    wake: Sender<()>,
    worker: Option<JoinHandle<()>>,
    _native: Option<native::NativeWatch>,
}

impl DirectoryWatcher {
    /// Starts watching `root`. `on_change` receives the changed paths, sorted,
    /// on the watcher thread.
    pub fn new(
        root: impl Into<PathBuf>,
        options: WatchOptions,
        on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
    ) -> std::io::Result<Self> {
        let root = root.into();
        if !root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} is not a folder", root.display()),
            ));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (wake, wakes) = channel::<()>();
        let native = if options.native {
            native::NativeWatch::start(&root, wake.clone(), stop.clone())
        } else {
            None
        };
        let backend = if native.is_some() {
            WatchBackend::Native
        } else {
            WatchBackend::Polling
        };
        let interval = match backend {
            WatchBackend::Native => options.safety_interval,
            WatchBackend::Polling => options.poll_interval,
        };
        let worker_root = root.clone();
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("cove-watch".to_owned())
            .spawn(move || {
                let mut snapshot = Snapshot::scan(&worker_root, options.ignore_hidden);
                loop {
                    let woken = match wakes.recv_timeout(interval) {
                        Ok(()) => true,
                        Err(RecvTimeoutError::Timeout) => false,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    if woken {
                        std::thread::sleep(options.debounce);
                        while wakes.try_recv().is_ok() {}
                        if worker_stop.load(Ordering::Acquire) {
                            break;
                        }
                    }
                    let next = Snapshot::scan(&worker_root, options.ignore_hidden);
                    let changes = snapshot.changes(&next);
                    snapshot = next;
                    if !changes.is_empty() {
                        on_change(changes);
                    }
                }
            })?;
        Ok(Self {
            root,
            backend,
            stop,
            wake,
            worker: Some(worker),
            _native: native,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn backend(&self) -> WatchBackend {
        self.backend
    }

    /// Compares the tree now instead of waiting for the next period.
    pub fn check_now(&self) {
        let _ = self.wake.send(());
    }
}

impl Drop for DirectoryWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
