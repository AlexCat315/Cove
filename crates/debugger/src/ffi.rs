//! Runtime loading of the native LLDB shim.
//!
//! The shim is compiled by `build.rs` into a cdylib and embedded here with
//! `include_bytes!`. Nothing in the controller links liblldb: on first use the
//! shim is written to a private temp directory, liblldb is preloaded so the
//! shim's imports resolve, and the shim is opened with `LoadLibraryW`/`dlopen`.
//! Every entry point is then resolved into a function table.
//!
//! When liblldb was absent at build time the embedded shim is the stub, whose
//! `cove_lldb_initialize` returns [`UNAVAILABLE`]; the loader still succeeds so
//! the backend can report that truthfully and the DAP adapter stays the fallback.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Return codes from the C ABI.
pub const OK: c_int = 0;

#[cfg(test)]
pub const UNAVAILABLE: c_int = 2;

/// Session states, mirroring `cove_lldb.h`.
#[cfg(test)]
pub const STATE_STARTING: c_int = 0;
pub const STATE_ATTACHED: c_int = 1;
pub const STATE_RUNNING: c_int = 2;
pub const STATE_STOPPED: c_int = 3;
pub const STATE_EXITED: c_int = 4;
pub const STATE_TERMINATED: c_int = 5;
pub const STATE_CLOSED: c_int = 6;

/// Stop reasons, mirroring `cove_lldb.h`.
pub const STOP_NONE: c_int = 0;
pub const STOP_BREAKPOINT: c_int = 1;
pub const STOP_STEP: c_int = 2;
pub const STOP_SIGNAL: c_int = 3;
pub const STOP_EXCEPTION: c_int = 4;
pub const STOP_WATCHPOINT: c_int = 5;

/// Step kinds, mirroring `cove_lldb.h`.
pub const STEP_OVER: c_int = 0;
pub const STEP_IN: c_int = 1;
pub const STEP_OUT: c_int = 2;

/// Opaque session pointer owned by the shim.
pub type Session = *mut c_void;
/// Opaque frame/value handle; 0 means "none".
pub type Handle = u64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawState {
    pub state: c_int,
    pub stop_reason: c_int,
    pub thread_id: u64,
    pub exit_code: c_int,
    pub has_exit_code: c_int,
    pub breakpoint_ids: [u64; 8],
    pub num_breakpoints: c_int,
    pub description: [c_char; 512],
    pub process_name: [c_char; 256],
    pub pid: u32,
    pub stop_count: u64,
}

/// Output categories, mirroring `cove_lldb.h`.
pub const OUTPUT_STDOUT: c_int = 1;
pub const OUTPUT_STDERR: c_int = 2;
pub const OUTPUT_LOG: c_int = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawOutput {
    pub category: c_int,
    pub text: [c_char; 1024],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawThread {
    pub id: u64,
    pub name: [c_char; 128],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawFrame {
    pub id: Handle,
    pub name: [c_char; 256],
    pub path: [c_char; 512],
    pub line: u32,
    pub column: u32,
    pub pc: [c_char; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawScope {
    pub name: [c_char; 64],
    pub reference: Handle,
    pub expensive: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawVariable {
    pub name: [c_char; 128],
    pub value: [c_char; 1024],
    pub r#type: [c_char; 128],
    pub evaluate_name: [c_char; 256],
    pub reference: Handle,
    pub num_children: u64,
    pub address: u64,
    pub has_address: c_int,
    pub value_truncated: c_int,
}

/// Copies a fixed C char array into a `String`, stopping at the first NUL.
pub fn char_array(bytes: &[c_char]) -> String {
    let bytes: Vec<u8> = bytes
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

// The raw entry-point signatures, kept identical to `cove_lldb.h`.
type InitFn = unsafe extern "C" fn() -> c_int;
type VersionFn = unsafe extern "C" fn() -> *const c_char;
type LastErrorFn = unsafe extern "C" fn() -> *const c_char;
type SessionCreateFn = unsafe extern "C" fn() -> Session;
type SessionDestroyFn = unsafe extern "C" fn(Session);
type AttachFn = unsafe extern "C" fn(Session, u32) -> c_int;
type AttachRemoteFn = unsafe extern "C" fn(Session, *const c_char, *const c_char) -> c_int;
type DetachFn = unsafe extern "C" fn(Session) -> c_int;
type AbortFn = unsafe extern "C" fn(Session);
type ContinueFn = unsafe extern "C" fn(Session) -> c_int;
type PauseFn = unsafe extern "C" fn(Session) -> c_int;
type StepFn = unsafe extern "C" fn(Session, u64, c_int) -> c_int;
type GetStateFn = unsafe extern "C" fn(Session, *mut RawState) -> c_int;
type WaitEventFn = unsafe extern "C" fn(Session, u32) -> c_int;
type WakeFn = unsafe extern "C" fn(Session);
type DrainOutputFn = unsafe extern "C" fn(Session, *mut RawOutput, c_int, *mut c_int) -> c_int;
type ThreadsFn = unsafe extern "C" fn(Session, *mut RawThread, c_int, *mut c_int) -> c_int;
type StackFn =
    unsafe extern "C" fn(Session, u64, u64, u64, *mut RawFrame, c_int, *mut u64) -> c_int;
type ScopesFn = unsafe extern "C" fn(Session, Handle, *mut RawScope, c_int, *mut c_int) -> c_int;
type VariablesFn =
    unsafe extern "C" fn(Session, Handle, u64, u64, *mut RawVariable, c_int, *mut u64) -> c_int;
type EvaluateFn =
    unsafe extern "C" fn(Session, *const c_char, Handle, u32, *mut RawVariable) -> c_int;
type BreakpointCreateFn = unsafe extern "C" fn(
    Session,
    *const c_char,
    u32,
    *const c_char,
    *const c_char,
    *mut u64,
    *mut c_int,
    *mut u32,
) -> c_int;
type BreakpointDeleteFn = unsafe extern "C" fn(Session, u64) -> c_int;
type BreakpointInfoFn = unsafe extern "C" fn(Session, u64, *mut c_int, *mut u32) -> c_int;
type ExceptionFilterFn = unsafe extern "C" fn(Session, *const c_char, c_int) -> c_int;

/// A loaded shim with every entry point resolved.
///
/// The handle is only ever used to call these functions and is never freed while
/// the process lives, so sharing it across the session's threads is sound; the
/// shim serializes its own mutable state with a mutex.
pub struct Library {
    _shim: Module,
    _runtime: Option<Module>,
    pub init: InitFn,
    pub version: VersionFn,
    pub last_error: LastErrorFn,
    pub session_create: SessionCreateFn,
    pub session_destroy: SessionDestroyFn,
    pub attach: AttachFn,
    pub attach_remote: AttachRemoteFn,
    pub detach: DetachFn,
    pub abort: AbortFn,
    pub continue_: ContinueFn,
    pub pause: PauseFn,
    pub step: StepFn,
    pub get_state: GetStateFn,
    pub wait_event: WaitEventFn,
    pub wake: WakeFn,
    pub drain_output: DrainOutputFn,
    pub threads: ThreadsFn,
    pub stack: StackFn,
    pub scopes: ScopesFn,
    pub variables: VariablesFn,
    pub evaluate: EvaluateFn,
    pub breakpoint_create: BreakpointCreateFn,
    pub breakpoint_delete: BreakpointDeleteFn,
    pub breakpoint_info: BreakpointInfoFn,
    pub set_exception_filter: ExceptionFilterFn,
}

// SAFETY: the module handle is a process-lifetime loader handle used only to
// dispatch the resolved C functions; it is never closed or mutated concurrently.
unsafe impl Send for Library {}
// SAFETY: concurrent calls through the resolved functions are sound; the shim
// guards its shared state internally.
unsafe impl Sync for Library {}

/// An opened dynamic module, kept open for the process lifetime.
struct Module(#[allow(dead_code)] *mut c_void);

struct State {
    library: Option<Library>,
    reason: Option<String>,
}

static STATE: OnceLock<State> = OnceLock::new();

/// The process-wide shim, loaded once. `None` when it could not be loaded.
pub fn library() -> Option<&'static Library> {
    STATE.get_or_init(load).library.as_ref()
}

/// Why the native backend is unavailable, for diagnostics. `None` when usable.
pub fn unavailable_reason() -> Option<&'static str> {
    STATE.get_or_init(load).reason.as_deref()
}

fn load() -> State {
    match try_load() {
        Ok(library) => State {
            library: Some(library),
            reason: None,
        },
        Err(reason) => State {
            library: None,
            reason: Some(reason),
        },
    }
}

fn try_load() -> Result<Library, String> {
    let shim_path = write_shim().map_err(|error| format!("cannot stage the LLDB shim: {error}"))?;

    // Preload liblldb so the shim's imports resolve regardless of where the shim
    // was staged. Best effort: on Unix the shim also carries an rpath.
    // SAFETY: the path names a dynamic library; loading runs its initializers.
    let runtime = runtime_path().and_then(|path| unsafe { open_module(&path) });

    // SAFETY: the staged shim is the library this crate built.
    let shim = unsafe { open_module(&shim_path) }
        .ok_or_else(|| format!("cannot load {}: {}", shim_path.display(), last_dl_error()))?;

    // SAFETY: `shim` is the module just opened from the embedded shim.
    let library = unsafe { resolve_table(shim, runtime) }?;
    Ok(library)
}

/// The liblldb runtime to preload: `COVE_LIBLLDB`, the build-time discovery,
/// then the platform's usual install locations.
pub fn runtime_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("COVE_LIBLLDB") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let embedded = env!("COVE_LLDB_LIB");
    if !embedded.is_empty() {
        let path = PathBuf::from(embedded);
        if path.is_file() {
            return Some(path);
        }
    }
    installed_runtimes().into_iter().find(|path| path.is_file())
}

#[cfg(windows)]
fn installed_runtimes() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("LLVM/bin/liblldb.dll"))
        .collect()
}

#[cfg(target_os = "macos")]
fn installed_runtimes() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/opt/homebrew/opt/llvm/lib/liblldb.dylib"),
        PathBuf::from("/usr/local/opt/llvm/lib/liblldb.dylib"),
        PathBuf::from(
            "/Library/Developer/CommandLineTools/Library/PrivateFrameworks/LLDB.framework/LLDB",
        ),
        PathBuf::from("/Applications/Xcode.app/Contents/SharedFrameworks/LLDB.framework/LLDB"),
    ];
    if let Ok(entries) = std::fs::read_dir("/Applications") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Xcode") && name.ends_with(".app") {
                paths.push(
                    entry
                        .path()
                        .join("Contents/SharedFrameworks/LLDB.framework/LLDB"),
                );
            }
        }
    }
    paths
}

#[cfg(all(unix, not(target_os = "macos")))]
fn installed_runtimes() -> Vec<PathBuf> {
    let mut directories = vec![PathBuf::from("/usr/local/lib")];
    for base in ["/usr/lib", "/usr/lib64"] {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        let mut nested: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("llvm-") || name.ends_with("-linux-gnu"))
            })
            .collect();
        nested.sort();
        for path in nested.into_iter().rev() {
            directories.push(path.join("lib"));
            directories.push(path);
        }
        directories.push(PathBuf::from(base));
    }
    let mut paths = Vec::new();
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("liblldb") && name.contains(".so"))
            })
            .collect();
        found.sort();
        paths.extend(found.into_iter().rev());
    }
    paths
}

#[cfg(not(any(windows, unix)))]
fn installed_runtimes() -> Vec<PathBuf> {
    Vec::new()
}

/// Writes the embedded shim to a private, content-addressed temp path.
fn write_shim() -> std::io::Result<PathBuf> {
    use std::hash::{Hash, Hasher};
    let bytes: &[u8] = include_bytes!(env!("COVE_LLDB_SHIM"));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let dir = std::env::temp_dir().join("cove-lldb-shim");
    std::fs::create_dir_all(&dir)?;
    let name = format!("{:016x}{}", hasher.finish(), shim_suffix());
    let path = dir.join(&name);
    if path.is_file() && std::fs::metadata(&path)?.len() == bytes.len() as u64 {
        return Ok(path);
    }
    // Stage beside the target then rename, so a concurrent reader never sees a
    // partially written library.
    let staging = dir.join(format!("{name}.{}.tmp", std::process::id()));
    std::fs::write(&staging, bytes)?;
    match std::fs::rename(&staging, &path) {
        Ok(()) => Ok(path),
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            // Another process may have won the race and produced a valid file.
            if path.is_file() && std::fs::metadata(&path)?.len() == bytes.len() as u64 {
                Ok(path)
            } else {
                Err(error)
            }
        }
    }
}

fn shim_suffix() -> &'static str {
    if cfg!(windows) {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

/// Resolves every entry point; a missing symbol fails the whole load.
///
/// # Safety
/// `shim` must be a valid module handle exporting the `cove_lldb_*` symbols.
unsafe fn resolve_table(shim: Module, runtime: Option<Module>) -> Result<Library, String> {
    let handle = shim.0;
    macro_rules! symbol {
        ($name:literal, $ty:ty) => {{
            // SAFETY: the caller guarantees `handle` exports this symbol with this signature.
            unsafe {
                let pointer = lookup(handle, $name)
                    .ok_or_else(|| format!("the LLDB shim is missing {}", $name))?;
                std::mem::transmute::<*mut c_void, $ty>(pointer)
            }
        }};
    }
    Ok(Library {
        _shim: shim,
        _runtime: runtime,
        init: symbol!("cove_lldb_initialize", InitFn),
        version: symbol!("cove_lldb_version", VersionFn),
        last_error: symbol!("cove_lldb_last_error", LastErrorFn),
        session_create: symbol!("cove_lldb_session_create", SessionCreateFn),
        session_destroy: symbol!("cove_lldb_session_destroy", SessionDestroyFn),
        attach: symbol!("cove_lldb_attach", AttachFn),
        attach_remote: symbol!("cove_lldb_attach_remote", AttachRemoteFn),
        detach: symbol!("cove_lldb_detach", DetachFn),
        abort: symbol!("cove_lldb_abort", AbortFn),
        continue_: symbol!("cove_lldb_continue", ContinueFn),
        pause: symbol!("cove_lldb_pause", PauseFn),
        step: symbol!("cove_lldb_step", StepFn),
        get_state: symbol!("cove_lldb_get_state", GetStateFn),
        wait_event: symbol!("cove_lldb_wait_event", WaitEventFn),
        wake: symbol!("cove_lldb_wake", WakeFn),
        drain_output: symbol!("cove_lldb_drain_output", DrainOutputFn),
        threads: symbol!("cove_lldb_threads", ThreadsFn),
        stack: symbol!("cove_lldb_stack", StackFn),
        scopes: symbol!("cove_lldb_scopes", ScopesFn),
        variables: symbol!("cove_lldb_variables", VariablesFn),
        evaluate: symbol!("cove_lldb_evaluate", EvaluateFn),
        breakpoint_create: symbol!("cove_lldb_breakpoint_create", BreakpointCreateFn),
        breakpoint_delete: symbol!("cove_lldb_breakpoint_delete", BreakpointDeleteFn),
        breakpoint_info: symbol!("cove_lldb_breakpoint_info", BreakpointInfoFn),
        set_exception_filter: symbol!("cove_lldb_set_exception_filter", ExceptionFilterFn),
    })
}

/// Reads the shim's thread-local error string, if any.
///
/// # Safety
/// `library` must be a resolved shim; the returned pointer is valid until the
/// next call on the same thread.
pub unsafe fn last_error(library: &Library) -> String {
    // SAFETY: the caller guarantees a resolved library; the string is copied at once.
    unsafe {
        let pointer = (library.last_error)();
        if pointer.is_null() {
            return String::new();
        }
        CStr::from_ptr(pointer).to_string_lossy().into_owned()
    }
}

#[cfg(windows)]
unsafe fn open_module(path: &Path) -> Option<Module> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::{
        LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // liblldb's own dependencies sit beside it, not beside the controller.
    // SAFETY: `wide` is a NUL-terminated path that outlives the call.
    let handle = unsafe {
        LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_WITH_ALTERED_SEARCH_PATH,
        )
    };
    if handle.is_null() {
        None
    } else {
        Some(Module(handle))
    }
}

#[cfg(windows)]
unsafe fn lookup(module: *mut c_void, name: &str) -> Option<*mut c_void> {
    use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
    let name = CString::new(name).ok()?;
    // SAFETY: `module` is a loaded module and `name` is NUL-terminated.
    let proc = unsafe { GetProcAddress(module as _, name.as_ptr() as *const u8) }?;
    Some(proc as usize as *mut c_void)
}

#[cfg(windows)]
fn last_dl_error() -> String {
    // SAFETY: GetLastError has no preconditions.
    format!("LoadLibrary failed (GetLastError={})", unsafe {
        windows_sys::Win32::Foundation::GetLastError()
    })
}

#[cfg(unix)]
unsafe fn open_module(path: &Path) -> Option<Module> {
    use std::os::unix::ffi::OsStrExt;
    let name = CString::new(path.as_os_str().as_bytes()).ok()?;
    let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
    if handle.is_null() {
        None
    } else {
        Some(Module(handle))
    }
}

#[cfg(unix)]
unsafe fn lookup(module: *mut c_void, name: &str) -> Option<*mut c_void> {
    let name = CString::new(name).ok()?;
    let pointer = unsafe { libc::dlsym(module, name.as_ptr()) };
    if pointer.is_null() {
        None
    } else {
        Some(pointer)
    }
}

#[cfg(unix)]
fn last_dl_error() -> String {
    unsafe {
        let error = libc::dlerror();
        if error.is_null() {
            "dlopen failed".to_owned()
        } else {
            CStr::from_ptr(error).to_string_lossy().into_owned()
        }
    }
}

#[cfg(not(any(windows, unix)))]
unsafe fn open_module(_path: &Path) -> Option<Module> {
    None
}

#[cfg(not(any(windows, unix)))]
unsafe fn lookup(_module: *mut c_void, _name: &str) -> Option<*mut c_void> {
    None
}

#[cfg(not(any(windows, unix)))]
fn last_dl_error() -> String {
    "dynamic loading is unsupported on this platform".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_shim_and_reports_truthfully() {
        let Some(library) = library() else {
            assert!(unavailable_reason().is_some());
            return;
        };
        // SAFETY: the library is resolved and the pointers are valid for the call.
        let code = unsafe { (library.init)() };
        if code == UNAVAILABLE {
            // SAFETY: the version string is static.
            let version = unsafe { CStr::from_ptr((library.version)()) };
            assert_eq!(version.to_string_lossy(), "unavailable");
            return;
        }
        assert_eq!(code, OK);
        // SAFETY: the version string is static.
        let version = unsafe { CStr::from_ptr((library.version)()) };
        assert!(!version.to_bytes().is_empty());

        // SAFETY: the library is resolved and the pointers are valid for the call.
        let session = unsafe { (library.session_create)() };
        assert!(!session.is_null());
        // SAFETY: RawState is plain data; zero is a valid value.
        let mut state: RawState = unsafe { std::mem::zeroed() };
        // SAFETY: the library is resolved and the pointers are valid for the call.
        let rc = unsafe { (library.get_state)(session, &mut state) };
        assert_eq!(rc, OK);
        assert_eq!(state.state, STATE_STARTING);
        // SAFETY: the session was created above and is not used again.
        unsafe { (library.session_destroy)(session) };
    }
}
