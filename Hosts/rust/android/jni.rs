//! JNI entry points for `com.cove.probe.MainActivity` and `RuntimeLog`.
//!
//! Translation of `Hosts/Android/Sources/CAndroidHost/{Host,Input}.c`. The
//! functions are `#[no_mangle]` Rust so a `cdylib` exports them; C objects in a
//! static archive would stay hidden behind rustc's export list. Only the JNI
//! table slots this host calls are bound; the indices are the `JNINativeInterface`
//! and `JNIInvokeInterface` layout from the NDK's `jni.h`.

use crate::android::sources::cove_android_app::bridge::{self, ApplicationPointer, SessionPointer};
use std::ffi::{c_char, c_void};

type JniEnv = *mut *const *const c_void;
type JavaVm = *mut *const *const c_void;
type Object = *mut c_void;

mod slot {
    pub const NEW_STRING: usize = 163;
    pub const GET_STRING_LENGTH: usize = 164;
    pub const GET_STRING_CHARS: usize = 165;
    pub const RELEASE_STRING_CHARS: usize = 166;
    pub const NEW_STRING_UTF: usize = 167;
    pub const GET_STRING_UTF_CHARS: usize = 169;
    pub const RELEASE_STRING_UTF_CHARS: usize = 170;
    pub const GET_ARRAY_LENGTH: usize = 171;
    pub const GET_FLOAT_ARRAY_ELEMENTS: usize = 189;
    pub const RELEASE_FLOAT_ARRAY_ELEMENTS: usize = 197;
}

const JNI_ABORT: i32 = 2;
const JNI_VERSION_1_6: i32 = 0x0001_0006;

unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: JniEnv, surface: Object) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
    /// SDL3's own `JNI_OnLoad`, renamed when Shell builds SDL for Android.
    fn CoveSDL_JNI_OnLoad(vm: JavaVm, reserved: *mut c_void) -> i32;
    fn cove_log_flush();
}

#[link(name = "android")]
unsafe extern "C" {}

/// Reads one function pointer from the JNI table.
///
/// # Safety
/// `env` must be a live `JNIEnv*` and `T` the exact C signature of `index`.
unsafe fn function<T: Copy>(env: JniEnv, index: usize) -> T {
    // SAFETY: the caller guarantees a live table whose `index` slot has type `T`.
    unsafe { std::mem::transmute_copy(&*(*env).add(index)) }
}

/// A Java string pinned as UTF-16 for the duration of a call.
struct Utf16<'a> {
    env: JniEnv,
    string: Object,
    units: *const u16,
    count: i32,
    _scope: std::marker::PhantomData<&'a ()>,
}

impl Utf16<'_> {
    /// # Safety
    /// `env` must be the current thread's `JNIEnv*` and `string` a `jstring`.
    unsafe fn new(env: JniEnv, string: Object) -> Option<Self> {
        if string.is_null() {
            return None;
        }
        // SAFETY: JNI slot signatures from jni.h.
        let units = unsafe {
            function::<unsafe extern "C" fn(JniEnv, Object, *mut u8) -> *const u16>(
                env,
                slot::GET_STRING_CHARS,
            )(env, string, std::ptr::null_mut())
        };
        if units.is_null() {
            return None;
        }
        // SAFETY: JNI slot signatures from jni.h.
        let count = unsafe {
            function::<unsafe extern "C" fn(JniEnv, Object) -> i32>(env, slot::GET_STRING_LENGTH)(
                env, string,
            )
        };
        Some(Self {
            env,
            string,
            units,
            count,
            _scope: std::marker::PhantomData,
        })
    }
}

impl Drop for Utf16<'_> {
    fn drop(&mut self) {
        // SAFETY: releases the characters pinned in `new` on the same thread.
        unsafe {
            function::<unsafe extern "C" fn(JniEnv, Object, *const u16)>(
                self.env,
                slot::RELEASE_STRING_CHARS,
            )(self.env, self.string, self.units)
        }
    }
}

/// The surface handle Java holds: the acquired window and its session.
struct Surface {
    window: *mut c_void,
    session: SessionPointer,
}

fn session(value: i64) -> Option<SessionPointer> {
    let surface = value as usize as *const Surface;
    // SAFETY: Java only passes handles returned by `open` and not yet closed.
    (!surface.is_null()).then(|| unsafe { (*surface).session })
}

/// Stores nothing beyond what SDL needs; call from the application's `JNI_OnLoad`.
///
/// # Safety
/// Must be called by the VM's library load with its own `vm` pointer.
pub unsafe fn on_load(vm: *mut c_void, reserved: *mut c_void) -> i32 {
    // SAFETY: forwarded from the VM's JNI_OnLoad call.
    let version = unsafe { CoveSDL_JNI_OnLoad(vm.cast(), reserved) };
    if version > 0 {
        version
    } else {
        JNI_VERSION_1_6
    }
}

/// # Safety
/// Called by the JVM with a live environment and `jstring` arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_createApplication(
    env: JniEnv,
    _class: Object,
    scale: f32,
    resources: Object,
    checkpoint: Object,
    configuration: Object,
    options: Object,
) -> i64 {
    if resources.is_null() {
        return 0;
    }
    // SAFETY: JNI slot signatures from jni.h; the UTF chars are released below.
    let path = unsafe {
        function::<unsafe extern "C" fn(JniEnv, Object, *mut u8) -> *const c_char>(
            env,
            slot::GET_STRING_UTF_CHARS,
        )(env, resources, std::ptr::null_mut())
    };
    if path.is_null() {
        return 0;
    }
    // SAFETY: `path` is a NUL-terminated modified-UTF-8 string owned by the VM.
    let value = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: JNI slot signatures from jni.h; releases the chars obtained above.
    unsafe {
        function::<unsafe extern "C" fn(JniEnv, Object, *const c_char)>(
            env,
            slot::RELEASE_STRING_UTF_CHARS,
        )(env, resources, path)
    };
    // SAFETY: the activity creates the application on the UI thread before any
    // native thread reads the environment.
    unsafe { std::env::set_var("PACKAGE_RESOURCE_BUNDLE_PATH", value) };
    // SAFETY: the VM passes live `jstring` arguments for this call.
    let (Some(state), Some(launch), Some(runtime)) = (unsafe {
        (
            Utf16::new(env, checkpoint),
            Utf16::new(env, configuration),
            Utf16::new(env, options),
        )
    }) else {
        return 0;
    };
    // SAFETY: each buffer stays pinned until the guards drop after the call.
    let application = unsafe {
        bridge::cove_android_create_application(
            scale,
            state.units,
            state.count,
            launch.units,
            launch.count,
            runtime.units,
            runtime.count,
        )
    };
    application as usize as i64
}

/// # Safety
/// `value` must be a handle returned by `createApplication`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_destroyApplication(
    _env: JniEnv,
    _class: Object,
    value: i64,
) {
    // SAFETY: forwarded application handle contract.
    unsafe { bridge::cove_android_destroy_application(value as usize as ApplicationPointer) }
}

/// # Safety
/// `application` must be live and `surface` an `android.view.Surface`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_open(
    env: JniEnv,
    _class: Object,
    application: i64,
    surface: Object,
    width: i32,
    height: i32,
) -> i64 {
    // SAFETY: `surface` is a live Surface for this JNI call.
    let window = unsafe { ANativeWindow_fromSurface(env, surface) };
    if window.is_null() {
        return 0;
    }
    // SAFETY: forwarded application handle; the window reference is retained.
    let session = unsafe {
        bridge::cove_android_open(
            application as usize as ApplicationPointer,
            window,
            width,
            height,
        )
    };
    if session.is_null() {
        // SAFETY: releases the reference acquired by `ANativeWindow_fromSurface`.
        unsafe { ANativeWindow_release(window) };
        return 0;
    }
    Box::into_raw(Box::new(Surface { window, session })) as usize as i64
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_frame(
    _env: JniEnv,
    _class: Object,
    value: i64,
    timestamp: i64,
) -> i32 {
    match session(value) {
        // SAFETY: forwarded live session handle.
        Some(session) if timestamp >= 0 => unsafe {
            bridge::cove_android_frame(session, timestamp as u64)
        },
        _ => 0,
    }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_systemBarIcons(
    _env: JniEnv,
    _class: Object,
    value: i64,
) -> i32 {
    // SAFETY: forwarded live session handle.
    session(value).map_or(-1, |session| unsafe {
        bridge::cove_android_system_bar_icons(session)
    })
}

/// # Safety
/// `value` must be a handle returned by `open`, closed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_close(
    _env: JniEnv,
    _class: Object,
    value: i64,
) {
    let surface = value as usize as *mut Surface;
    if surface.is_null() {
        return;
    }
    // SAFETY: the handle came from `Box::into_raw` in `open`.
    let surface = unsafe { Box::from_raw(surface) };
    // SAFETY: the session closes before its window reference is released.
    unsafe {
        bridge::cove_android_close(surface.session);
        ANativeWindow_release(surface.window);
    }
}

/// # Safety
/// `application` must be a live application handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_memoryPressure(
    _env: JniEnv,
    _class: Object,
    application: i64,
) {
    // SAFETY: forwarded application handle contract.
    unsafe { bridge::cove_android_memory_pressure(application as usize as ApplicationPointer) }
}

/// # Safety
/// `application` must be a live application handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_pumpDiagnostics(
    _env: JniEnv,
    _class: Object,
    application: i64,
) {
    // SAFETY: forwarded application handle contract.
    unsafe { bridge::cove_android_pump_diagnostics(application as usize as ApplicationPointer) }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_pointer(
    _env: JniEnv,
    _class: Object,
    value: i64,
    kind: i32,
    x: f32,
    y: f32,
) {
    if let Some(session) = session(value) {
        // SAFETY: forwarded live session handle.
        unsafe { bridge::cove_android_pointer(session, kind, x, y) }
    }
}

/// # Safety
/// `value` must be a handle returned by `open` and `text` a `jstring`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_text(
    env: JniEnv,
    _class: Object,
    value: i64,
    text: Object,
    composing: u8,
) {
    let Some(session) = session(value) else {
        return;
    };
    // SAFETY: `text` is a live jstring for this call.
    if let Some(units) = unsafe { Utf16::new(env, text) } {
        // SAFETY: the buffer stays pinned until `units` drops.
        unsafe {
            bridge::cove_android_text(session, units.units, units.count, i32::from(composing))
        }
    }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_key(
    _env: JniEnv,
    _class: Object,
    value: i64,
    scancode: i32,
    modifiers: i32,
) {
    if let Some(session) = session(value) {
        // SAFETY: forwarded live session handle.
        unsafe { bridge::cove_android_key(session, scancode as u32, modifiers as u16) }
    }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_scroll(
    _env: JniEnv,
    _class: Object,
    value: i64,
    x: f32,
    y: f32,
    delta_x: f32,
    delta_y: f32,
) {
    if let Some(session) = session(value) {
        // SAFETY: forwarded live session handle.
        unsafe { bridge::cove_android_scroll(session, x, y, delta_x, delta_y) }
    }
}

/// # Safety
/// `value` must be a handle returned by `open` and `text_sizes` a `float[]`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_surface(
    env: JniEnv,
    _class: Object,
    value: i64,
    density: f32,
    idiom: i32,
    top: f32,
    leading: f32,
    bottom: f32,
    trailing: f32,
    keyboard: f32,
    text_sizes: Object,
) {
    let Some(session) = session(value) else {
        return;
    };
    if text_sizes.is_null() {
        return;
    }
    // SAFETY: JNI slot signatures from jni.h.
    let count = unsafe {
        function::<unsafe extern "C" fn(JniEnv, Object) -> i32>(env, slot::GET_ARRAY_LENGTH)(
            env, text_sizes,
        )
    };
    if count > 128 || count % 2 != 0 {
        return;
    }
    // SAFETY: JNI slot signatures from jni.h; released with JNI_ABORT below.
    let samples = unsafe {
        function::<unsafe extern "C" fn(JniEnv, Object, *mut u8) -> *mut f32>(
            env,
            slot::GET_FLOAT_ARRAY_ELEMENTS,
        )(env, text_sizes, std::ptr::null_mut())
    };
    if samples.is_null() {
        return;
    }
    // SAFETY: `samples` holds `count` floats until released.
    unsafe {
        bridge::cove_android_surface(
            session, density, idiom, top, leading, bottom, trailing, keyboard, samples, count,
        );
        function::<unsafe extern "C" fn(JniEnv, Object, *mut f32, i32)>(
            env,
            slot::RELEASE_FLOAT_ARRAY_ELEMENTS,
        )(env, text_sizes, samples, JNI_ABORT);
    }
}

type SnapshotCopy = unsafe extern "C" fn(SessionPointer, *mut u16, i32) -> i32;

/// # Safety
/// `env` must be the current thread's environment.
unsafe fn copy_snapshot(env: JniEnv, value: i64, copy: SnapshotCopy) -> Object {
    let Some(session) = session(value) else {
        // SAFETY: JNI slot signatures from jni.h.
        return unsafe {
            function::<unsafe extern "C" fn(JniEnv, *const c_char) -> Object>(
                env,
                slot::NEW_STRING_UTF,
            )(env, c"{}".as_ptr())
        };
    };
    // SAFETY: a null buffer queries the required length.
    let count = unsafe { copy(session, std::ptr::null_mut(), 0) };
    if count <= 0 || count > 1024 * 1024 {
        return std::ptr::null_mut();
    }
    let mut units = vec![0u16; count as usize];
    // SAFETY: `units` holds `count` elements.
    let written = unsafe { copy(session, units.as_mut_ptr(), count) };
    if written < 0 || written > count {
        return std::ptr::null_mut();
    }
    // SAFETY: JNI slot signatures from jni.h; NewString copies the units.
    unsafe {
        function::<unsafe extern "C" fn(JniEnv, *const u16, i32) -> Object>(env, slot::NEW_STRING)(
            env,
            units.as_ptr(),
            written,
        )
    }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_snapshot(
    env: JniEnv,
    _class: Object,
    value: i64,
) -> Object {
    // SAFETY: forwarded environment and session handle.
    unsafe { copy_snapshot(env, value, bridge::cove_android_snapshot) }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_inputSnapshot(
    env: JniEnv,
    _class: Object,
    value: i64,
) -> Object {
    // SAFETY: forwarded environment and session handle.
    unsafe { copy_snapshot(env, value, bridge::cove_android_input_snapshot) }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_keyState(
    _env: JniEnv,
    _class: Object,
    value: i64,
    scancode: i32,
    down: u8,
) {
    if let Some(session) = session(value) {
        // SAFETY: forwarded live session handle.
        unsafe { bridge::cove_android_key_state(session, scancode as u32, i32::from(down)) }
    }
}

/// # Safety
/// `value` must be a handle returned by `open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_MainActivity_edit(
    _env: JniEnv,
    _class: Object,
    value: i64,
    kind: i32,
    start: i32,
    end: i32,
) {
    if let Some(session) = session(value) {
        // SAFETY: forwarded live session handle.
        unsafe { bridge::cove_android_edit(session, kind, start, end) }
    }
}

/// # Safety
/// Called by the JVM with live `jstring` arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_RuntimeLog_write(
    env: JniEnv,
    _class: Object,
    level: i32,
    label: Object,
    message: Object,
) {
    // JNI modified UTF-8 encodes surrogate pairs differently; keep Java's UTF-16.
    // SAFETY: both arguments are live jstrings for this call.
    let (Some(category), Some(text)) = (unsafe { Utf16::new(env, label) }, unsafe {
        Utf16::new(env, message)
    }) else {
        return;
    };
    // SAFETY: both buffers stay pinned until the guards drop.
    unsafe {
        bridge::cove_android_log_java(
            level,
            category.units,
            category.count,
            text.units,
            text.count,
        )
    }
}

/// # Safety
/// Called by the JVM.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_cove_probe_RuntimeLog_flush(_env: JniEnv, _class: Object) {
    // SAFETY: the C log flush takes no arguments and locks internally.
    unsafe { cove_log_flush() }
}
