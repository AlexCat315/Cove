//! Windows backend. Win32 has no system eyedropper API, so this reads one pixel
//! from the screen device context ("screenshot, then sample") after the user
//! clicks. Windows 11's `Eyedropper`/`EyeDropper` APIs belong to WinUI and
//! Chromium respectively and only see their own window's content.
//!
//! The whole interaction runs on a worker thread. `GetAsyncKeyState` polling
//! and `GetPixel` must never run on the platform thread.

use super::{CompletionSink, Session, pixel::decode_colorref};
use cove_platform::ScreenColorSample;
use std::{
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const VK_LBUTTON: i32 = 0x01;
const VK_RBUTTON: i32 = 0x02;
const VK_ESCAPE: i32 = 0x1B;
const CLR_INVALID: u32 = u32::MAX;
const POLL_INTERVAL: Duration = Duration::from_millis(8);
/// A picker left open must not keep a worker thread alive forever.
const PICK_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(key: i32) -> i16;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn GetDC(window: *mut c_void) -> *mut c_void;
    fn ReleaseDC(window: *mut c_void, dc: *mut c_void) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
}

fn pressed(key: i32) -> bool {
    // SAFETY: GetAsyncKeyState takes a virtual-key code and is thread-safe.
    let state = unsafe { GetAsyncKeyState(key) };
    state < 0
}

fn read_pixel() -> ScreenColorSample {
    let mut point = Point { x: 0, y: 0 };
    // SAFETY: `point` is a valid two-int POINT receiving the physical cursor
    // position in virtual-screen coordinates.
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return ScreenColorSample::Failed("GetCursorPos failed".into());
    }
    // SAFETY: `GetDC(NULL)` covers the whole virtual screen, including negative
    // coordinates on secondary monitors; the DC is released before returning.
    let dc = unsafe { GetDC(std::ptr::null_mut()) };
    if dc.is_null() {
        return ScreenColorSample::Failed("GetDC(screen) failed".into());
    }
    // SAFETY: `dc` is a live screen DC and the cursor position is in physical
    // pixels, the same coordinate space GetPixel uses.
    let colorref = unsafe { GetPixel(dc, point.x, point.y) };
    // SAFETY: `dc` came from GetDC(NULL) above and has not been released yet.
    unsafe { ReleaseDC(std::ptr::null_mut(), dc) };
    if colorref == CLR_INVALID {
        return ScreenColorSample::Failed("the screen pixel is outside the device context".into());
    }
    ScreenColorSample::Picked(decode_colorref(colorref))
}

fn interact(cancel: &AtomicBool) -> ScreenColorSample {
    let started = Instant::now();
    let cancelled = || cancel.load(Ordering::Relaxed) || started.elapsed() >= PICK_TIMEOUT;
    // The click that opened the picker is still held down; do not sample it.
    while pressed(VK_LBUTTON) {
        if cancelled() {
            return ScreenColorSample::Cancelled;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    loop {
        if cancelled() {
            return ScreenColorSample::Cancelled;
        }
        if pressed(VK_ESCAPE) || pressed(VK_RBUTTON) {
            return ScreenColorSample::Cancelled;
        }
        if pressed(VK_LBUTTON) {
            return read_pixel();
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

pub(super) fn start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    std::thread::Builder::new()
        .name("cove-color-sampler".into())
        .spawn(move || sink.deliver(request, interact(&flag)))
        .map_err(|error| {
            ScreenColorSample::Failed(format!("the colour sampler thread failed: {error}"))
        })?;
    Ok(Session::worker(cancel))
}
