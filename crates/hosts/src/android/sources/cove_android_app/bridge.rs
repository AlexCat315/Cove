//! Translation of Android/CoveAndroidApp/Bridge.swift.
//!
//! Swift's bridge was a set of `@_cdecl` entry points that the Java activity
//! called through JNI, handing raw pointers to `AndroidApplication` and
//! `SurfaceSession`. The Rust port keeps the exact same C ABI so the existing
//! Java/Kotlin side needs no change, and keeps the same argument validation,
//! UTF-16 string decoding and JSON payload shapes.
//!
//! The NDK symbols the bridge needs (`__android_log_write`, and the
//! `ANativeWindow_*` accessors used by the surface session) are resolved through
//! `dlopen`/`dlsym` at first use, so this crate gains no new dependency and the
//! module still links on hosts that have no `libandroid` (the symbols simply
//! stay unresolved and every call reports absence rather than crashing).

use super::application::AndroidApplication;
use super::content::scene::HostedScene;
use super::frame::runtime_options::RuntimeOptions;
use super::surface_session::SurfaceSession;
use crate::android::contract::launch::SceneLaunch;
use cove_core::{Idiom, Insets, Keyboard, TextScale, TextScaleSample};
use cove_logging::{Level, LogEvent, LogSystem, LogValue};
use cove_platform::KeyModifiers;
use std::collections::BTreeMap;
use std::ffi::{CStr, c_void};
use std::rc::Rc;

// MARK: - NDK symbol resolution

#[cfg(target_os = "android")]
mod ndk {
    use super::*;
    use std::ffi::{c_char, c_int};

    /// `__android_log_write(prio, tag, text)`.
    pub type LogWrite = unsafe extern "C" fn(c_int, *const c_char, *const c_char) -> c_int;

    /// `ANativeWindow_getWidth`/`getHeight`/`getFormat` share one shape.
    pub type WindowQuery = unsafe extern "C" fn(*mut c_void) -> i32;

    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlerror() -> *const c_char;
    }

    const RTLD_NOW: c_int = 2;

    struct Library(*mut c_void);

    // SAFETY: the handle is never mutated after construction and dlopen handles
    // are process-wide and safe to share between threads.
    unsafe impl Send for Library {}
    unsafe impl Sync for Library {}

    impl Library {
        fn open(name: &CStr) -> Option<Self> {
            // SAFETY: `name` is a NUL terminated library name and `RTLD_NOW`
            // resolves every symbol eagerly.
            let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
            if handle.is_null() {
                // SAFETY: `dlerror` returns a NUL terminated diagnostic or null.
                let _ = unsafe { dlerror() };
                return None;
            }
            Some(Self(handle))
        }

        fn symbol<T: Copy>(&self, name: &CStr) -> Option<T> {
            // SAFETY: `name` is NUL terminated and the handle is live for the
            // process lifetime. The caller guarantees `T` is the symbol's type.
            let pointer = unsafe { dlsym(self.0, name.as_ptr()) };
            if pointer.is_null() {
                None
            } else {
                Some(unsafe { std::mem::transmute_copy(&pointer) })
            }
        }
    }

    /// `liblog.so` exists on every Android release; other hosts resolve no
    /// handle and log through the platform logging crate instead.
    fn log_library() -> Option<&'static Library> {
        static LIBRARY: std::sync::OnceLock<Option<Library>> = std::sync::OnceLock::new();
        LIBRARY.get_or_init(|| Library::open(c"liblog.so")).as_ref()
    }

    /// `libandroid.so` provides the `ANativeWindow` accessors.
    fn android_library() -> Option<&'static Library> {
        static LIBRARY: std::sync::OnceLock<Option<Library>> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| Library::open(c"libandroid.so"))
            .as_ref()
    }

    /// Writes one record to logcat under `tag`.
    pub fn log_write(tag: &CStr, message: &CStr) -> bool {
        const ANDROID_LOG_INFO: c_int = 4;
        let Some(write) =
            log_library().and_then(|library| library.symbol::<LogWrite>(c"__android_log_write"))
        else {
            return false;
        };
        // SAFETY: both C strings are borrowed for the duration of the call.
        (unsafe { write(ANDROID_LOG_INFO, tag.as_ptr(), message.as_ptr()) }) >= 0
    }

    /// Queries one `ANativeWindow` extent. `which` selects width, height or
    /// format through the resolved accessor.
    pub fn window_query(window: *mut c_void, which: &CStr) -> Option<i32> {
        let query = android_library()?.symbol::<WindowQuery>(which)?;
        // SAFETY: the caller passes the `ANativeWindow*` its Java surface gave
        // us; the accessor only reads the window's current geometry.
        Some(unsafe { query(window) })
    }

    /// Adds a reference to `window`.
    pub fn window_acquire(window: *mut c_void) -> bool {
        type Acquire = unsafe extern "C" fn(*mut c_void);
        let Some(acquire) = android_library()
            .and_then(|library| library.symbol::<Acquire>(c"ANativeWindow_acquire"))
        else {
            return false;
        };
        // SAFETY: the caller holds a live reference to this `ANativeWindow*`.
        unsafe { acquire(window) };
        true
    }

    /// Drops a reference taken by [`window_acquire`].
    pub fn window_release(window: *mut c_void) {
        type Release = unsafe extern "C" fn(*mut c_void);
        if let Some(release) = android_library()
            .and_then(|library| library.symbol::<Release>(c"ANativeWindow_release"))
        {
            // SAFETY: releases exactly the reference `window_acquire` added.
            unsafe { release(window) };
        }
    }
}

#[cfg(not(target_os = "android"))]
mod ndk {
    use super::*;

    pub fn log_write(_tag: &CStr, _message: &CStr) -> bool {
        false
    }

    pub fn window_acquire(_window: *mut c_void) -> bool {
        true
    }

    pub fn window_release(_window: *mut c_void) {}

    pub fn window_query(_window: *mut c_void, _which: &CStr) -> Option<i32> {
        None
    }
}

/// The `ANativeWindow` accessors the surface session needs beyond the raw
/// pointer handed in by Java.
pub struct NativeWindow;

impl NativeWindow {
    /// Pixel width reported by the NDK, or `None` when the symbol is absent.
    pub fn width(window: *mut c_void) -> Option<i32> {
        ndk::window_query(window, c"ANativeWindow_getWidth")
    }

    pub fn height(window: *mut c_void) -> Option<i32> {
        ndk::window_query(window, c"ANativeWindow_getHeight")
    }

    /// `WINDOW_FORMAT_RGBA_8888` is `1`; other values mean the buffer format
    /// must be translated before a swapchain is created.
    pub fn format(window: *mut c_void) -> Option<i32> {
        ndk::window_query(window, c"ANativeWindow_getFormat")
    }

    pub fn library_available() -> bool {
        cfg!(target_os = "android")
    }
}

/// An owned reference to an `ANativeWindow`, valid on any thread until dropped.
pub struct AcquiredNativeWindow(usize);

impl AcquiredNativeWindow {
    pub fn acquire(window: usize) -> Option<Self> {
        ndk::window_acquire(window as *mut c_void).then_some(Self(window))
    }

    pub fn raw(&self) -> usize {
        self.0
    }
}

impl Drop for AcquiredNativeWindow {
    fn drop(&mut self) {
        ndk::window_release(self.0 as *mut c_void);
    }
}

// MARK: - C ABI

/// Runtime handle the Java side holds. The `Box` keeps the address stable.
pub type ApplicationPointer = *mut c_void;
pub type SessionPointer = *mut c_void;

/// Java's `SurfaceSession` counterpart; retained by the pointer Java keeps.
pub struct SessionHandle {
    session: SurfaceSession,
}

impl SessionHandle {
    pub fn session(&self) -> &SurfaceSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut SurfaceSession {
        &mut self.session
    }
}

/// Rebuilds a `&str` from the UTF-16 buffer a JNI caller passes.
///
/// # Safety
/// `pointer` must be null or point to `count` readable `u16` values.
unsafe fn utf16_string(pointer: *const u16, count: i32) -> String {
    if pointer.is_null() || count <= 0 {
        return String::new();
    }
    // SAFETY: the caller guarantees `count` readable UTF-16 code units.
    let units = unsafe { std::slice::from_raw_parts(pointer, count as usize) };
    String::from_utf16_lossy(units)
}

/// Copies `text` into a UTF-16 output buffer and returns the length in units,
/// matching the Swift `copyText` contract: the caller passes a null buffer first
/// to learn the required capacity.
///
/// # Safety
/// `output` must be null or point to `capacity` writable `u16` values.
unsafe fn copy_text(text: &str, output: *mut u16, capacity: i32) -> i32 {
    let value: Vec<u16> = text.encode_utf16().collect();
    if !output.is_null() && capacity >= value.len() as i32 {
        // SAFETY: the caller guaranteed `capacity` writable units.
        unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), output, value.len()) };
    }
    value.len() as i32
}

/// Mirrors Swift's `Logger.Level.allCases` index mapping.
fn level_from_index(index: i32) -> Level {
    const LEVELS: [Level; 7] = [
        Level::Trace,
        Level::Debug,
        Level::Info,
        Level::Notice,
        Level::Warning,
        Level::Error,
        Level::Critical,
    ];
    LEVELS.get(index as usize).copied().unwrap_or(Level::Info)
}

fn log(message: &str, level: Level, label: &str) {
    let mut metadata = BTreeMap::new();
    metadata.insert("origin".to_owned(), LogValue::String("java".to_owned()));
    LogSystem::logger(label).log(&LogEvent::new(level, message).with_metadata(metadata));
    // Mirror into logcat so a native record is also visible to `adb logcat`,
    // which is what the Java side reads. On other hosts the symbol is absent and
    // this is a no-op.
    forward_to_logcat(message);
}

/// Forwards one record to Android's log facility when `liblog` is available.
fn forward_to_logcat(message: &str) {
    let Ok(tag) = std::ffi::CString::new("CoveHost") else {
        return;
    };
    let Ok(text) = std::ffi::CString::new(message.replace('\0', "")) else {
        return;
    };
    ndk::log_write(&tag, &text);
}

/// `cove_android_log_java`
///
/// # Safety
/// The label and message pointers must be null or valid UTF-16 buffers of the
/// given counts, as JNI guarantees for a `jstring` critical section.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_log_java(
    level: i32,
    label: *const u16,
    label_count: i32,
    message: *const u16,
    message_count: i32,
) {
    // SAFETY: forwarded pointer/count contract from the caller.
    let label = unsafe { utf16_string(label, label_count) };
    // SAFETY: forwarded pointer/count contract from the caller.
    let message = unsafe { utf16_string(message, message_count) };
    log(&message, level_from_index(level), &label);
}

/// `cove_android_create_application`
///
/// Decodes the launch and runtime-option documents the APK producer wrote, then
/// retains a new [`AndroidApplication`]. Returns null when either document is
/// invalid or initialisation fails, exactly as the Swift bridge did.
///
/// # Safety
/// Every pointer must be null or a valid UTF-16 buffer of the given count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_create_application(
    scale: f32,
    checkpoint: *const u16,
    checkpoint_count: i32,
    configuration: *const u16,
    configuration_count: i32,
    options: *const u16,
    options_count: i32,
) -> ApplicationPointer {
    // SAFETY: forwarded pointer/count contracts from the caller.
    let checkpoint = unsafe { utf16_string(checkpoint, checkpoint_count) };
    // SAFETY: forwarded pointer/count contracts from the caller.
    let configuration = unsafe { utf16_string(configuration, configuration_count) };
    // SAFETY: forwarded pointer/count contracts from the caller.
    let options = unsafe { utf16_string(options, options_count) };

    let launch = match SceneLaunch::from_json(configuration.as_bytes()) {
        Ok(launch) => launch,
        Err(error) => {
            log(
                &format!("Application initialization failed: {error}"),
                Level::Error,
                "com.cove.android.application",
            );
            return std::ptr::null_mut();
        }
    };
    // Remote image loading needs a platform transport; without this call the
    // holder stays empty and every http(s) image reports HostUnavailable.
    let capabilities = crate::application_services::install_platform_services();
    log(
        &format!(
            "Platform services installed http={} https={} file={}",
            capabilities.http, capabilities.https, capabilities.file
        ),
        Level::Info,
        "com.cove.android.application",
    );
    let runtime = match RuntimeOptions::from_json(options.as_bytes()) {
        Ok(runtime) => runtime,
        Err(error) => {
            log(
                &format!("Application initialization failed: {error}"),
                Level::Error,
                "com.cove.android.application",
            );
            return std::ptr::null_mut();
        }
    };
    if let Err(error) = LogSystem::bootstrap(
        Some(&runtime.logging.application),
        runtime.logging.configuration.clone(),
        Some(runtime.logging.run_id.clone()),
    ) {
        log(
            &format!("Application initialization failed: {error}"),
            Level::Error,
            "com.cove.android.application",
        );
        return std::ptr::null_mut();
    }
    log(
        "Application starting",
        Level::Info,
        "com.cove.android.application",
    );
    match AndroidApplication::new(scale, &checkpoint, launch, runtime) {
        Ok(application) => Rc::into_raw(application).cast_mut().cast(),
        Err(error) => {
            log(
                &format!("Application initialization failed: {error}"),
                Level::Error,
                "com.cove.android.application",
            );
            std::ptr::null_mut()
        }
    }
}

/// Reclaims an [`AndroidApplication`] created by
/// `cove_android_create_application`.
///
/// # Safety
/// `pointer` must be null or a pointer returned by
/// `cove_android_create_application` that has not already been destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_destroy_application(pointer: ApplicationPointer) {
    if !pointer.is_null() {
        // SAFETY: the pointer came from `Rc::into_raw` and is consumed once.
        drop(unsafe { Rc::from_raw(pointer.cast::<AndroidApplication>()) });
    }
    LogSystem::flush();
}

/// Borrows the application behind a Java-held pointer.
///
/// `Rc::from_raw` + `mem::forget` would briefly build a second owner of the same
/// allocation. Borrowing the pointee as a plain reference is both simpler and
/// exact: JNI guarantees the pointer stays retained for the whole call.
///
/// # Safety
/// `pointer` must be null or a live application pointer.
unsafe fn borrow_application(pointer: ApplicationPointer) -> Option<Rc<AndroidApplication>> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees a live retained pointer. Rebuilding the `Rc`
    // and forgetting it clones the count without taking ownership, so the Java
    // side keeps its own reference.
    let retained = unsafe { Rc::from_raw(pointer.cast::<AndroidApplication>()) };
    let cloned = retained.clone();
    std::mem::forget(retained);
    Some(cloned)
}

/// Borrows the session behind a Java-held pointer.
///
/// # Safety
/// `pointer` must be a live session pointer.
unsafe fn session(pointer: SessionPointer) -> Option<&'static mut SessionHandle> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees a live session pointer and JNI serializes
    // every call to it on the UI thread, so exclusive access holds.
    Some(unsafe { &mut *pointer.cast::<SessionHandle>() })
}

/// `cove_android_open`
///
/// # Safety
/// `application` must be a live application pointer and `window` the
/// `ANativeWindow*` the Java surface owns for the lifetime of the session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_open(
    application: ApplicationPointer,
    window: *mut c_void,
    width: i32,
    height: i32,
) -> SessionPointer {
    if window.is_null() || width <= 0 || height <= 0 {
        return std::ptr::null_mut();
    }
    // A surface can be reconfigured by the window system between the Java
    // callback and this call; prefer the geometry the NDK reports.
    let (width_px, height_px) = match (NativeWindow::width(window), NativeWindow::height(window)) {
        (Some(0) | None, _) | (_, Some(0) | None) => (width as usize, height as usize),
        (Some(ndk_width), Some(ndk_height)) => (ndk_width as usize, ndk_height as usize),
    };
    // SAFETY: forwarded application pointer contract from the caller.
    let Some(retained) = (unsafe { borrow_application(application) }) else {
        return std::ptr::null_mut();
    };
    match SurfaceSession::new(retained, window as usize, width_px, height_px) {
        Ok(session) => Box::into_raw(Box::new(SessionHandle { session })).cast(),
        Err(error) => {
            log(
                &format!("Surface initialization failed: {error}"),
                Level::Error,
                "com.cove.android.surface",
            );
            std::ptr::null_mut()
        }
    }
}

/// `cove_android_frame`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_frame(pointer: SessionPointer, timestamp: u64) -> i32 {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return 0;
    };
    match handle.session_mut().frame(timestamp) {
        Ok(presented) => i32::from(presented),
        Err(error) => {
            log(
                &format!("Frame failed: {error}"),
                Level::Error,
                "com.cove.android.frame",
            );
            -1
        }
    }
}

/// `cove_android_pointer`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_pointer(pointer: SessionPointer, kind: i32, x: f32, y: f32) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    handle.session().scene().pointer(kind, x, y);
}

/// `cove_android_text`
///
/// # Safety
/// `pointer` must be null or a live session pointer; `text` must be null or a
/// valid UTF-16 buffer of `count` units.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_text(
    pointer: SessionPointer,
    text: *const u16,
    count: i32,
    composing: i32,
) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    // SAFETY: forwarded pointer/count contract from the caller.
    let text = unsafe { utf16_string(text, count) };
    // SAFETY: the handle is live for the duration of this call.
    handle.session().scene().text(&text, composing != 0);
}

/// `cove_android_key`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_key(pointer: SessionPointer, scancode: u32, modifiers: u16) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    handle
        .session()
        .scene()
        .key(scancode, KeyModifiers(modifiers));
}

/// `cove_android_scroll`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_scroll(
    pointer: SessionPointer,
    x: f32,
    y: f32,
    delta_x: f32,
    delta_y: f32,
) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    handle.session().scene().scroll(x, y, delta_x, delta_y);
}

/// `cove_android_surface`
///
/// Reports device facts the window itself cannot derive: density, idiom, safe
/// area, keyboard height and the accessibility text-size samples.
///
/// # Safety
/// `pointer` must be null or a live session pointer; `text_sizes` must be null
/// or point to `text_count` readable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_surface(
    pointer: SessionPointer,
    density: f32,
    idiom: i32,
    top: f32,
    leading: f32,
    bottom: f32,
    trailing: f32,
    keyboard: f32,
    text_sizes: *const f32,
    text_count: i32,
) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    if text_sizes.is_null() || !(0..=128).contains(&text_count) || text_count % 2 != 0 {
        return;
    }
    // SAFETY: the caller guarantees `text_count` readable floats.
    let sizes = unsafe { std::slice::from_raw_parts(text_sizes, text_count as usize) };
    let text_scale =
        TextScale::from_samples(sizes.as_chunks::<2>().0.iter().map(|pair| TextScaleSample {
            size: pair[0],
            scaled_size: pair[1],
        }));
    handle.session().application().update_scale(density);
    let safe_area = Insets {
        top,
        bottom,
        leading,
        trailing,
    };
    let idiom = match idiom {
        0 => Idiom::Phone,
        1 => Idiom::Tablet,
        _ => Idiom::Desktop,
    };
    handle.session().scene().content().update_surface(
        idiom,
        safe_area,
        Keyboard::new(keyboard),
        text_scale,
    );
}

/// `cove_android_system_bar_icons`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_system_bar_icons(pointer: SessionPointer) -> i32 {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return -1;
    };
    // SAFETY: the handle is live for the duration of this call.
    handle.session().scene().content().system_bar_icon_style()
}

/// `cove_android_key_state`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_key_state(pointer: SessionPointer, scancode: u32, down: i32) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    if down != 0 {
        // SAFETY: the handle is live for the duration of this call.
        handle
            .session()
            .scene()
            .key(scancode, KeyModifiers::default());
    }
}

/// `cove_android_snapshot`
///
/// # Safety
/// `pointer` must be null or a live session pointer; `output` must be null or
/// point to `capacity` writable UTF-16 units.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_snapshot(
    pointer: SessionPointer,
    output: *mut u16,
    capacity: i32,
) -> i32 {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return 0;
    };
    // SAFETY: the handle is live for the duration of this call.
    let text = handle.session().scene().snapshot();
    // SAFETY: forwarded output contract from the caller.
    unsafe { copy_text(&text, output, capacity) }
}

/// `cove_android_input_snapshot`
///
/// # Safety
/// `pointer` must be null or a live session pointer; `output` must be null or
/// point to `capacity` writable UTF-16 units.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_input_snapshot(
    pointer: SessionPointer,
    output: *mut u16,
    capacity: i32,
) -> i32 {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return 0;
    };
    // SAFETY: the handle is live for the duration of this call.
    let text = handle.session().scene().input_snapshot();
    // SAFETY: forwarded output contract from the caller.
    unsafe { copy_text(&text, output, capacity) }
}

/// `cove_android_edit`
///
/// # Safety
/// `pointer` must be null or a live session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_edit(
    pointer: SessionPointer,
    kind: i32,
    start: i32,
    end: i32,
) {
    let Some(handle) = (unsafe { session(pointer) }) else {
        return;
    };
    handle.session().scene().edit(kind, start, end);
}

/// `cove_android_close`
///
/// # Safety
/// `pointer` must be null or a session pointer that has not already been
/// closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_close(pointer: SessionPointer) {
    if pointer.is_null() {
        return;
    }
    // SAFETY: the pointer came from `Box::into_raw` in `cove_android_open` and
    // is consumed exactly once here.
    let mut handle = unsafe { Box::from_raw(pointer.cast::<SessionHandle>()) };
    handle.session.scene().suspend();
    handle.session.close();
    LogSystem::flush();
}

/// `cove_android_pump_diagnostics`
///
/// # Safety
/// `pointer` must be null or a live application pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_pump_diagnostics(pointer: ApplicationPointer) {
    // SAFETY: forwarded application pointer contract from the caller.
    if let Some(application) = unsafe { borrow_application(pointer) } {
        application.scene().borrow().content().pump_diagnostics();
    }
}

/// `cove_android_memory_pressure`
///
/// # Safety
/// `pointer` must be null or a live application pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cove_android_memory_pressure(pointer: ApplicationPointer) {
    // SAFETY: forwarded application pointer contract from the caller.
    if let Some(application) = unsafe { borrow_application(pointer) } {
        application.trim_memory();
    }
}

/// Convenience for the platform entry: converts a Java `jstring` critical
/// section's UTF-16 buffer into a `HostedScene` snapshot without going through
/// the C ABI.
pub fn scene_snapshot(scene: &HostedScene) -> String {
    scene.snapshot()
}
