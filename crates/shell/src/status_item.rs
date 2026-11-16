//! The menu-bar extra: a persistent status item whose menu the app fills.
//!
//! The item itself is AppKit's `NSStatusItem`; this module owns the menu model
//! and the delivery of picks back to the UI thread. An empty title inserts a
//! separator, and an id of `quit` terminates the process from the native side —
//! a hidden app can still reach the menu bar, so the last entry must not depend
//! on the UI thread being alive.
//!
//! Also like [`crate::services`]: the menu is built on the main thread, while the
//! handler belongs to whichever thread built the UI.

use std::fmt;

/// One entry in the menu-bar menu. `title` empty ⇒ separator.
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
/// main thread once AppKit is ready.
static SPEC: std::sync::Mutex<Option<(String, String, &'static [StatusEntry])>> = std::sync::Mutex::new(None);
/// Picks waiting for the UI thread.
static QUEUE: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// Publishes the menu-bar extra with `symbol` (an SF Symbol name) and `entries`,
/// routing picks to `handler` on the thread that calls this.
///
/// Call where the UI is built: [`poll`] dispatches picks there. Re-calling
/// rebuilds the menu, so the entry list can change at runtime.
#[cfg(target_os = "macos")]
pub fn install(
    symbol: &str,
    tooltip: &str,
    entries: &'static [StatusEntry],
    handler: impl Fn(&str) + 'static,
) -> Result<(), Unsupported> {
    HANDLER.with(|cell| *cell.borrow_mut() = Some(Box::new(handler)));
    *IDS.lock().unwrap_or_else(|error| error.into_inner()) = entries.iter().map(|entry| entry.id).collect();
    *SPEC.lock().unwrap_or_else(|error| error.into_inner()) = Some((symbol.to_string(), tooltip.to_string(), entries));
    flush_pending();
    Ok(())
}

/// No menu-bar extra on this platform.
#[cfg(not(target_os = "macos"))]
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
#[cfg(target_os = "macos")]
pub fn flush_pending() {
    if !crate::is_main_thread() {
        return;
    }
    let spec = SPEC.lock().unwrap_or_else(|error| error.into_inner()).clone();
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

/// No-op without a menu-bar extra.
#[cfg(not(target_os = "macos"))]
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

#[cfg(target_os = "macos")]
mod bridge {
    use super::*;
    use std::ffi::{CString, c_char};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    unsafe extern "C" {
        fn cove_shell_status_item_install(
            symbol: *const c_char,
            tooltip: *const c_char,
            titles: *const *const c_char,
            ids: *const *const c_char,
            count: usize,
            callback: extern "C" fn(usize),
        ) -> bool;
        fn cove_shell_wake();
    }

    pub(super) unsafe fn install(symbol: &str, tooltip: &str, titles: Vec<&'static str>, ids: Vec<&'static str>, callback: extern "C" fn(usize)) {
        let symbol = CString::new(symbol).ok();
        let tooltip = CString::new(tooltip).ok();
        let titles: Vec<CString> = titles.iter().filter_map(|title| CString::new(*title).ok()).collect();
        let ids: Vec<CString> = ids.iter().filter_map(|id| CString::new(*id).ok()).collect();
        let titles: Vec<*const c_char> = titles.iter().map(|title| title.as_ptr()).collect();
        let ids: Vec<*const c_char> = ids.iter().map(|id| id.as_ptr()).collect();
        // SAFETY: every pointer outlives the call, which copies what it keeps.
        unsafe {
            cove_shell_status_item_install(
                symbol.as_ref().map_or(std::ptr::null(), |symbol| symbol.as_ptr()),
                tooltip.as_ref().map_or(std::ptr::null(), |tooltip| tooltip.as_ptr()),
                titles.as_ptr(),
                ids.as_ptr(),
                titles.len(),
                callback,
            );
        }
    }

    pub(super) extern "C" fn picked(index: usize) {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let Some(id) = IDS
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(index)
                .copied()
            else {
                return;
            };
            QUEUE.lock().unwrap_or_else(|error| error.into_inner()).push(id);
            // SAFETY: wakes the platform loop so the queue drains next frame.
            unsafe { cove_shell_wake() };
        }));
    }
}
