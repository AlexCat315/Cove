//! Documents and URLs the OS opened with the application ("Open With", file
//! associations, URL schemes), kept apart from a drag onto a window.
//!
//! macOS and iOS deliver these asynchronously on the platform thread, so the
//! native side queues them and the shell drains the queue into deferred work.
//! Windows and Linux receive them as command-line arguments at launch.

/// Drains every document the platform has queued since the last call.
pub(crate) fn drain() -> Vec<String> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        apple::drain()
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        argv::drain()
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use std::ffi::{CStr, c_char};
    use std::sync::Once;

    static INSTALL: Once = Once::new();

    unsafe extern "C" {
        fn cove_shell_open_documents_install();
        fn cove_shell_open_documents_take(out: *mut *const c_char) -> bool;
        fn cove_shell_free(memory: *mut std::ffi::c_void);
    }

    pub(super) fn drain() -> Vec<String> {
        INSTALL.call_once(|| {
            // SAFETY: registers the process-wide AppleEvent / openURL handlers
            // once, on the platform thread that owns the shell.
            unsafe { cove_shell_open_documents_install() };
        });
        let mut documents = Vec::new();
        loop {
            let mut text: *const c_char = std::ptr::null();
            // SAFETY: the bridge writes one owned NUL-terminated string per call
            // and returns false once the queue is empty.
            if !unsafe { cove_shell_open_documents_take(&mut text) } || text.is_null() {
                break;
            }
            // SAFETY: the bridge transferred ownership of this NUL-terminated copy.
            let owned = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: the copy was allocated by the bridge's strdup.
            unsafe { cove_shell_free(text.cast_mut().cast()) };
            documents.push(owned);
        }
        documents
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod argv {
    use std::sync::{Mutex, Once};

    static DOCUMENTS: Mutex<Option<Vec<String>>> = Mutex::new(None);
    static INIT: Once = Once::new();

    pub(super) fn drain() -> Vec<String> {
        INIT.call_once(|| {
            // The OS passes opened files as arguments at launch; flags are not
            // documents. Captured once so later ticks do not replay them.
            let args = std::env::args()
                .skip(1)
                .filter(|arg| !arg.starts_with('-') && !arg.is_empty())
                .collect();
            *DOCUMENTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(args);
        });
        DOCUMENTS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .unwrap_or_default()
    }
}
