//! JNI providers used by the statically linked androidmedia plugin.
use crate::{MediaError, MediaErrorKind, MediaResult};
use std::{
    any::Any,
    ffi::c_void,
    sync::{Arc, OnceLock},
};

struct Environment {
    java_vm: usize,
    application: usize,
    class_loader: usize,
    _owner: Arc<dyn Any + Send + Sync>,
}
static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();

/// Install once, before any GStreamer initialization in this process.
///
/// # Safety
/// `java_vm` must be the running application's JavaVM. `application` and
/// `class_loader` must be JNI global references, never local references. The
/// supplied owner must keep the VM and both references valid for its lifetime.
/// Cove retains that owner for the lifetime of the process, as GStreamer does
/// with its registry. JNI thread attachment remains GStreamer's responsibility.
pub unsafe fn install_environment(
    java_vm: *mut c_void,
    application: *mut c_void,
    class_loader: *mut c_void,
    owner: Arc<dyn Any + Send + Sync>,
) -> MediaResult<()> {
    if java_vm.is_null() || application.is_null() || class_loader.is_null() {
        return Err(MediaError::new(
            MediaErrorKind::InvalidInput,
            "Android media requires a VM, application and class loader",
        ));
    }
    // SAFETY: this process-global GStreamer query takes no arguments.
    if unsafe { gstreamer::ffi::gst_is_initialized() } != 0 {
        return Err(MediaError::new(
            MediaErrorKind::Initialization,
            "install Android media before initializing GStreamer",
        ));
    }
    ENVIRONMENT
        .set(Environment {
            java_vm: java_vm as usize,
            application: application as usize,
            class_loader: class_loader as usize,
            _owner: owner,
        })
        .map_err(|_| {
            MediaError::new(
                MediaErrorKind::Initialization,
                "Android media environment is already installed",
            )
        })
}
pub fn is_installed() -> bool {
    ENVIRONMENT.get().is_some()
}

// GStreamer's JNI helper resolves these symbols from the application module.
#[unsafe(no_mangle)]
pub extern "C" fn gst_android_get_java_vm() -> *mut c_void {
    ENVIRONMENT
        .get()
        .map_or(std::ptr::null_mut(), |env| env.java_vm as *mut c_void)
}
#[unsafe(no_mangle)]
pub extern "C" fn gst_android_get_application_context() -> *mut c_void {
    ENVIRONMENT
        .get()
        .map_or(std::ptr::null_mut(), |env| env.application as *mut c_void)
}
#[unsafe(no_mangle)]
pub extern "C" fn gst_android_get_application_class_loader() -> *mut c_void {
    ENVIRONMENT
        .get()
        .map_or(std::ptr::null_mut(), |env| env.class_loader as *mut c_void)
}
