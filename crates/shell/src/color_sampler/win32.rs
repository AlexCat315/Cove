//! Windows backend. Win32 has no system eyedropper API, so the picker puts up
//! an invisible, fullscreen capture window: while the pick is active the window
//! owns every click on the screen, and the click that ends the pick samples its
//! pixel straight from the screen device context. Windows 11's
//! `Eyedropper`/`EyeDropper` APIs belong to WinUI and Chromium respectively and
//! only see their own window's content.
//!
//! The crosshair cursor is hidden and replaced by a magnifier loupe that follows
//! the pointer and shows the live screen pixels around it, the same pointing
//! feedback `NSColorSampler` gives on macOS. The loupe is its own layered,
//! click-through window painted through `UpdateLayeredWindow`; the fullscreen
//! capture window keeps receiving the moves.
//!
//! The whole interaction runs on a worker thread — window, message loop,
//! `GetPixel` and loupe painting never touch the platform thread. Dropping the
//! `Session` sets the cancel flag; a timer inside the message loop notices it
//! and tears the window down, which answers the request with `Cancelled`.

use super::{CompletionSink, Session, pixel::decode_colorref};
use cove_platform::ScreenColorSample;
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr, slice,
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
const WM_SETCURSOR: u32 = 0x0020;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_TIMER: u32 = 0x0113;
const WM_DESTROY: u32 = 0x0002;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_RBUTTONDOWN: u32 = 0x0203;
const WM_QUIT: u32 = 0x0012;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const SW_SHOW: i32 = 5;
const SW_SHOWNOACTIVATE: i32 = 4;
const LWA_ALPHA: u32 = 0x0000_0002;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
const CANCEL_TIMER: usize = 1;
const TIMEOUT_TIMER: usize = 2;

/// Screen pixels sampled around the pointer; the center cell is the pick.
const LOUPE_CELLS: i32 = 11;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;
const LOGPIXELSX: i32 = 88;
const ULW_ALPHA: u32 = 0x0000_0002;
const AC_SRC_OVER: u8 = 0;
const AC_SRC_ALPHA: u8 = 1;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Size {
    cx: i32,
    cy: i32,
}

#[repr(C)]
struct BlendFunction {
    blend_op: u8,
    blend_flags: u8,
    source_constant_alpha: u8,
    alpha_format: u8,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pixels_per_meter: i32,
    y_pixels_per_meter: i32,
    colors_used: u32,
    colors_important: u32,
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
    fn SetCursor(cursor: *mut c_void) -> *mut c_void;
    fn SetLayeredWindowAttributes(window: *mut c_void, key: u32, alpha: u8, flags: u32) -> i32;
    fn UpdateLayeredWindow(
        window: *mut c_void,
        destination_dc: *mut c_void,
        destination: *const Point,
        size: *const Size,
        source_dc: *mut c_void,
        source: *const Point,
        color_key: u32,
        blend: *const BlendFunction,
        flags: u32,
    ) -> i32;
    fn SetTimer(window: *mut c_void, id: usize, milliseconds: u32, proc: *mut c_void) -> usize;
    fn GetMessageW(message: *mut Msg, window: *mut c_void, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Msg) -> i32;
    fn DispatchMessageW(message: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn DefWindowProcW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
    fn GetDeviceCaps(dc: *mut c_void, index: i32) -> i32;
    fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
    fn DeleteDC(dc: *mut c_void) -> i32;
    fn CreateDIBSection(
        dc: *mut c_void,
        header: *const BitmapInfoHeader,
        usage: u32,
        bits: *mut *mut c_void,
        section: *mut c_void,
        offset: u32,
    ) -> *mut c_void;
    fn SelectObject(dc: *mut c_void, object: *mut c_void) -> *mut c_void;
    fn DeleteObject(object: *mut c_void) -> i32;
    fn BitBlt(
        destination: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        source: *mut c_void,
        source_x: i32,
        source_y: i32,
        operation: u32,
    ) -> i32;
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
    static LOUPE: RefCell<Option<Loupe>> = const { RefCell::new(None) };
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

/// A top-level layered surface with a 32-bit premultiplied DIB and its DC.
struct Surface {
    dc: *mut c_void,
    bitmap: *mut c_void,
    bits: *mut u32,
    width: i32,
    height: i32,
}

impl Surface {
    /// Creates a top-down 32-bit DIB selected into a memory DC.
    fn new(screen_dc: *mut c_void, width: i32, height: i32) -> Option<Self> {
        let header = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width,
            height: -height,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        };
        let mut bits: *mut c_void = ptr::null_mut();
        // SAFETY: `header` is a fully initialised 32-bit DIB header; the
        // returned section and `bits` outlive the surface and are released in
        // `Drop`.
        let bitmap = unsafe {
            CreateDIBSection(
                screen_dc,
                &header,
                DIB_RGB_COLORS,
                &mut bits,
                ptr::null_mut(),
                0,
            )
        };
        if bitmap.is_null() || bits.is_null() {
            return None;
        }
        // SAFETY: a memory DC compatible with the screen DC can select the
        // section; both are owned by this surface.
        let dc = unsafe { CreateCompatibleDC(screen_dc) };
        if dc.is_null() {
            unsafe { DeleteObject(bitmap) };
            return None;
        }
        // SAFETY: `dc` is a live memory DC and `bitmap` a live section.
        unsafe { SelectObject(dc, bitmap) };
        Some(Self {
            dc,
            bitmap,
            bits: bits.cast(),
            width,
            height,
        })
    }

    fn pixels(&self) -> &[u32] {
        // SAFETY: the section allocated `width * height` 32-bit pixels and
        // stays alive for the lifetime of the borrow.
        unsafe { slice::from_raw_parts(self.bits, (self.width * self.height) as usize) }
    }

    fn pixels_mut(&mut self) -> &mut [u32] {
        // SAFETY: as `pixels`, with exclusive access through `&mut self`.
        unsafe { slice::from_raw_parts_mut(self.bits, (self.width * self.height) as usize) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: both handles were created by `new` and are released once.
        unsafe {
            DeleteDC(self.dc);
            DeleteObject(self.bitmap);
        }
    }
}

/// Magnifier loupe that follows the pointer while a pick is active.
struct Loupe {
    window: *mut c_void,
    screen: *mut c_void,
    target: Surface,
    source: Surface,
    cell: i32,
    ring: i32,
    shadow: i32,
    scale: f32,
    virtual_origin: Point,
    virtual_extent: Size,
    shown: bool,
}

impl Loupe {
    fn new(screen: *mut c_void) -> Option<Self> {
        // SAFETY: `screen` is a live screen DC; the DPI query only reads it.
        let dpi = unsafe { GetDeviceCaps(screen, LOGPIXELSX) };
        let scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
        let cell = (9.0 * scale).round().max(6.0) as i32;
        let ring = (2.0 * scale).round().max(2.0) as i32;
        let padding = (5.0 * scale).round().max(4.0) as i32;
        let diameter = cell * LOUPE_CELLS + 2 * (ring + padding);
        let target = Surface::new(screen, diameter, diameter)?;
        let source = Surface::new(screen, LOUPE_CELLS, LOUPE_CELLS)?;

        let class_name = wide(&format!(
            "CoveColorLoupe-{}",
            CLASS_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        // SAFETY: the default procedure has the required signature, and the
        // class name outlives the registration call.
        let class = WndClassExW {
            size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            wnd_proc: DefWindowProcW,
            class_extra: 0,
            window_extra: 0,
            instance: unsafe { GetModuleHandleW(ptr::null()) },
            icon: ptr::null_mut(),
            cursor: ptr::null_mut(),
            background: ptr::null_mut(),
            menu_name: ptr::null(),
            class_name: class_name.as_ptr(),
            small_icon: ptr::null_mut(),
        };
        // SAFETY: `class` is fully initialised for the duration of the call.
        if unsafe { RegisterClassExW(&class) } == 0 {
            return None;
        }
        // SAFETY: the class was just registered on this thread. The loupe is a
        // layered, click-through overlay so the capture window keeps every
        // click and move.
        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT,
                class_name.as_ptr(),
                wide("").as_ptr(),
                WS_POPUP,
                0,
                0,
                diameter,
                diameter,
                ptr::null_mut(),
                ptr::null_mut(),
                class.instance,
                ptr::null_mut(),
            )
        };
        if window.is_null() {
            return None;
        }
        // SAFETY: the virtual-screen metrics are global reads.
        let virtual_origin = Point {
            x: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
            y: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        };
        let virtual_extent = Size {
            cx: unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
            cy: unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        };
        Some(Self {
            window,
            screen,
            target,
            source,
            cell,
            ring,
            shadow: padding,
            scale,
            virtual_origin,
            virtual_extent,
            shown: false,
        })
    }

    /// Repaints the loupe from the screen pixels around `cursor` and moves it
    /// next to the pointer.
    fn update(&mut self, cursor: Point) {
        let cells = LOUPE_CELLS;
        let half = cells / 2;
        // SAFETY: both DCs are live and the source section is `cells` square,
        // so the blit fits; `SRCCOPY` copies the screen pixels verbatim.
        let copied = unsafe {
            BitBlt(
                self.source.dc,
                0,
                0,
                cells,
                cells,
                self.screen,
                cursor.x - half,
                cursor.y - half,
                SRCCOPY,
            )
        };
        if copied == 0 {
            return;
        }
        let colors = self.source.pixels().to_vec();
        let diameter = self.target.width;
        let center = diameter as f32 / 2.0;
        let shadow = self.shadow as f32;
        let radius = center - shadow;
        let inner = radius - self.ring as f32;
        let cell = self.cell as f32;
        let half_cell = cell / 2.0;
        let ring = self.ring as f32;
        let pixels = self.target.pixels_mut();
        for y in 0..diameter {
            for x in 0..diameter {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                let distance = (dx * dx + dy * dy).sqrt();
                let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
                let index = (y * diameter + x) as usize;
                if coverage <= 0.0 {
                    // Soft drop shadow so the loupe separates from light
                    // screen content as well as dark content.
                    let falloff = (1.0 - (distance - radius) / shadow).clamp(0.0, 1.0);
                    let alpha = (0.30 * falloff * falloff * 255.0).round() as u32;
                    pixels[index] = alpha << 24;
                    continue;
                }
                let (red, green, blue, alpha) = if distance <= inner {
                    let column = ((dx / cell).round() as i32 + half).clamp(0, cells - 1);
                    let row = ((dy / cell).round() as i32 + half).clamp(0, cells - 1);
                    let color = colors[(row * cells + column) as usize];
                    // The pick cell is ringed in white on a darker outline so
                    // it reads on any screen content.
                    let half_side = half_cell;
                    let border = half_side + ring;
                    let outline = border + 1.0;
                    let inside = |limit: f32| dx.abs() <= limit && dy.abs() <= limit;
                    if inside(outline) && !inside(half_side) {
                        if inside(border) {
                            (255, 255, 255, 1.0)
                        } else {
                            (24, 26, 32, 0.4)
                        }
                    } else {
                        (
                            ((color >> 16) & 0xFF) as u8,
                            ((color >> 8) & 0xFF) as u8,
                            (color & 0xFF) as u8,
                            1.0,
                        )
                    }
                } else if distance > radius - 0.75 {
                    // A hairline edge separates the loupe from the desktop.
                    (36, 38, 44, 1.0)
                } else {
                    (250, 250, 252, 0.96)
                };
                let total = (alpha * coverage * 255.0).round() as u32;
                let premultiply = |channel: u8| (u32::from(channel) * total) / 255;
                pixels[index] = (total << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue);
            }
        }

        let offset = (18.0 * self.scale).round() as i32;
        let mut x = cursor.x + offset;
        let mut y = cursor.y + offset;
        if x + diameter > self.virtual_origin.x + self.virtual_extent.cx {
            x = cursor.x - offset - diameter;
        }
        if y + diameter > self.virtual_origin.y + self.virtual_extent.cy {
            y = cursor.y - offset - diameter;
        }
        let destination = Point { x, y };
        let source = Point { x: 0, y: 0 };
        let size = Size {
            cx: diameter,
            cy: diameter,
        };
        let blend = BlendFunction {
            blend_op: AC_SRC_OVER,
            blend_flags: 0,
            source_constant_alpha: 255,
            alpha_format: AC_SRC_ALPHA,
        };
        // SAFETY: `window` is the live layered window, `target.dc` holds the
        // composed premultiplied frame of exactly `size` pixels.
        unsafe {
            UpdateLayeredWindow(
                self.window,
                ptr::null_mut(),
                &destination,
                &size,
                self.target.dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );
        }
        if !self.shown {
            // SAFETY: showing without activating keeps focus in the capture
            // window so Escape and clicks still reach it.
            unsafe { ShowWindow(self.window, SW_SHOWNOACTIVATE) };
            self.shown = true;
        }
    }
}

impl Drop for Loupe {
    fn drop(&mut self) {
        // SAFETY: `window` is a live window created by `new`; the DCs and DIBs
        // are released by `Surface::drop`.
        unsafe { DestroyWindow(self.window) };
    }
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
    // The picker shows its own loupe, so the system pointer stays hidden. This
    // must answer before the capture borrow: WM_SETCURSOR arrives during
    // ordinary mouse movement, not only during a pick.
    if message == WM_SETCURSOR {
        // SAFETY: hiding the shared cursor is scoped to this fullscreen window;
        // it returns as soon as the pick ends and another window owns it.
        unsafe { SetCursor(ptr::null_mut()) };
        return 1;
    }
    if message == WM_MOUSEMOVE {
        let mut point = Point { x: 0, y: 0 };
        // SAFETY: `point` receives the physical cursor position.
        if unsafe { GetCursorPos(&mut point) } != 0 {
            LOUPE.with_borrow_mut(|loupe| {
                if let Some(loupe) = loupe.as_mut() {
                    loupe.update(point);
                }
            });
        }
        return 0;
    }

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
    // signature, and a null class cursor plus WM_SETCURSOR keeps the system
    // pointer hidden behind the loupe.
    let class = WndClassExW {
        size: std::mem::size_of::<WndClassExW>() as u32,
        style: 0,
        wnd_proc: capture_wnd_proc,
        class_extra: 0,
        window_extra: 0,
        instance: unsafe { GetModuleHandleW(ptr::null()) },
        icon: ptr::null_mut(),
        cursor: ptr::null_mut(),
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
    // neither receive the pick click nor the moves that drive the loupe; alpha
    // 1 is visually indistinguishable from invisible but keeps the whole
    // virtual screen hit-testable.
    let screen = unsafe { GetDC(ptr::null_mut()) };
    // SAFETY: `window` is a live layered window; showing and activating it puts
    // the loupe over the desktop and lets Escape reach the message loop.
    unsafe {
        SetLayeredWindowAttributes(window, 0, 1, LWA_ALPHA);
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
        SetFocus(window);
        SetTimer(
            window,
            CANCEL_TIMER,
            CANCEL_POLL.as_millis() as u32,
            ptr::null_mut(),
        );
        SetTimer(
            window,
            TIMEOUT_TIMER,
            PICK_TIMEOUT.as_millis() as u32,
            ptr::null_mut(),
        );
    }

    // The loupe needs a screen DC to blit from; without one the pick still
    // works, it just shows the hidden pointer with no magnifier.
    let mut loupe = if screen.is_null() {
        None
    } else {
        Loupe::new(screen)
    };
    if let Some(loupe) = loupe.as_mut() {
        let mut point = Point { x: 0, y: 0 };
        // SAFETY: `point` receives the physical cursor position.
        if unsafe { GetCursorPos(&mut point) } != 0 {
            loupe.update(point);
        }
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
    LOUPE.with_borrow_mut(|slot| *slot = loupe.take());

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

    // Dropping the loupe destroys its window; releasing the screen DC after it
    // keeps the last composited frame valid until the window is gone.
    LOUPE.with_borrow_mut(|slot| *slot = None);
    if !screen.is_null() {
        // SAFETY: `screen` came from GetDC(NULL) above and is released once.
        unsafe { ReleaseDC(ptr::null_mut(), screen) };
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
