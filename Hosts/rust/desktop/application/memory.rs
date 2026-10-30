//! Translation of Desktop/Sources/CoveUI/Application/Memory.swift.
//!
//! `AppWindowMemorySnapshot`/`AppMemorySnapshot` are the host's memory report,
//! and `ApplicationMemoryPressure` subscribes to platform pressure so the host
//! can trim caches off the hot path. Each OS keeps its own native mechanism; the
//! Linux branch samples both host RAM and the cgroup limit because a container
//! limit is what actually applies.

use cove_platform::WindowId;
use cove_text::TextCacheSnapshot;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppWindowRendererState {
    #[default]
    Unconfigured,
    Active,
    Suspended,
    Closed,
}

/// Memory owned by one window's presentation resources.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppWindowMemorySnapshot {
    pub state: AppWindowRendererState,
    pub samples: u32,
    pub has_swapchain: bool,
    /// Owned MSAA, backdrop and effect scratch. Drawable pool/driver memory is
    /// excluded.
    pub estimated_target_bytes: usize,
    pub msaa_logical_bytes: usize,
    /// Backend allocation of the current MSAA target; `None` means unavailable.
    pub msaa_allocated_bytes: Option<usize>,
    pub msaa_memoryless: bool,
}

impl AppWindowMemorySnapshot {
    pub fn new(
        state: AppWindowRendererState,
        samples: u32,
        has_swapchain: bool,
        estimated_target_bytes: usize,
    ) -> Self {
        Self {
            state,
            samples,
            has_swapchain,
            estimated_target_bytes,
            msaa_logical_bytes: 0,
            msaa_allocated_bytes: None,
            msaa_memoryless: false,
        }
    }

    /// The `windows` counters the DevTools memory snapshot reports.
    pub fn counters(&self) -> BTreeMap<String, u64> {
        let mut counters = BTreeMap::from([
            ("samples".to_owned(), u64::from(self.samples)),
            ("hasSwapchain".to_owned(), u64::from(self.has_swapchain)),
            ("suspended".to_owned(), u64::from(self.state == AppWindowRendererState::Suspended)),
            ("targetBytes".to_owned(), self.estimated_target_bytes as u64),
            ("msaaLogicalBytes".to_owned(), self.msaa_logical_bytes as u64),
            ("msaaMemoryless".to_owned(), u64::from(self.msaa_memoryless)),
        ]);
        if let Some(allocated) = self.msaa_allocated_bytes {
            counters.insert("msaaAllocatedBytes".to_owned(), allocated as u64);
        }
        counters
    }
}

/// Whole-application memory report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppMemorySnapshot {
    pub shaping: Option<TextCacheSnapshot>,
    pub text_layout: Option<TextCacheSnapshot>,
    pub cached_image_bytes: usize,
    pub windows: HashMap<WindowId, AppWindowMemorySnapshot>,
}

/// Per-platform low-memory predicates.
///
/// Linux reads host RAM and the cgroup v2 limit because a container limit is
/// what actually applies. macOS takes the pressure level a dispatch source
/// publishes. Windows queries the low-memory resource notification.
pub mod platform_pressure {
    use std::sync::atomic::{AtomicBool, Ordering};

    static SUBSCRIBED: AtomicBool = AtomicBool::new(false);

    #[cfg(target_os = "linux")]
    mod linux {
        /// Host RAM still available, as a fraction of total.
        pub fn available_ratio() -> f64 {
            let Ok(text) = std::fs::read_to_string("/proc/meminfo") else { return 1.0 };
            let (mut total, mut available) = (0.0_f64, 0.0_f64);
            for line in text.lines() {
                let mut fields = line.split_whitespace();
                let (Some(name), Some(value)) = (fields.next(), fields.next()) else { continue };
                let Ok(value) = value.parse::<f64>() else { continue };
                match name {
                    "MemTotal:" => total = value,
                    "MemAvailable:" => available = value,
                    _ => {}
                }
            }
            if total > 0.0 { available / total } else { 1.0 }
        }

        /// Used fraction of the cgroup v2 limit, or `0` when no limit applies.
        pub fn container_ratio() -> f64 {
            let Ok(membership) = std::fs::read_to_string("/proc/self/cgroup") else { return 0.0 };
            let Some(entry) = membership.lines().find(|line| line.starts_with("0::")) else {
                return 0.0;
            };
            let suffix = &entry[3..];
            for path in [format!("/sys/fs/cgroup{suffix}"), "/sys/fs/cgroup".to_owned()] {
                let Ok(current) = std::fs::read_to_string(format!("{path}/memory.current")) else {
                    continue;
                };
                let Ok(maximum) = std::fs::read_to_string(format!("{path}/memory.max")) else {
                    continue;
                };
                let (Ok(used), Ok(limit)) = (current.trim().parse::<f64>(), maximum.trim().parse::<f64>())
                else {
                    continue;
                };
                if limit > 0.0 {
                    return used / limit;
                }
            }
            0.0
        }

        pub fn is_low() -> bool {
            available_ratio() < 0.1 || container_ratio() > 0.9
        }
    }

    #[cfg(target_os = "macos")]
    mod darwin {
        use std::sync::Mutex;

        /// Level published by the host's dispatch memory-pressure source:
        /// 1 = normal, 2 = warning, 4 = critical.
        static PRESSURE_LEVEL: Mutex<u32> = Mutex::new(1);

        pub fn set_level(level: u32) {
            *PRESSURE_LEVEL.lock().unwrap_or_else(|error| error.into_inner()) = level;
        }

        pub fn is_low() -> bool {
            *PRESSURE_LEVEL.lock().unwrap_or_else(|error| error.into_inner()) >= 2
        }
    }

    #[cfg(target_os = "macos")]
    pub use darwin::set_level;

    #[cfg(target_os = "windows")]
    mod windows {
        unsafe extern "system" {
            fn CreateMemoryResourceNotification(kind: u32) -> *mut std::ffi::c_void;
            fn QueryMemoryResourceNotification(
                handle: *mut std::ffi::c_void,
                state: *mut i32,
            ) -> i32;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }

        /// `LowMemoryResourceNotification`
        const LOW_MEMORY: u32 = 0;

        pub fn is_low() -> bool {
            // SAFETY: the API takes no arguments and returns a kernel object this
            // process owns; the handle is closed before returning.
            unsafe {
                let handle = CreateMemoryResourceNotification(LOW_MEMORY);
                if handle.is_null() {
                    return false;
                }
                let mut state = 0_i32;
                let signalled = QueryMemoryResourceNotification(handle, &mut state) != 0;
                CloseHandle(handle);
                signalled && state != 0
            }
        }
    }

    pub fn subscribe() -> bool {
        SUBSCRIBED.store(true, Ordering::Relaxed);
        true
    }

    pub fn unsubscribe() {
        SUBSCRIBED.store(false, Ordering::Relaxed);
    }

    pub fn is_subscribed() -> bool {
        SUBSCRIBED.load(Ordering::Relaxed)
    }

    /// Whether the platform is currently reporting low memory.
    pub fn is_low() -> bool {
        if !is_subscribed() {
            return false;
        }
        #[cfg(target_os = "linux")]
        {
            linux::is_low()
        }
        #[cfg(target_os = "macos")]
        {
            darwin::is_low()
        }
        #[cfg(target_os = "windows")]
        {
            windows::is_low()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }
}

/// Samples platform pressure and reports only transitions to low memory.
pub struct MemoryPressureProbe {
    previous: bool,
}

impl Default for MemoryPressureProbe {
    fn default() -> Self {
        Self { previous: false }
    }
}

impl MemoryPressureProbe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn changed_to_low_memory(&mut self) -> bool {
        let low = platform_pressure::is_low();
        let transition = low && !self.previous;
        self.previous = low;
        transition
    }

    pub fn close(&mut self) {
        self.previous = false;
    }
}

/// Polls pressure off the UI thread and signals the run loop on a transition.
///
/// The shell host is confined to the application thread, so the sampler cannot
/// hold it. The sampler only flips a shared flag; the host's own frame loop
/// drains that flag with [`ApplicationMemoryPressure::take_pending_trim`] and
/// runs the trim on the UI thread.
///
/// Only a transition to low memory raises the flag, so an already-trimmed
/// process does not thrash. Every platform uses the same sampler; the difference
/// is what [`platform_pressure::is_low`] reads.
pub struct ApplicationMemoryPressure {
    stop: Arc<AtomicBool>,
    pending: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ApplicationMemoryPressure {
    pub fn new() -> Self {
        platform_pressure::subscribe();
        let stop = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_pending = pending.clone();
        let thread = std::thread::Builder::new()
            .name("cove-memory-pressure".to_owned())
            .spawn(move || {
                let mut probe = MemoryPressureProbe::new();
                while !thread_stop.load(Ordering::Relaxed) {
                    if probe.changed_to_low_memory() {
                        thread_pending.store(true, Ordering::Relaxed);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                probe.close();
            })
            .ok();
        Self { stop, pending, thread }
    }

    /// Consumes a raised pressure flag. The caller runs the trim on the UI thread.
    pub fn take_pending_trim(&self) -> bool {
        self.pending.swap(false, Ordering::Relaxed)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        platform_pressure::unsubscribe();
    }
}

impl Default for ApplicationMemoryPressure {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ApplicationMemoryPressure {
    fn drop(&mut self) {
        self.stop();
    }
}
