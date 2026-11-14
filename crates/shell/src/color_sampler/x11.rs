//! X11 backend: screenshot the root window with `XGetImage`, then sample the
//! pixel under the pointer. The display connection, button polling and image
//! read all live on the sampler's worker thread, never the platform thread.
//!
//! While the pick is active the default pointer is replaced by an override-
//! redirect magnifier loupe that follows it and shows the live screen pixels
//! around it — the same feedback `NSColorSampler` gives on macOS. The loupe is
//! a 1-bit shaped window (libXext), so it stays circular without a compositing
//! manager; when libXext is missing it degrades to a square window.
//!
//! `libX11.so.6` is resolved with `dlopen` at run time so a Linux build does
//! not need X11 development headers and still works on a headless/Wayland
//! installation, where this backend reports `Unavailable` and the portal is
//! tried instead.

use super::{Attempt, pixel::decode_masked_pixel};
use cove_platform::ScreenColorSample;
use libc::{c_char, c_int, c_uint, c_ulong, c_ushort, c_void};
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

const INPUT_OUTPUT: c_uint = 1;
const CW_BACK_PIXEL: c_ulong = 1 << 1;
const CW_OVERRIDE_REDIRECT: c_ulong = 1 << 9;
const SHAPE_BOUNDING: c_int = 0;
const SHAPE_SET: c_int = 0;
const LSB_FIRST: c_int = 0;
const LOUPE_CELLS: c_int = 11;
const LOUPE_MARGIN: c_int = 2;

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

/// `XSetWindowAttributes` is only ever used with `override_redirect` and
/// `background_pixel`, but the full layout is needed so Xlib reads the right
/// offsets.
#[repr(C)]
struct SetWindowAttributes {
    background_pixmap: c_ulong,
    background_pixel: c_ulong,
    border_pixmap: c_ulong,
    border_pixel: c_ulong,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: c_int,
    event_mask: c_ulong,
    do_not_propagate_mask: c_ulong,
    override_redirect: c_int,
    colormap: c_ulong,
    cursor: c_ulong,
}

#[repr(C)]
struct XColor {
    pixel: c_ulong,
    red: c_ushort,
    green: c_ushort,
    blue: c_ushort,
    flags: c_char,
    pad: c_char,
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
type DefaultScreen = unsafe extern "C" fn(*mut c_void) -> c_int;
type CreateWindow = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    c_int,
    c_int,
    c_uint,
    c_uint,
    c_uint,
    c_int,
    c_uint,
    *mut c_void,
    c_ulong,
    *mut SetWindowAttributes,
) -> c_ulong;
type DestroyWindow = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type MapRaised = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type MoveWindow = unsafe extern "C" fn(*mut c_void, c_ulong, c_int, c_int) -> c_int;
type RaiseWindow = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type CreateGC = unsafe extern "C" fn(*mut c_void, c_ulong, c_ulong, *mut c_void) -> *mut c_void;
type FreeGC = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type PutImage = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    *mut c_void,
    *mut XImagePrefix,
    c_int,
    c_int,
    c_int,
    c_int,
    c_uint,
    c_uint,
) -> c_int;
type CreateBitmapFromData =
    unsafe extern "C" fn(*mut c_void, c_ulong, *const c_char, c_uint, c_uint) -> c_ulong;
type CreatePixmapCursor = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    c_ulong,
    *mut XColor,
    *mut XColor,
    c_uint,
    c_uint,
) -> c_ulong;
type DefineCursor = unsafe extern "C" fn(*mut c_void, c_ulong, c_ulong) -> c_int;
type UndefineCursor = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type FreeCursor = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type FreePixmap = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type CreatePixmap = unsafe extern "C" fn(*mut c_void, c_ulong, c_uint, c_uint, c_uint) -> c_ulong;
type FillArc = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    *mut c_void,
    c_int,
    c_int,
    c_uint,
    c_uint,
    c_int,
    c_int,
) -> c_int;
type SetForeground = unsafe extern "C" fn(*mut c_void, *mut c_void, c_ulong) -> c_int;
type Flush = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type DisplayWidth = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type DisplayHeight = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type DisplayWidthMM = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type ShapeCombineMask =
    unsafe extern "C" fn(*mut c_void, c_ulong, c_int, c_int, c_int, c_ulong, c_int) -> c_int;

struct Xlib {
    open: OpenDisplay,
    close: CloseDisplay,
    root: DefaultRootWindow,
    screen: DefaultScreen,
    query_pointer: QueryPointer,
    get_image: GetImage,
    get_pixel: GetPixel,
    destroy_image: DestroyImage,
    keysym_to_keycode: KeysymToKeycode,
    query_keymap: QueryKeymap,
    create_window: CreateWindow,
    destroy_window: DestroyWindow,
    map_raised: MapRaised,
    move_window: MoveWindow,
    raise_window: RaiseWindow,
    create_gc: CreateGC,
    free_gc: FreeGC,
    put_image: PutImage,
    create_bitmap_from_data: CreateBitmapFromData,
    create_pixmap_cursor: CreatePixmapCursor,
    define_cursor: DefineCursor,
    undefine_cursor: UndefineCursor,
    free_cursor: FreeCursor,
    free_pixmap: FreePixmap,
    create_pixmap: CreatePixmap,
    fill_arc: FillArc,
    set_foreground: SetForeground,
    flush: Flush,
    display_width: DisplayWidth,
    display_height: DisplayHeight,
    display_width_mm: DisplayWidthMM,
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
        screen: symbol(handle, c"XDefaultScreen")?,
        query_pointer: symbol(handle, c"XQueryPointer")?,
        get_image: symbol(handle, c"XGetImage")?,
        get_pixel: symbol(handle, c"XGetPixel")?,
        destroy_image: symbol(handle, c"XDestroyImage")?,
        keysym_to_keycode: symbol(handle, c"XKeysymToKeycode")?,
        query_keymap: symbol(handle, c"XQueryKeymap")?,
        create_window: symbol(handle, c"XCreateWindow")?,
        destroy_window: symbol(handle, c"XDestroyWindow")?,
        map_raised: symbol(handle, c"XMapRaised")?,
        move_window: symbol(handle, c"XMoveWindow")?,
        raise_window: symbol(handle, c"XRaiseWindow")?,
        create_gc: symbol(handle, c"XCreateGC")?,
        free_gc: symbol(handle, c"XFreeGC")?,
        put_image: symbol(handle, c"XPutImage")?,
        create_bitmap_from_data: symbol(handle, c"XCreateBitmapFromData")?,
        create_pixmap_cursor: symbol(handle, c"XCreatePixmapCursor")?,
        define_cursor: symbol(handle, c"XDefineCursor")?,
        undefine_cursor: symbol(handle, c"XUndefineCursor")?,
        free_cursor: symbol(handle, c"XFreeCursor")?,
        free_pixmap: symbol(handle, c"XFreePixmap")?,
        create_pixmap: symbol(handle, c"XCreatePixmap")?,
        fill_arc: symbol(handle, c"XFillArc")?,
        set_foreground: symbol(handle, c"XSetForeground")?,
        flush: symbol(handle, c"XFlush")?,
        display_width: symbol(handle, c"XDisplayWidth")?,
        display_height: symbol(handle, c"XDisplayHeight")?,
        display_width_mm: symbol(handle, c"XDisplayWidthMM")?,
    })
}

/// libXext is optional: it only provides the circular window shape, and the
/// loupe falls back to a square window when it is missing.
struct Xext {
    combine_mask: ShapeCombineMask,
}
// SAFETY: as for `Xlib`, the loaded function pointer is immutable.
unsafe impl Send for Xext {}
// SAFETY: as above.
unsafe impl Sync for Xext {}

fn shape_library() -> Option<&'static Xext> {
    static LIBRARY: OnceLock<Option<Xext>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| {
            // SAFETY: a static NUL-terminated library name; the handle stays
            // open for the process lifetime.
            let handle = unsafe { dlopen(c"libXext.so.6".as_ptr(), RTLD_LAZY) };
            if handle.is_null() {
                return None;
            }
            Some(Xext {
                combine_mask: symbol(handle, c"XShapeCombineMask")?,
            })
        })
        .as_ref()
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

// `c_ulong` is 32-bit on 32-bit Linux, so widening this value is not a no-op
// everywhere.
#[allow(clippy::unnecessary_cast)]
fn channel(mask: c_ulong, value: u8) -> c_ulong {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = mask >> shift;
    ((c_ulong::from(value) * max + 127) / 255) << shift
}

fn pack(masks: [c_ulong; 3], red: u8, green: u8, blue: u8) -> c_ulong {
    channel(masks[0], red) | channel(masks[1], green) | channel(masks[2], blue)
}

/// Writes one already-packed pixel into an XImage's server-format rows.
fn write_pixel(image: *mut XImagePrefix, x: c_int, y: c_int, value: c_ulong) {
    // SAFETY: `image` is live, `x`/`y` are inside its bounds, and the Rust
    // writes stay within the row the server allocated for that pixel.
    unsafe {
        let image = &*image;
        let row = image.data.add(y as usize * image.bytes_per_line as usize);
        match image.bits_per_pixel / 8 {
            4 => *(row.add(x as usize * 4) as *mut u32) = value as u32,
            3 => {
                let pixel = value as u32;
                let bytes = [pixel as u8, (pixel >> 8) as u8, (pixel >> 16) as u8];
                let bytes = if image.byte_order == LSB_FIRST {
                    bytes
                } else {
                    [bytes[2], bytes[1], bytes[0]]
                };
                *row.add(x as usize * 3) = bytes[0] as c_char;
                *row.add(x as usize * 3 + 1) = bytes[1] as c_char;
                *row.add(x as usize * 3 + 2) = bytes[2] as c_char;
            }
            _ => {}
        }
    }
}

/// Magnifier loupe that follows the pointer while a pick is active. It hides
/// the pointer by defining a fully transparent cursor on the root window and
/// restores it on drop.
struct Loupe {
    x: &'static Xlib,
    display: *mut c_void,
    root: c_ulong,
    window: c_ulong,
    gc: *mut c_void,
    frame: *mut XImagePrefix,
    cursor: c_ulong,
    cursor_bitmap: c_ulong,
    diameter: c_int,
    cell: c_int,
    ring: c_int,
    scale: f32,
    screen_width: c_int,
    screen_height: c_int,
    last: Option<(c_int, c_int)>,
    shown: bool,
}

impl Loupe {
    fn new(x: &'static Xlib, display: *mut c_void, root: c_ulong) -> Option<Self> {
        // SAFETY: `display` is a live connection; the screen metrics are plain
        // reads used to derive pixel sizes.
        let screen = unsafe { (x.screen)(display) };
        let screen_width = unsafe { (x.display_width)(display, screen) };
        let screen_height = unsafe { (x.display_height)(display, screen) };
        let width_mm = unsafe { (x.display_width_mm)(display, screen) };
        let dpi: f32 = if width_mm > 0 {
            screen_width as f32 / (width_mm as f32 / 25.4)
        } else {
            96.0
        };
        let scale: f32 = (dpi / 96.0).clamp(1.0, 2.0);
        let cell: c_int = (9.0 * scale).round() as c_int;
        let ring: c_int = (2.0 * scale).round().max(2.0) as c_int;
        let diameter = LOUPE_CELLS * cell + 2 * (ring + LOUPE_MARGIN);
        if diameter > screen_width || diameter > screen_height {
            return None;
        }
        // The loupe window is opaque (X11 has no per-pixel alpha without
        // XRender), so the frame is a screen-format image reused every update.
        // SAFETY: the request fits the screen, and Xlib owns the returned
        // image until it is destroyed in `Drop`.
        let frame = unsafe {
            (x.get_image)(
                display,
                root,
                0,
                0,
                diameter as c_uint,
                diameter as c_uint,
                ALL_PLANES,
                ZPIXMAP,
            )
        };
        if frame.is_null() {
            return None;
        }
        // SAFETY: `frame` is a live XImage prefix; 24 and 32 bpp cover the
        // TrueColor depths X servers expose in practice.
        if !matches!(unsafe { (*frame).bits_per_pixel }, 24 | 32) {
            unsafe { (x.destroy_image)(frame) };
            return None;
        }
        // SAFETY: a plain aggregate of integers, all zeroed then assigned.
        let mut attributes: SetWindowAttributes = unsafe { std::mem::zeroed() };
        attributes.background_pixel = 0;
        attributes.override_redirect = 1;
        // SAFETY: the parent root window belongs to `display`, the size fits
        // the screen, and the attributes pointer stays valid for the call.
        let window = unsafe {
            (x.create_window)(
                display,
                root,
                0,
                0,
                diameter as c_uint,
                diameter as c_uint,
                0,
                0,
                INPUT_OUTPUT,
                std::ptr::null_mut(),
                CW_OVERRIDE_REDIRECT | CW_BACK_PIXEL,
                &mut attributes,
            )
        };
        if window == 0 {
            unsafe { (x.destroy_image)(frame) };
            return None;
        }
        // SAFETY: `window` was just created on `display`.
        let gc = unsafe { (x.create_gc)(display, window, 0, std::ptr::null_mut()) };
        if gc.is_null() {
            unsafe {
                (x.destroy_window)(display, window);
                (x.destroy_image)(frame);
            }
            return None;
        }
        // A 1-bit bounding shape keeps the loupe circular on plain X11; the
        // square fallback still reads because the frame paints its corners
        // dark.
        if let Some(ext) = shape_library() {
            // SAFETY: all drawables belong to `display`; the mask is released
            // after being combined into the window.
            unsafe {
                let mask =
                    (x.create_pixmap)(display, window, diameter as c_uint, diameter as c_uint, 1);
                if mask != 0 {
                    let mask_gc = (x.create_gc)(display, mask, 0, std::ptr::null_mut());
                    if !mask_gc.is_null() {
                        (x.set_foreground)(display, mask_gc, 1);
                        (x.fill_arc)(
                            display,
                            mask,
                            mask_gc,
                            0,
                            0,
                            diameter as c_uint,
                            diameter as c_uint,
                            0,
                            360 * 64,
                        );
                        (ext.combine_mask)(display, window, SHAPE_BOUNDING, 0, 0, mask, SHAPE_SET);
                        (x.free_gc)(display, mask_gc);
                    }
                    (x.free_pixmap)(display, mask);
                }
            }
        }
        // A transparent 1x1 cursor hides the pointer for the whole pick.
        // SAFETY: the bitmap data lives for the call; the returned pixmap and
        // cursor are freed in `Drop`.
        let hidden = [0u8; 1];
        let cursor_bitmap = unsafe {
            (x.create_bitmap_from_data)(display, root, hidden.as_ptr() as *const c_char, 1, 1)
        };
        let mut color = XColor {
            pixel: 0,
            red: 0,
            green: 0,
            blue: 0,
            flags: 0,
            pad: 0,
        };
        let cursor = if cursor_bitmap == 0 {
            0
        } else {
            // SAFETY: source and mask are the same all-zero 1x1 bitmap, which
            // produces a fully transparent cursor.
            unsafe {
                (x.create_pixmap_cursor)(
                    display,
                    cursor_bitmap,
                    cursor_bitmap,
                    &mut color,
                    &mut color,
                    0,
                    0,
                )
            }
        };
        if cursor != 0 {
            // SAFETY: `root` and `cursor` belong to `display`.
            unsafe { (x.define_cursor)(display, root, cursor) };
        }
        Some(Self {
            x,
            display,
            root,
            window,
            gc,
            frame,
            cursor,
            cursor_bitmap,
            diameter,
            cell,
            ring,
            scale,
            screen_width,
            screen_height,
            last: None,
            shown: false,
        })
    }

    /// Repaints the loupe from the screen pixels around the pointer and moves
    /// it next to it.
    fn follow(&mut self, px: c_int, py: c_int) {
        if self.last == Some((px, py)) {
            return;
        }
        self.last = Some((px, py));
        let cells = LOUPE_CELLS;
        let half = cells / 2;
        // SAFETY: the request is a small region around the pointer; XGetImage
        // returns null (checked below) when it leaves the screen.
        let source = unsafe {
            (self.x.get_image)(
                self.display,
                self.root,
                px - half,
                py - half,
                cells as c_uint,
                cells as c_uint,
                ALL_PLANES,
                ZPIXMAP,
            )
        };
        if source.is_null() {
            return;
        }
        // SAFETY: `source` is a live 11x11 image; all reads stay in bounds and
        // the masks describe the server pixel format.
        let (colors, masks) = unsafe {
            let mut colors = [0 as c_ulong; (LOUPE_CELLS * LOUPE_CELLS) as usize];
            for row in 0..cells {
                for column in 0..cells {
                    colors[(row * cells + column) as usize] =
                        (self.x.get_pixel)(source, column, row);
                }
            }
            let masks = [
                (*source).red_mask,
                (*source).green_mask,
                (*source).blue_mask,
            ];
            (colors, masks)
        };
        // SAFETY: `source` came from XGetImage above and is used no further.
        unsafe { (self.x.destroy_image)(source) };

        let white = pack(masks, 255, 255, 255);
        let dark = pack(masks, 24, 26, 32);
        let diameter = self.diameter;
        let center = diameter as f32 / 2.0;
        let radius = center - 1.0;
        let inner = radius - self.ring as f32;
        let cell = self.cell as f32;
        let half_cell = cell / 2.0;
        let ring = self.ring as f32;
        for y in 0..diameter {
            for x in 0..diameter {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                let distance = (dx * dx + dy * dy).sqrt();
                let value = if distance > radius {
                    // Square fallback corners; the shape mask hides these when
                    // libXext is present.
                    dark
                } else if distance > inner {
                    if distance > radius - 0.75 {
                        dark
                    } else {
                        white
                    }
                } else {
                    let column = ((dx / cell).round() as c_int + half).clamp(0, cells - 1);
                    let row = ((dy / cell).round() as c_int + half).clamp(0, cells - 1);
                    let border = half_cell + ring;
                    let outline = border + 1.0;
                    let inside = |limit: f32| dx.abs() <= limit && dy.abs() <= limit;
                    if inside(outline) && !inside(half_cell) {
                        if inside(border) { white } else { dark }
                    } else {
                        colors[(row * cells + column) as usize]
                    }
                };
                write_pixel(self.frame, x, y, value);
            }
        }
        // SAFETY: `frame` holds a live composed image of exactly this size.
        unsafe {
            (self.x.put_image)(
                self.display,
                self.window,
                self.gc,
                self.frame,
                0,
                0,
                0,
                0,
                diameter as c_uint,
                diameter as c_uint,
            )
        };

        let offset = (18.0 * self.scale).round() as c_int;
        let mut x = px + offset;
        let mut y = py + offset;
        if x + diameter > self.screen_width {
            x = px - offset - diameter;
        }
        if y + diameter > self.screen_height {
            y = py - offset - diameter;
        }
        // SAFETY: the window is live and the coordinates keep the loupe on
        // screen; raising keeps it above managed windows.
        unsafe {
            (self.x.move_window)(self.display, self.window, x, y);
            if self.shown {
                (self.x.raise_window)(self.display, self.window);
            } else {
                (self.x.map_raised)(self.display, self.window);
                self.shown = true;
            }
            (self.x.flush)(self.display, 0);
        }
    }
}

impl Drop for Loupe {
    fn drop(&mut self) {
        // SAFETY: every handle was created by `new` on this display and is
        // released exactly once; the display outlives the loupe because
        // `interact` returns before its caller closes it.
        unsafe {
            if self.cursor != 0 {
                (self.x.undefine_cursor)(self.display, self.root);
                (self.x.free_cursor)(self.display, self.cursor);
            }
            if self.cursor_bitmap != 0 {
                (self.x.free_pixmap)(self.display, self.cursor_bitmap);
            }
            (self.x.free_gc)(self.display, self.gc);
            (self.x.destroy_window)(self.display, self.window);
            (self.x.destroy_image)(self.frame);
            (self.x.flush)(self.display, 0);
        }
    }
}

fn interact(x: &'static Xlib, display: *mut c_void, cancel: &AtomicBool) -> Attempt {
    // SAFETY: `display` is a live connection created by XOpenDisplay.
    let root = unsafe { (x.root)(display) };
    // SAFETY: XK_Escape is a standard KeySym; the return is a KeyCode or zero.
    let escape = unsafe { (x.keysym_to_keycode)(display, XK_ESCAPE) };
    let started = Instant::now();
    let cancelled = || cancel.load(Ordering::Relaxed) || started.elapsed() >= PICK_TIMEOUT;
    let mut released = false;
    let mut loupe = Loupe::new(x, display, root);
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
        if let Some(loupe) = loupe.as_mut() {
            loupe.follow(px, py);
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
