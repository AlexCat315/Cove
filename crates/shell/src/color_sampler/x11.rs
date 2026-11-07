//! X11 backend: screenshot the root window with `XGetImage`, then sample the
//! pixel under the pointer. The display connection, button polling and image
//! read all live on the sampler's worker thread, never the platform thread.
//!
//! `libX11.so.6` is resolved with `dlopen` at run time so a Linux build does
//! not need X11 development headers and still works on a headless/Wayland
//! installation, where this backend reports `Unavailable` and the portal is
//! tried instead.

use super::{Attempt, pixel::decode_masked_pixel};
use cove_platform::ScreenColorSample;
use libc::{c_char, c_int, c_uint, c_ulong, c_void};
use std::{
    ffi::CStr,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const ZPIXMAP: c_int = 2;
const ALL_PLANES: c_ulong = c_ulong::MAX;
const BUTTON1_MASK: c_uint = 1 << 8;
const BUTTON3_MASK: c_uint = 1 << 10;
const XK_ESCAPE: c_ulong = 0xff1b;
const POLL_INTERVAL: Duration = Duration::from_millis(8);
const PICK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const RTLD_LAZY: c_int = 0x00001;

/// Prefix of `XImage` up to the colour masks. These fields are laid out by the
/// Xlib ABI and are all we need to decode a TrueColor pixel.
#[repr(C)]
struct XImagePrefix {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut c_char,
    byte_order: c_int,
    bitmap_unit: c_int,
    bitmap_bit_order: c_int,
    bitmap_pad: c_int,
    depth: c_int,
    bytes_per_line: c_int,
    bits_per_pixel: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
}

type OpenDisplay = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type CloseDisplay = unsafe extern "C" fn(*mut c_void) -> c_int;
type DefaultRootWindow = unsafe extern "C" fn(*mut c_void) -> c_ulong;
type QueryPointer = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    *mut c_ulong,
    *mut c_ulong,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_uint,
) -> c_int;
type GetImage = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    c_int,
    c_int,
    c_uint,
    c_uint,
    c_ulong,
    c_int,
) -> *mut XImagePrefix;
type GetPixel = unsafe extern "C" fn(*mut XImagePrefix, c_int, c_int) -> c_ulong;
type DestroyImage = unsafe extern "C" fn(*mut XImagePrefix) -> c_int;
type KeysymToKeycode = unsafe extern "C" fn(*mut c_void, c_ulong) -> u8;
type QueryKeymap = unsafe extern "C" fn(*mut c_void, *mut [u8; 32]) -> c_int;

struct Xlib {
    open: OpenDisplay,
    close: CloseDisplay,
    root: DefaultRootWindow,
    query_pointer: QueryPointer,
    get_image: GetImage,
    get_pixel: GetPixel,
    destroy_image: DestroyImage,
    keysym_to_keycode: KeysymToKeycode,
    query_keymap: QueryKeymap,
}
// SAFETY: `Xlib` only holds immutable function pointers from a `dlopen` handle
// that is deliberately never closed; every call uses its own `Display`, so the
// value can be shared by sampler threads without mutation.
unsafe impl Send for Xlib {}
// SAFETY: as above; the function pointers are never written after loading.
unsafe impl Sync for Xlib {}

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

fn symbol<T>(handle: *mut c_void, name: &CStr) -> Option<T> {
    // SAFETY: `name` is a static NUL-terminated symbol name and `handle` is a
    // live dlopen result; dlsym is thread-safe.
    let pointer = unsafe { dlsym(handle, name.as_ptr()) };
    if pointer.is_null() {
        return None;
    }
    // SAFETY: every `T` used here is a pointer-sized Xlib function pointer and
    // `dlsym` returned the address of exactly that symbol.
    Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&pointer) })
}

fn load() -> Option<Xlib> {
    // SAFETY: a static NUL-terminated library name; RTLD_LAZY keeps the handle
    // open for the process lifetime, which is what the cached pointers need.
    let handle = unsafe { dlopen(c"libX11.so.6".as_ptr(), RTLD_LAZY) };
    if handle.is_null() {
        return None;
    }
    Some(Xlib {
        open: symbol(handle, c"XOpenDisplay")?,
        close: symbol(handle, c"XCloseDisplay")?,
        root: symbol(handle, c"XDefaultRootWindow")?,
        query_pointer: symbol(handle, c"XQueryPointer")?,
        get_image: symbol(handle, c"XGetImage")?,
        get_pixel: symbol(handle, c"XGetPixel")?,
        destroy_image: symbol(handle, c"XDestroyImage")?,
        keysym_to_keycode: symbol(handle, c"XKeysymToKeycode")?,
        query_keymap: symbol(handle, c"XQueryKeymap")?,
    })
}

fn library() -> Option<&'static Xlib> {
    static LIBRARY: OnceLock<Option<Xlib>> = OnceLock::new();
    LIBRARY.get_or_init(load).as_ref()
}

fn pointer(x: &Xlib, display: *mut c_void, root: c_ulong) -> Option<(c_int, c_int, c_uint)> {
    let mut root_return: c_ulong = 0;
    let mut child: c_ulong = 0;
    let mut root_x: c_int = 0;
    let mut root_y: c_int = 0;
    let mut win_x: c_int = 0;
    let mut win_y: c_int = 0;
    let mut mask: c_uint = 0;
    // SAFETY: `display` and `root` are live; all nine out-parameters are valid
    // stack locations for the duration of this synchronous call.
    let ok = unsafe {
        (x.query_pointer)(
            display,
            root,
            &mut root_return,
            &mut child,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        )
    };
    (ok != 0).then_some((root_x, root_y, mask))
}

fn escape_pressed(x: &Xlib, display: *mut c_void, keycode: u8) -> bool {
    if keycode == 0 {
        return false;
    }
    let mut keys = [0u8; 32];
    // SAFETY: `keys` is a 32-byte keymap buffer as XQueryKeymap requires.
    unsafe { (x.query_keymap)(display, &mut keys) };
    keys[usize::from(keycode) / 8] & (1 << (keycode % 8)) != 0
}

// `c_ulong` is 64-bit on LP64 Linux but 32-bit elsewhere, so the widening
// casts below are only no-ops on some targets.
#[allow(clippy::unnecessary_cast)]
fn sample(
    x: &Xlib,
    display: *mut c_void,
    root: c_ulong,
    px: c_int,
    py: c_int,
) -> ScreenColorSample {
    // SAFETY: `root` is the viewable root window of `display`; a 1x1 ZPixmap
    // read returns a live XImage that is destroyed before this function ends.
    let image = unsafe { (x.get_image)(display, root, px, py, 1, 1, ALL_PLANES, ZPIXMAP) };
    if image.is_null() {
        return ScreenColorSample::Failed("XGetImage could not read the screen".into());
    }
    // SAFETY: `image` is non-null and owned here; the mask fields are part of
    // the XImage ABI prefix and the image is still alive during both reads.
    let (pixel, masks) = unsafe {
        let pixel = (x.get_pixel)(image, 0, 0);
        let masks = [(*image).red_mask, (*image).green_mask, (*image).blue_mask];
        (pixel, masks)
    };
    // SAFETY: `image` came from XGetImage above and is used no further.
    unsafe { (x.destroy_image)(image) };
    ScreenColorSample::Picked(decode_masked_pixel(
        pixel as u64,
        masks.map(|mask| mask as u64),
    ))
}

fn interact(x: &Xlib, display: *mut c_void, cancel: &AtomicBool) -> Attempt {
    // SAFETY: `display` is a live connection created by XOpenDisplay.
    let root = unsafe { (x.root)(display) };
    // SAFETY: XK_Escape is a standard KeySym; the return is a KeyCode or zero.
    let escape = unsafe { (x.keysym_to_keycode)(display, XK_ESCAPE) };
    let started = Instant::now();
    let cancelled = || cancel.load(Ordering::Relaxed) || started.elapsed() >= PICK_TIMEOUT;
    let mut released = false;
    loop {
        if cancelled() {
            return Attempt::Done(ScreenColorSample::Cancelled);
        }
        let Some((px, py, mask)) = pointer(x, display, root) else {
            return Attempt::Unavailable("XQueryPointer failed on the X11 display".into());
        };
        // Wait for the click that opened the picker to be released first.
        if !released {
            if mask & BUTTON1_MASK == 0 {
                released = true;
            } else {
                std::thread::sleep(POLL_INTERVAL);
                continue;
            }
        }
        if escape_pressed(x, display, escape) || mask & BUTTON3_MASK != 0 {
            return Attempt::Done(ScreenColorSample::Cancelled);
        }
        if mask & BUTTON1_MASK != 0 {
            return Attempt::Done(sample(x, display, root, px, py));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

pub(super) fn pick_pixel(cancel: &AtomicBool) -> Attempt {
    let Some(x) = library() else {
        return Attempt::Unavailable("libX11.so.6 is not installed".into());
    };
    // SAFETY: NULL selects $DISPLAY; the connection is closed before return.
    let display = unsafe { (x.open)(std::ptr::null()) };
    if display.is_null() {
        return Attempt::Unavailable("no X11 display is reachable".into());
    }
    let outcome = interact(x, display, cancel);
    // SAFETY: `display` is a live connection opened above and used no further.
    unsafe { (x.close)(display) };
    outcome
}
