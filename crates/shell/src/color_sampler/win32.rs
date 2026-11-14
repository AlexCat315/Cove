//! Windows backend. Win32 has no system eyedropper API, so the picker puts up
//! an invisible, fullscreen capture window with a crosshair cursor: while the
//! pick is active the window owns every click on the screen, and the click
//! that ends the pick samples its pixel straight from the screen device
//! context. Windows 11's `Eyedropper`/`EyeDropper` APIs belong to WinUI and
//! Chromium respectively and only see their own window's content.
//!
//! The whole interaction runs on a worker thread — window, message loop and
//! `GetPixel` never touch the platform thread. Dropping the `Session` sets the
//! cancel flag; a timer inside the message loop notices it and tears the
//! window down, which answers the request with `Cancelled`.

use super::{CompletionSink, Session, pixel::decode_colorref};
use cove_platform::ScreenColorSample;
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const VK_ESCAPE: i32 = 0x1B;
const CLR_INVALID: u32 = u32::MAX;
/// How often the message loop re-checks the session's cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(100);
/// A picker left open must not keep a worker thread alive forever.
const PICK_TIMEOUT: Duration = Duration::from_secs(5 * 60);

const WM_KEYDOWN: u32 = 0x0100;
const WM_TIMER: u32 = 0x0113;
const WM_DESTROY: u32 = 0x0002;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_RBUTTONDOWN: u32 = 0x0203;
const WM_QUIT: u32 = 0x0012;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const SW_SHOW: i32 = 5;
const LWA_ALPHA: u32 = 0x0000_0002;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
/// `IDC_CROSS` as `MAKEINTRESOURCE`.
const IDC_CROSS: *const u16 = 32515 as *const u16;
const CANCEL_TIMER: usize = 1;
const TIMEOUT_TIMER: usize = 2;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    window: *mut c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
}

#[repr(C)]
struct WndClassExW {
    size: u32,
    style: u32,
    wnd_proc: unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize,
    class_extra: i32,
    window_extra: i32,
    instance: *mut c_void,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu_name: *const u16,
    class_name: *const u16,
    small_icon: *mut c_void,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
    fn GetDC(window: *mut c_void) -> *mut c_void;
    fn ReleaseDC(window: *mut c_void, dc: *mut c_void) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn RegisterClassExW(class: *const WndClassExW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: *mut c_void,
        menu: *mut c_void,
        instance: *mut c_void,
        param: *mut c_void,
    ) -> *mut c_void;
    fn DestroyWindow(window: *mut c_void) -> i32;
    fn ShowWindow(window: *mut c_void, command: i32) -> i32;
    fn SetForegroundWindow(window: *mut c_void) -> i32;
    fn SetFocus(window: *mut c_void) -> *mut c_void;
    fn SetLayeredWindowAttributes(window: *mut c_void, key: u32, alpha: u8, flags: u32) -> i32;
    fn SetTimer(window: *mut c_void, id: usize, milliseconds: u32, proc: *mut c_void) -> usize;
    fn GetMessageW(message: *mut Msg, window: *mut c_void, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Msg) -> i32;
    fn DispatchMessageW(message: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn DefWindowProcW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn LoadCursorW(instance: *mut c_void, name: *const u16) -> *mut c_void;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
}

/// One live pick. The window procedure runs on the worker thread, so the
/// state rides in a thread-local instead of window bytes.
struct Capture {
    request: u64,
    sink: CompletionSink,
    cancel: Arc<AtomicBool>,
    started: Instant,
    answered: Cell<bool>,
}

thread_local! {
    static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

fn wide(string: &str) -> Vec<u16> {
    string.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Session-unique class name: a previous pick's class is still registered,
/// and re-registering the same name would fail.
static CLASS_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn read_pixel_at_cursor() -> ScreenColorSample {
    let mut point = Point { x: 0, y: 0 };
    // SAFETY: `point` is a valid two-int POINT receiving the physical cursor
    // position in virtual-screen coordinates.
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return ScreenColorSample::Failed("GetCursorPos failed".into());
    }
    // SAFETY: `GetDC(NULL)` covers the whole virtual screen, including negative
    // coordinates on secondary monitors; the DC is released before returning.
    let dc = unsafe { GetDC(ptr::null_mut()) };
    if dc.is_null() {
        return ScreenColorSample::Failed("GetDC(screen) failed".into());
    }
    // SAFETY: `dc` is a live screen DC and the cursor position is in physical
    // pixels, the same coordinate space GetPixel uses.
    let colorref = unsafe { GetPixel(dc, point.x, point.y) };
    // SAFETY: `dc` came from GetDC(NULL) above and has not been released yet.
    unsafe { ReleaseDC(ptr::null_mut(), dc) };
    if colorref == CLR_INVALID {
        return ScreenColorSample::Failed("the screen pixel is outside the device context".into());
    }
    ScreenColorSample::Picked(decode_colorref(colorref))
}

/// Answers the pending request exactly once, then marks the capture done so
/// the cleanup path cannot answer again.
fn answer(capture: &Capture, sample: ScreenColorSample) {
    if capture.answered.replace(true) {
        return;
    }
    capture.sink.deliver(capture.request, sample);
}

unsafe extern "system" fn capture_wnd_proc(
    window: *mut c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    // DestroyWindow delivers WM_DESTROY synchronously, re-entering this
    // procedure; destroying only after the borrow ends keeps that re-entry
    // from hitting an already-borrowed RefCell (which, through
    // `extern "system"`, would abort the process instead of unwinding).
    let mut destroy = false;
    CAPTURE.with_borrow_mut(|capture| {
        let Some(capture) = capture.as_ref() else {
            return;
        };
        match message {
            // The pick itself: the click that lands on the capture window
            // samples the pixel under the cursor.
            WM_LBUTTONDOWN => {
                let sample = read_pixel_at_cursor();
                answer(capture, sample);
                destroy = true;
            }
            // Right-click and Escape are the two standard ways to back out.
            WM_RBUTTONDOWN => {
                answer(capture, ScreenColorSample::Cancelled);
                destroy = true;
            }
            WM_KEYDOWN if wparam as i32 == VK_ESCAPE => {
                answer(capture, ScreenColorSample::Cancelled);
                destroy = true;
            }
            // The message loop is blocked in GetMessageW, so the session's
            // cancel flag is observed on this timer instead.
            WM_TIMER if wparam == CANCEL_TIMER => {
                if capture.cancel.load(Ordering::Relaxed)
                    || capture.started.elapsed() >= PICK_TIMEOUT
                {
                    answer(capture, ScreenColorSample::Cancelled);
                    destroy = true;
                }
            }
            WM_DESTROY => {
                // Every other exit path answers first; this catches the rest
                // (window killed externally, message-loop failure).
                answer(capture, ScreenColorSample::Cancelled);
                unsafe { PostQuitMessage(0) };
            }
            _ => {}
        }
    });
    if destroy {
        // SAFETY: `window` is a live window owned by this thread, and no
        // thread-local borrow is held while the destroy re-enters this
        // procedure with WM_DESTROY.
        unsafe { DestroyWindow(window) };
    }
    // SAFETY: the default procedure completes handling for unhandled messages
    // and the tail of handled ones, as Win32 requires.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn run_capture(request: u64, sink: CompletionSink, cancel: Arc<AtomicBool>) {
    let class_name = wide(&format!(
        "CoveColorSampler-{}",
        CLASS_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    // SAFETY: `class_name` outlives the call, the procedure has the required
    // signature, and a crosshair class cursor gives the capture window its
    // pointing feedback without extra message handling.
    let class = WndClassExW {
        size: std::mem::size_of::<WndClassExW>() as u32,
        style: 0,
        wnd_proc: capture_wnd_proc,
        class_extra: 0,
        window_extra: 0,
        instance: unsafe { GetModuleHandleW(ptr::null()) },
        icon: ptr::null_mut(),
        cursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_CROSS) },
        background: ptr::null_mut(),
        menu_name: ptr::null(),
        class_name: class_name.as_ptr(),
        small_icon: ptr::null_mut(),
    };
    // SAFETY: `class` is fully initialised for the duration of the call.
    if unsafe { RegisterClassExW(&class) } == 0 {
        sink.deliver(
            request,
            ScreenColorSample::Failed("the capture window class failed to register".into()),
        );
        return;
    }

    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    // SAFETY: the class above was just registered on this thread; the window
    // covers the whole virtual screen, is layered so it paints nothing yet
    // still receives clicks, and topmost so it sits over the app itself.
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name.as_ptr(),
            wide("").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            width,
            height,
            ptr::null_mut(),
            ptr::null_mut(),
            class.instance,
            ptr::null_mut(),
        )
    };
    if window.is_null() {
        sink.deliver(
            request,
            ScreenColorSample::Failed("the capture window failed to create".into()),
        );
        return;
    }

    // SAFETY: `window` is a live layered window. Layered-window hit testing
    // passes mouse messages through alpha-0 regions, so the window would
    // neither show its class cursor nor receive the pick click; alpha 1 is
    // visually indistinguishable from invisible but keeps the whole virtual
    // screen hit-testable.
    unsafe { SetLayeredWindowAttributes(window, 0, 1, LWA_ALPHA) };
    // SAFETY: `window` is a live window; showing and activating it puts the
    // crosshair under the cursor and lets Escape reach the message loop.
    unsafe {
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
        SetFocus(window);
        SetTimer(window, CANCEL_TIMER, CANCEL_POLL.as_millis() as u32, ptr::null_mut());
        SetTimer(
            window,
            TIMEOUT_TIMER,
            PICK_TIMEOUT.as_millis() as u32,
            ptr::null_mut(),
        );
    }

    CAPTURE.with_borrow_mut(|slot| {
        *slot = Some(Capture {
            request,
            sink,
            cancel,
            started: Instant::now(),
            answered: Cell::new(false),
        });
    });

    // SAFETY: `message` is a valid MSG receiving the thread's queue; the loop
    // ends when WM_DESTROY posts WM_QUIT.
    let mut message = Msg {
        window: ptr::null_mut(),
        message: 0,
        wparam: 0,
        lparam: 0,
        time: 0,
        point: Point { x: 0, y: 0 },
    };
    loop {
        let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
        if result <= 0 || message.message == WM_QUIT {
            break;
        }
        // SAFETY: the translated message is dispatched back into this loop's
        // window procedure.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    CAPTURE.with_borrow_mut(|slot| *slot = None);
}

pub(super) fn start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let thread_sink = sink.clone();
    std::thread::Builder::new()
        .name("cove-color-sampler".into())
        .spawn(move || run_capture(request, thread_sink, flag))
        .map_err(|error| {
            ScreenColorSample::Failed(format!("the colour sampler thread failed: {error}"))
        })?;
    Ok(Session::worker(cancel))
}
