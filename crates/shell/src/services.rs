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
//! scripts, Dolphin service menus, Thunar actions, Nemo actions). Nothing here
//! publishes on Linux yet, and [`install`] reports [`ServiceError::Unsupported`].

use std::fmt;
use std::path::Path;

/// A verb published to the OS file menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellVerb {
    /// Stable id handed to the installed handler; also the Windows key name.
    pub id: &'static str,
    /// Menu label. Falls back to `id`, because an unlabelled descriptor is
    /// unusable as a menu entry.
    pub label: Option<&'static str>,
    /// macOS: the `NSMessage` selector declared in the bundle's `NSServices`.
    pub selector: &'static str,
}

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

/// Registry hive a verb key lives under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryScope {
    /// `HKCU\Software\Classes` — per-user install, no elevation.
    CurrentUser,
    /// `HKCR` — machine-wide; writing it requires elevation.
    Machine,
}

impl RegistryScope {
    fn root(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU\\Software\\Classes",
            Self::Machine => "HKCR",
        }
    }
}

/// Where Explorer offers the verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerbTarget {
    /// Right-click on any file (`*\shell`).
    AnyFile,
    /// Right-click on a folder (`Directory\shell`).
    Directory,
    /// Right-click on empty space inside a folder (`Directory\Background\shell`).
    Background,
    /// Right-click on one extension (`SystemFileAssociations\.zip\shell`).
    Extension(&'static str),
}

impl VerbTarget {
    fn key_suffix(self) -> String {
        match self {
            Self::AnyFile => "*".to_string(),
            Self::Directory => "Directory".to_string(),
            Self::Background => "Directory\\Background".to_string(),
            Self::Extension(extension) => format!("SystemFileAssociations\\.{}", extension.trim_start_matches('.')),
        }
    }
}

/// One registry value an installer has to write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    pub hive: &'static str,
    /// Key below the hive; a `name` of `None` means the key's default value.
    pub key: String,
    pub name: Option<String>,
    pub value: String,
}

/// Registry values publishing `verb` as a static Explorer verb whose command
/// runs `exe` with the clicked path.
///
/// Static verbs are legacy entries: Windows 11 folds them into "Show more
/// options", so a top-level entry needs an `IExplorerCommand` COM handler
/// instead. `MultiSelectModel` is written as `Single` because a static verb is
/// launched once per selected item; `Player` would be needed for one launch
/// carrying the whole selection.
pub fn windows_verb_entries(
    target: VerbTarget,
    verb: &ShellVerb,
    exe: &Path,
    scope: RegistryScope,
) -> Vec<RegistryEntry> {
    let root = scope.root();
    let hive = if scope == RegistryScope::CurrentUser { "HKCU" } else { "HKCR" };
    let key = format!("{}\\{}\\shell\\{}", root, target.key_suffix(), verb.id);
    let command_key = format!("{key}\\command");
    let exe = exe.display().to_string();
    vec![
        RegistryEntry { hive, key: key.clone(), name: Some("MUIVerb".to_string()), value: verb.label.unwrap_or(verb.id).to_string() },
        RegistryEntry { hive, key: key.clone(), name: Some("Icon".to_string()), value: format!("\"{exe}\",0") },
        RegistryEntry { hive, key: key.clone(), name: Some("MultiSelectModel".to_string()), value: "Single".to_string() },
        // The default value of the `command` subkey is the command line.
        RegistryEntry { hive, key: command_key, name: None, value: format!("\"{exe}\" \"%1\"") },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERB: ShellVerb = ShellVerb { id: "extract", label: Some("Extract here"), selector: "coveExtractService:" };

    #[test]
    fn user_scope_any_file_verb_writes_under_classes() {
        let entries = windows_verb_entries(VerbTarget::AnyFile, &VERB, Path::new("C:/Apps/zip.exe"), RegistryScope::CurrentUser);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].hive, "HKCU");
        assert_eq!(entries[0].key, "HKCU\\Software\\Classes\\*\\shell\\extract");
        assert_eq!(entries[0].name.as_deref(), Some("MUIVerb"));
        assert_eq!(entries[0].value, "Extract here");
        assert_eq!(entries[3].key, "HKCU\\Software\\Classes\\*\\shell\\extract\\command");
        assert_eq!(entries[3].name, None);
        assert_eq!(entries[3].value, "\"C:/Apps/zip.exe\" \"%1\"");
    }

    #[test]
    fn machine_scope_uses_hkcr_and_needs_elevation() {
        let entries = windows_verb_entries(VerbTarget::Directory, &VERB, Path::new("C:/zip.exe"), RegistryScope::Machine);
        assert_eq!(entries[0].hive, "HKCR");
        assert_eq!(entries[0].key, "HKCR\\Directory\\shell\\extract");
    }

    #[test]
    fn background_and_extension_targets_get_their_own_keys() {
        let background = windows_verb_entries(VerbTarget::Background, &VERB, Path::new("C:/zip.exe"), RegistryScope::CurrentUser);
        assert_eq!(background[0].key, "HKCU\\Software\\Classes\\Directory\\Background\\shell\\extract");
        let zip = windows_verb_entries(VerbTarget::Extension(".zip"), &VERB, Path::new("C:/zip.exe"), RegistryScope::CurrentUser);
        assert_eq!(zip[0].key, "HKCU\\Software\\Classes\\SystemFileAssociations\\.zip\\shell\\extract");
    }

    #[test]
    fn command_keeps_an_executable_path_with_spaces_quoted() {
        let entries = windows_verb_entries(VerbTarget::AnyFile, &VERB, Path::new("C:/Program Files/Cove/zip.exe"), RegistryScope::CurrentUser);
        assert_eq!(entries[3].value, "\"C:/Program Files/Cove/zip.exe\" \"%1\"");
        assert_eq!(entries[1].value, "\"C:/Program Files/Cove/zip.exe\",0");
    }
}
