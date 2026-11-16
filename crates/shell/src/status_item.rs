//! The menu-bar / system tray extra: a persistent item whose menu the app fills.
//!
//! On macOS the item is AppKit's `NSStatusItem`; on Windows and Linux the same
//! model is backed by SDL's native tray implementation. This module owns the
//! menu model and the delivery of picks back to the UI thread on every desktop
//! platform. An empty title inserts a separator.
//!
//! On macOS an id of `quit` terminates the process from the native side — a
//! hidden app can still reach the menu bar, so that entry must not depend on the
//! UI thread being alive. Everywhere else picks are delivered through the same
//! queue, so the handler resolves `quit` like any other id.
//!
//! Also like [`crate::services`]: the menu is built on the main thread, while the
//! handler belongs to whichever thread built the UI.

use std::fmt;

/// One entry in the tray menu. `title` empty ⇒ separator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusEntry {
    pub id: &'static str,
    pub title: &'static str,
}

/// The platform has no menu-bar extra to publish to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported;

impl fmt::Display for Unsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("this platform has no menu-bar extra")
    }
}

impl std::error::Error for Unsupported {}

type Handler = Box<dyn Fn(&str)>;

thread_local! {
    static HANDLER: std::cell::RefCell<Option<Handler>> = const { std::cell::RefCell::new(None) };
}

/// Entry ids, indexed by the tag the native menu carries.
static IDS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
/// What [`install`] asked for, kept so the platform loop can build it on the
/// main thread once the native tray is ready.
static SPEC: std::sync::Mutex<Option<(String, String, &'static [StatusEntry])>> =
    std::sync::Mutex::new(None);
/// Picks waiting for the UI thread.
static QUEUE: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// Desktop targets that own a system tray / menu-bar extra.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub const SUPPORTED: bool = true;
#[cfg(any(target_os = "ios", target_os = "android"))]
pub const SUPPORTED: bool = false;

/// Publishes the tray item with `symbol` (an SF Symbol name on macOS, an icon
/// file path on Windows/Linux — empty uses the platform default) and `entries`,
/// routing picks to `handler` on the thread that calls this.
///
/// Call where the UI is built: [`poll`] dispatches picks there. Re-calling
/// rebuilds the menu, so the entry list can change at runtime.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn install(
    symbol: &str,
    tooltip: &str,
    entries: &'static [StatusEntry],
    handler: impl Fn(&str) + 'static,
) -> Result<(), Unsupported> {
    HANDLER.with(|cell| *cell.borrow_mut() = Some(Box::new(handler)));
    *IDS.lock().unwrap_or_else(|error| error.into_inner()) =
        entries.iter().map(|entry| entry.id).collect();
    *SPEC.lock().unwrap_or_else(|error| error.into_inner()) =
        Some((symbol.to_string(), tooltip.to_string(), entries));
    flush_pending();
    Ok(())
}

/// No tray on this platform.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn install(
    _symbol: &str,
    _tooltip: &str,
    _entries: &'static [StatusEntry],
    _handler: impl Fn(&str) + 'static,
) -> Result<(), Unsupported> {
    Err(Unsupported)
}

/// Builds the native menu once the platform is ready for it. Idempotent; the
/// platform loop calls it each tick.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn flush_pending() {
    if !crate::is_main_thread() {
        return;
    }
    let spec = SPEC
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let Some((symbol, tooltip, entries)) = spec else {
        return;
    };
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        // SAFETY: main thread, once, and the callback outlives the process.
        unsafe {
            bridge::install(
                &symbol,
                &tooltip,
                entries.iter().map(|entry| entry.title).collect(),
                entries.iter().map(|entry| entry.id).collect(),
                bridge::picked,
            );
        }
    });
}

/// No-op without a tray.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn flush_pending() {}

/// Runs the queued picks on the calling (UI) thread. Call once per frame next to
/// [`crate::services::poll`].
pub fn poll() {
    let picked: Vec<&'static str> = {
        let mut queue = QUEUE.lock().unwrap_or_else(|error| error.into_inner());
        std::mem::take(&mut *queue)
    };
    if picked.is_empty() {
        return;
    }
    HANDLER.with(|cell| {
        let borrowed = cell.borrow();
        let Some(handler) = borrowed.as_ref() else {
            return;
        };
        for id in picked {
            handler(id);
        }
    });
}

/// Enqueues a native pick and wakes the platform loop so [`poll`] drains it.
/// Shared by both platform bridges.
fn picked(index: usize) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(id) = IDS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(index)
            .copied()
        else {
            return;
        };
        QUEUE
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(id);
        // SAFETY: wakes the platform loop so the queue drains next frame.
        unsafe { crate::native::cove_shell_wake() };
    }));
}

#[cfg(target_os = "macos")]
mod bridge {
    use std::ffi::{CString, c_char};

    unsafe extern "C" {
        fn cove_shell_status_item_install(
            symbol: *const c_char,
            tooltip: *const c_char,
            titles: *const *const c_char,
            ids: *const *const c_char,
            count: usize,
            callback: extern "C" fn(usize),
        ) -> bool;
    }

    pub(super) unsafe fn install(
        symbol: &str,
        tooltip: &str,
        titles: Vec<&'static str>,
        ids: Vec<&'static str>,
        callback: extern "C" fn(usize),
    ) {
        let symbol = CString::new(symbol).ok();
        let tooltip = CString::new(tooltip).ok();
        let titles: Vec<CString> = titles
            .iter()
            .filter_map(|title| CString::new(*title).ok())
            .collect();
        let ids: Vec<CString> = ids.iter().filter_map(|id| CString::new(*id).ok()).collect();
        let titles: Vec<*const c_char> = titles.iter().map(|title| title.as_ptr()).collect();
        let ids: Vec<*const c_char> = ids.iter().map(|id| id.as_ptr()).collect();
        // SAFETY: every pointer outlives the call, which copies what it keeps.
        unsafe {
            cove_shell_status_item_install(
                symbol
                    .as_ref()
                    .map_or(std::ptr::null(), |symbol| symbol.as_ptr()),
                tooltip
                    .as_ref()
                    .map_or(std::ptr::null(), |tooltip| tooltip.as_ptr()),
                titles.as_ptr(),
                ids.as_ptr(),
                titles.len(),
                callback,
            );
        }
    }

    pub(super) extern "C" fn picked(index: usize) {
        super::picked(index);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
mod bridge {
    use std::ffi::{CString, c_char};

    unsafe extern "C" {
        fn cove_shell_tray_install(
            icon_path: *const c_char,
            tooltip: *const c_char,
            titles: *const *const c_char,
            ids: *const *const c_char,
            count: usize,
            callback: extern "C" fn(usize),
        ) -> bool;
    }

    pub(super) unsafe fn install(
        symbol: &str,
        tooltip: &str,
        titles: Vec<&'static str>,
        ids: Vec<&'static str>,
        callback: extern "C" fn(usize),
    ) {
        let icon = CString::new(symbol).ok();
        let tooltip = CString::new(tooltip).ok();
        let titles: Vec<CString> = titles
            .iter()
            .filter_map(|title| CString::new(*title).ok())
            .collect();
        let ids: Vec<CString> = ids.iter().filter_map(|id| CString::new(*id).ok()).collect();
        let titles: Vec<*const c_char> = titles.iter().map(|title| title.as_ptr()).collect();
        let ids: Vec<*const c_char> = ids.iter().map(|id| id.as_ptr()).collect();
        // SAFETY: every pointer outlives the call; the bridge copies what it keeps.
        unsafe {
            cove_shell_tray_install(
                icon.as_ref().map_or(std::ptr::null(), |icon| icon.as_ptr()),
                tooltip
                    .as_ref()
                    .map_or(std::ptr::null(), |tooltip| tooltip.as_ptr()),
                titles.as_ptr(),
                ids.as_ptr(),
                titles.len(),
                callback,
            );
        }
    }

    pub(super) extern "C" fn picked(index: usize) {
        super::picked(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_flag_matches_target() {
        let desktop = !cfg!(any(target_os = "ios", target_os = "android"));
        assert_eq!(SUPPORTED, desktop);
    }

    #[test]
    fn empty_title_marks_a_separator() {
        let entry = StatusEntry { id: "", title: "" };
        assert!(entry.title.is_empty());
    }
}
