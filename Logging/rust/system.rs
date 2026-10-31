//! Translation of `Logging/Sources/CoveLogging/System.swift`.
//!
//! One process-wide runtime shared by every producer. The Swift build installs
//! the runtime through swift-log's `LoggingSystem`; the Rust workspace has no
//! logging facade, so `bootstrap` records the installation and returns whether
//! this call performed it, while producers use `LogSystem::logger`.
use super::configuration::{Installation, LoggingConfiguration};
use super::environment::LogLaunch;
use super::handler::{LogEvent, UnifiedLogHandler};
use super::observation::LogObservation;
use super::record::{Level, LogQuery, LogRecord, LogSnapshot, LogValue};
use super::runtime::{LogRuntime, LoggingStatus, register_runtime};
use std::collections::BTreeMap;
use std::ffi::{CStr, c_char};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Standard producers, one process backend, independent subscribers.
pub struct LogSystem;

impl LogSystem {
    pub fn runtime() -> Arc<LogRuntime> {
        static RUNTIME: OnceLock<Arc<LogRuntime>> = OnceLock::new();
        RUNTIME
            .get_or_init(|| {
                let run_id =
                    std::env::var("COVE_LOG_RUN_ID").unwrap_or_else(|_| unique_run_identifier());
                let runtime = LogRuntime::new(run_id);
                register_runtime(&runtime);
                runtime
            })
            .clone()
    }

    /// Installs configuration and the native bridge. Repeated process
    /// installation returns `false` after reconfiguring the runtime.
    pub fn bootstrap(
        application: Option<&str>,
        configuration: LoggingConfiguration,
        run_id: Option<String>,
    ) -> io::Result<bool> {
        let application = application
            .map(str::to_owned)
            .unwrap_or_else(|| std::env::args().next().unwrap_or_else(|| "Cove".into()));
        let runtime = Self::runtime();
        runtime.configure(configuration.clone(), &application, run_id)?;
        let mut installed = INSTALLATION
            .get_or_init(|| Mutex::new(false))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        native::set_handler(native_log, flush_native_logs);
        if configuration.installation == Installation::Process && !*installed {
            *installed = true;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn logger(label: &str) -> UnifiedLogHandler {
        Self::runtime().logger(label)
    }

    pub fn flush() {
        Self::runtime().flush();
    }

    pub fn status() -> LoggingStatus {
        Self::runtime().status()
    }

    pub fn launch() -> LogLaunch {
        let runtime = Self::runtime();
        let identity = runtime.identity();
        LogLaunch::new(runtime.configuration(), identity.run_id, identity.name)
    }

    pub fn snapshot(query: LogQuery) -> LogSnapshot {
        Self::runtime().snapshot(&query)
    }

    pub fn observe(
        query: LogQuery,
        replay: bool,
        receive: Arc<dyn Fn(&LogRecord) + Send + Sync>,
    ) -> LogObservation {
        Self::runtime().observe(query, replay, receive)
    }

    /// Platform log root. Windows uses `LOCALAPPDATA`, Apple desktop uses
    /// `~/Library/Logs`, sandboxed Apple platforms use Application Support and
    /// other Unix systems follow the XDG state directory.
    pub fn default_directory() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            let base = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(home_directory);
            return base.join("Cove").join("Logs");
        }
        #[cfg(target_os = "macos")]
        {
            return home_directory().join("Library").join("Logs").join("Cove");
        }
        #[cfg(any(
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        {
            let base = std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join("Library").join("Application Support"))
                .unwrap_or_else(std::env::temp_dir);
            return base.join("Cove").join("Logs");
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        )))]
        {
            let base = std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_directory().join(".local").join("state"));
            return base.join("cove").join("logs");
        }
    }
}

static INSTALLATION: OnceLock<Mutex<bool>> = OnceLock::new();

fn home_directory() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// Mirrors Foundation's UUID text form without adding a UUID dependency.
fn unique_run_identifier() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = u64::from(std::process::id());
    let high = nanos ^ (pid << 32) ^ counter.rotate_left(17);
    let low = nanos.rotate_left(23) ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ pid;
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (high >> 32) as u32,
        (high >> 16) as u16,
        high as u16 & 0x0fff,
        (low >> 48) as u16 & 0x0fff,
        low & 0x0000_ffff_ffff_ffff
    )
}

/// Native producers (the C router and platform bridges) enter here. The
/// callback never unwinds across the C boundary.
unsafe extern "C" fn native_log(
    level: i32,
    label: *const c_char,
    message: *const c_char,
    file: *const c_char,
    function: *const c_char,
    line: u32,
) {
    let label = unsafe { string_from_pointer(label) }.unwrap_or_else(|| "com.cove.native".into());
    let message = unsafe { string_from_pointer(message) }.unwrap_or_default();
    let mut metadata = BTreeMap::new();
    metadata.insert("origin".to_owned(), LogValue::from("native"));
    let event = LogEvent {
        level: Level::from_native_index(level),
        message,
        metadata: Some(metadata),
        source: "native".into(),
        file: unsafe { string_from_pointer(file) }.unwrap_or_default(),
        function: unsafe { string_from_pointer(function) }.unwrap_or_default(),
        line: u64::from(line),
    };
    LogSystem::logger(&label).log(&event);
}

extern "C" fn flush_native_logs() {
    LogSystem::flush();
}

unsafe fn string_from_pointer(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    // The native bridge guarantees NUL-terminated UTF-8 text.
    Some(
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// Console fallback used when no native handler is installed and by the
/// runtime for saturation and storage failures.
///
/// Apple/Windows/Linux write a line to stderr, exactly like
/// `cove_log_console_write`; Android routes through `__android_log_write` in
/// chunks that do not split a UTF-8 continuation byte.
pub fn console_write(level: i32, message: &str) {
    if message.is_empty() {
        return;
    }
    #[cfg(target_os = "android")]
    {
        android_console_write(level, message);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = level;
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        let _ = handle.write_all(message.as_bytes());
        let _ = handle.write_all(b"\n");
        let _ = handle.flush();
    }
}

#[cfg(target_os = "android")]
fn android_console_write(level: i32, message: &str) {
    const PRIORITIES: [i32; 7] = [2, 3, 4, 4, 5, 6, 7]; // VERBOSE..FATAL
    let priority = PRIORITIES[if (0..=6).contains(&level) {
        level as usize
    } else {
        2
    }];
    let bytes = message.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        let mut count = (bytes.len() - offset).min(3800);
        while offset + count < bytes.len() && count > 0 && bytes[offset + count] & 0xc0 == 0x80 {
            count -= 1;
        }
        let chunk = &bytes[offset..offset + count];
        if let Ok(text) = std::str::from_utf8(chunk) {
            if let Ok(text) = std::ffi::CString::new(text) {
                if let Ok(tag) = std::ffi::CString::new("Cove") {
                    unsafe {
                        __android_log_write(priority, tag.as_ptr(), text.as_ptr());
                    }
                }
            }
        }
        offset += count;
    }
}

#[cfg(target_os = "android")]
unsafe extern "C" {
    fn __android_log_write(priority: i32, tag: *const c_char, text: *const c_char) -> i32;
}

/// The CCoveLog router is not part of the Rust build; the externs document the
/// native contract and are only reached when the target links that library.
mod native {
    use super::*;

    type Handler =
        unsafe extern "C" fn(i32, *const c_char, *const c_char, *const c_char, *const c_char, u32);
    type Flush = extern "C" fn();

    unsafe extern "C" {
        fn cove_log_set_handler(handler: Handler, flush: Flush);
    }

    pub fn set_handler(handler: Handler, flush: Flush) {
        unsafe {
            cove_log_set_handler(handler, flush);
        }
    }
}
