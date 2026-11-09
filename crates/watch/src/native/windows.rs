//! `FindFirstChangeNotificationW` over the whole subtree.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

type Handle = *mut c_void;
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;
const WAIT_OBJECT_0: u32 = 0;
const FILE_NOTIFY_CHANGE_FILE_NAME: u32 = 0x1;
const FILE_NOTIFY_CHANGE_DIR_NAME: u32 = 0x2;
const FILE_NOTIFY_CHANGE_SIZE: u32 = 0x8;
const FILE_NOTIFY_CHANGE_LAST_WRITE: u32 = 0x10;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn FindFirstChangeNotificationW(path: *const u16, subtree: i32, filter: u32) -> Handle;
    fn FindNextChangeNotification(handle: Handle) -> i32;
    fn FindCloseChangeNotification(handle: Handle) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
}

/// The handle moves to the waiting thread, which alone closes it.
struct OwnedHandle(Handle);
// SAFETY: change-notification handles may be waited on from any thread.
unsafe impl Send for OwnedHandle {}

pub struct NativeWatch {
    stop: Arc<AtomicBool>,
}

impl NativeWatch {
    pub fn start(root: &Path, wake: Sender<()>, stop: Arc<AtomicBool>) -> Option<Self> {
        let wide: Vec<u16> = root
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `wide` is NUL-terminated and outlives the call.
        let handle = unsafe {
            FindFirstChangeNotificationW(
                wide.as_ptr(),
                1,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_SIZE
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
            )
        };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let owned = OwnedHandle(handle);
        let thread_stop = stop.clone();
        let spawned = std::thread::Builder::new()
            .name("cove-watch-windows".to_owned())
            .spawn(move || {
                let owned = owned;
                while !thread_stop.load(Ordering::Acquire) {
                    // SAFETY: the handle stays open until this thread closes it.
                    let result = unsafe { WaitForSingleObject(owned.0, 500) };
                    if result == WAIT_OBJECT_0 {
                        if wake.send(()).is_err() {
                            break;
                        }
                        // SAFETY: re-arms the same open handle.
                        if unsafe { FindNextChangeNotification(owned.0) } == 0 {
                            break;
                        }
                    }
                }
                // SAFETY: closed exactly once, by its only owner.
                unsafe { FindCloseChangeNotification(owned.0) };
            });
        spawned.ok()?;
        Some(Self { stop })
    }
}

impl Drop for NativeWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
