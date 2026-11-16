//! OS shell services: verbs the platform's own file menu can invoke.
//!
//! macOS publishes them through Finder's **Services** submenu. The menu entries
//! come from the app bundle's `NSServices` list, and AppKit calls the selector
//! declared there on the provider installed by [`install`]. Services are not
//! AppleEvents, so no app delegate is involved.
//!
//! Threading: apps build their launch object on the UI thread, while AppKit
//! delivers services on the main (platform) thread. The provider therefore only
//! resolves a selector to a verb and records `(verb, paths)` in a process-wide
//! queue, waking the loop; the handler stays on the UI thread and runs from
//! [`poll`].
//!
//! Windows has no runtime-injectable verb: Explorer reads registry keys, and the
//! installer owns writes/uninstalls. This module therefore only *plans* those
//! keys ([`windows_verb_entries`]) so the installer and tests share one source
//! of truth for paths, labels and command lines.
//!
//! Linux has neither mechanism: every file manager has its own (Nautilus
//! scripts, Dolphin service menus, Thunar actions, Nemo actions). There is no
//! runtime publication path, so [`install`] reports [`ServiceError::Unsupported`];
//! the installer writes those action files via `cove-package` instead.

use std::fmt;

// Descriptors and the registry key plan live in a dependency-free crate so the
// installer plans the exact same keys the runtime installs.
pub use cove_shell_verbs::{
    MultiSelectModel, RegistryEntry, RegistryScope, ShellVerb, VerbError, VerbTarget,
    windows_verb_entries, windows_verb_entries_with, windows_verb_key,
};

/// Why a service request could not be published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The platform publishes no runtime file-menu verbs. Windows registry verbs
    /// belong to the installer; Linux verbs belong to per-desktop action files.
    Unsupported,
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str("this platform publishes no runtime shell services"),
        }
    }
}

impl std::error::Error for ServiceError {}

type Handler = Box<dyn Fn(&str, Vec<String>)>;

/// One service activation waiting for the UI thread.
struct Activation {
    verb: String,
    paths: Vec<String>,
}

thread_local! {
    // UI-thread storage for the handler. The provider runs on the main thread
    // and never touches this; it only enqueues activations.
    static HANDLER: std::cell::RefCell<Option<Handler>> = const { std::cell::RefCell::new(None) };
}

/// Selector → verb id. Shared across threads and holds no callbacks, so the
/// main thread can resolve an incoming service without borrowing UI state.
static SELECTORS: std::sync::Mutex<Vec<(&'static str, &'static str)>> = std::sync::Mutex::new(Vec::new());
/// Activations handed over by the main thread; drained by [`poll`].
static QUEUE: std::sync::Mutex<Vec<Activation>> = std::sync::Mutex::new(Vec::new());

/// Publishes `verbs` to the platform file menu and routes activations to
/// `handler` as `(verb id, selected paths)`.
///
/// Call from the thread that builds the UI: the handler is stored there and
/// every activation is dispatched on that thread by [`poll`]. macOS installs the
/// `NSApplication` services provider on the main thread once it is available.
///
/// The menu entries still have to be declared in the bundle's `NSServices`, and
/// the bundle has to be registered with LaunchServices — an unpackaged
/// `cargo run` binary installs fine and still shows nothing in Finder.
#[cfg(target_os = "macos")]
pub fn install(verbs: &[ShellVerb], handler: impl Fn(&str, Vec<String>) + 'static) -> Result<(), ServiceError> {
    HANDLER.with(|cell| *cell.borrow_mut() = Some(Box::new(handler)));
    SELECTORS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .extend(verbs.iter().map(|verb| (verb.selector, verb.id)));
    /* Apps register while building their launch object, before SDL owns the
       main thread, so the provider is applied on the first tick instead. */
    flush_pending();
    Ok(())
}

/// Publishes `verbs` to the platform file menu. See the macOS overload; other
/// platforms have no runtime publication path.
#[cfg(not(target_os = "macos"))]
pub fn install(_verbs: &[ShellVerb], _handler: impl Fn(&str, Vec<String>) + 'static) -> Result<(), ServiceError> {
    Err(ServiceError::Unsupported)
}

/// Applies registrations that arrived before the platform was ready for them.
/// Idempotent; the platform loop calls it each tick.
#[cfg(target_os = "macos")]
pub fn flush_pending() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    if !crate::is_main_thread() {
        return;
    }
    if SELECTORS.lock().unwrap_or_else(|error| error.into_inner()).is_empty() {
        return;
    }
    INSTALLED.call_once(|| {
        // SAFETY: main thread, once, and the callback outlives the process.
        bridge::install_provider();
    });
}

/// No-op: no platform publishes runtime verbs here.
#[cfg(not(target_os = "macos"))]
pub fn flush_pending() {}

/// Runs the queued activations on the calling (UI) thread. Call once per frame
/// from the host; a no-op when nothing arrived.
pub fn poll() {
    let pending: Vec<Activation> = {
        let mut queue = QUEUE.lock().unwrap_or_else(|error| error.into_inner());
        std::mem::take(&mut *queue)
    };
    if pending.is_empty() {
        return;
    }
    HANDLER.with(|cell| {
        let borrowed = cell.borrow();
        let Some(handler) = borrowed.as_ref() else {
            return;
        };
        for activation in pending {
            handler(&activation.verb, activation.paths);
        }
    });
}

#[cfg(target_os = "macos")]
mod bridge {
    use super::*;
    use std::cell::RefCell;
    use std::ffi::{CStr, CString, c_char};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    unsafe extern "C" {
        fn cove_shell_services_install(callback: extern "C" fn(*const c_char, *const *const c_char, usize) -> *const c_char) -> bool;
        fn cove_shell_wake();
    }

    // The message AppKit shows for the current activation. The native side
    // copies it before returning, so a thread-local slot is enough.
    thread_local! {
        static MESSAGE: RefCell<Option<CString>> = const { RefCell::new(None) };
    }

    pub(super) fn install_provider() -> bool {
        // SAFETY: called on the main thread as `cove_shell_services_install` requires.
        unsafe { cove_shell_services_install(service_trampoline) }
    }

    extern "C" fn service_trampoline(
        selector: *const c_char,
        paths: *const *const c_char,
        count: usize,
    ) -> *const c_char {
        let message = catch_unwind(AssertUnwindSafe(|| dispatch(selector, paths, count)))
            .unwrap_or_else(|_| Some("service handler panicked".to_string()));
        match message {
            Some(text) => MESSAGE.with(|slot| {
                *slot.borrow_mut() = CString::new(text.replace('\0', " ")).ok();
                slot.borrow().as_ref().map_or(std::ptr::null(), |text| text.as_ptr())
            }),
            None => std::ptr::null(),
        }
    }

    fn dispatch(selector: *const c_char, paths: *const *const c_char, count: usize) -> Option<String> {
        let selector = if selector.is_null() {
            return Some("service arrived without a selector".to_string());
        } else {
            // SAFETY: the provider passes a NUL-terminated selector for this call.
            unsafe { CStr::from_ptr(selector) }.to_string_lossy().into_owned()
        };
        /* AppKit invokes the full selector — the `NSMessage` from NSServices with
           `userData:error:` appended — while registrations carry just the
           message part, so match on that. */
        let message = selector.strip_suffix(":userData:error:").unwrap_or(&selector);
        let Some(verb) = SELECTORS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .find(|(registered, _)| *registered == message)
            .map(|(_, id)| id.to_string())
        else {
            return Some(format!("no verb is registered for service '{message}'"));
        };
        let mut selected = Vec::with_capacity(count);
        for index in 0..count {
            // SAFETY: `count` entries were filled by the provider before the call.
            let entry = unsafe { *paths.add(index) };
            if entry.is_null() {
                continue;
            }
            // SAFETY: each entry is a NUL-terminated UTF-8 path owned by the call.
            selected.push(unsafe { CStr::from_ptr(entry) }.to_string_lossy().into_owned());
        }
        if selected.is_empty() {
            return Some("no files were selected".to_string());
        }
        QUEUE
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(Activation { verb, paths: selected });
        // SAFETY: wakes the platform loop; thread-safe by contract.
        unsafe { cove_shell_wake() };
        None
    }
}
