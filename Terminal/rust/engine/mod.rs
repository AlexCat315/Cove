//! The terminal emulator: libghostty-vt behind an owned, thread-confined handle.
//!
//! Ghostty types never cross this module boundary; callers see `TerminalFrame`
//! snapshots and byte arrays. libghostty callbacks run synchronously inside
//! `feed`, so they reach the engine through `ACTIVE_ENGINE`, which `feed`
//! publishes for the duration of the call.

use crate::engine::ffi::GhosttyTerminal;
use crate::engine::frame_reader::{FrameScratch, RenderState};
use crate::engine::key_encoder::KeyEncoder;
use crate::engine::mouse::MouseEncoder;
use crate::engine::selection::SelectionGesture;
use std::{cell::Cell, ptr, rc::Rc};

pub mod ffi;
pub mod frame;
mod frame_reader;
mod hyperlink;
mod images;
mod key_encoder;
mod mouse;
mod placeholder_diacritics;
mod placeholders;
mod selection;

pub use frame::{CellWidth, CursorShape, TerminalCell, TerminalCellAttributes, TerminalCursor, TerminalFrame, TerminalRGB};
pub use frame::TerminalScrollPosition;
pub use images::TerminalImagePlacement;
pub use key_encoder::{KeyModifiers, TerminalKey};
pub use mouse::{MouseAction, MouseButton, TerminalMouseEvent};
pub use placeholders::{PlaceholderCell, PlaceholderRun, PLACEHOLDER_CODEPOINT};
pub use selection::{SelectionPhase, TerminalPointer};

thread_local! {
    /// The engine libghostty callbacks should reach, published for the duration
    /// of a call that can invoke one.
    ///
    /// Callbacks run synchronously inside `feed`, and the engine is owned by a
    /// `RefCell` whose address changes until it is installed, so a pointer stored
    /// at construction would go stale. Publishing per call keeps the callback
    /// exact without constraining how the engine is owned.
    static ACTIVE_ENGINE: std::cell::Cell<*const TerminalEngine> =
        const { std::cell::Cell::new(std::ptr::null()) };
}

/// Publishes `engine` as the callback target until the guard drops.
struct ActiveEngineGuard(*const TerminalEngine);

impl ActiveEngineGuard {
    fn new(engine: *const TerminalEngine) -> Self {
        let previous = ACTIVE_ENGINE.with(|slot| slot.replace(engine));
        Self(previous)
    }
}

impl Drop for ActiveEngineGuard {
    fn drop(&mut self) {
        ACTIVE_ENGINE.with(|slot| slot.set(self.0));
    }
}

/// The terminal's own answers to queries (DSR, DA, mode reports).
pub type ReplyHandler = Rc<dyn Fn(&[u8])>;
pub type BellHandler = Rc<dyn Fn()>;
pub type TitleHandler = Rc<dyn Fn(&str)>;

/// A terminal emulator bound to no process.
///
/// Confined to one thread (the UI thread in production): every method takes
/// `&mut self` where it mutates terminal state, and the Ghostty handles it owns
/// are neither `Send` nor `Sync`.
pub struct TerminalEngine {
    pub(crate) terminal: GhosttyTerminal,
    pub(crate) render: RenderState,
    pub(crate) scratch: FrameScratch,
    pub(crate) keys: KeyEncoder,
    pub(crate) mouse: MouseEncoder,
    pub(crate) selection: SelectionGesture,
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    cell: (usize, usize),
    on_reply: Cell<Option<ReplyHandler>>,
    on_bell: Cell<Option<BellHandler>>,
    on_title_change: Cell<Option<TitleHandler>>,
}

impl TerminalEngine {
    pub fn new(columns: usize, rows: usize, scrollback_lines: usize) -> Self {
        let columns = clamp_dimension(columns);
        let rows = clamp_dimension(rows);
        // SAFETY: every allocation is owned by the returned value; the callbacks
        // installed below are `unsafe extern "C"` shims that only touch the
        // engine through the userdata pointer stored here.
        unsafe {
            let mut terminal: GhosttyTerminal = ptr::null_mut();
            if ffi::ghostty_terminal_new(ptr::null(), &mut terminal, columns as u16, rows as u16) != ffi::GHOSTTY_SUCCESS
            {
                panic!("libghostty-vt: terminal allocation failed");
            }
            let engine = Self {
                terminal,
                render: RenderState::new(),
                scratch: FrameScratch::default(),
                keys: KeyEncoder::new(),
                mouse: MouseEncoder::new(),
                selection: SelectionGesture::new(),
                columns,
                rows,
                cell: (8, 16),
                on_reply: Cell::new(None),
                on_bell: Cell::new(None),
                on_title_change: Cell::new(None),
            };
            engine.install_callbacks();
            engine.enable_images();
            engine.set_scrollback_lines(scrollback_lines);
            engine.set_default_cursor_blink(true);
            // Kitty placement geometry is measured in the pixel cell size the
            // library was told, and a cell size of zero suppresses every
            // placement, so establish the default before any output arrives.
            // This goes straight to the library: `resize` treats the unchanged
            // dimensions as a no-op and would leave the size unset.
            ffi::ghostty_terminal_resize(
                engine.terminal,
                columns as u16,
                rows as u16,
                engine.cell.0 as u32,
                engine.cell.1 as u32,
            );
            engine
        }
    }

    // MARK: Input

    /// Feed program output.
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // Callbacks run synchronously inside this call and need to reach us.
        let _active = ActiveEngineGuard::new(self as *const Self);
        // SAFETY: the terminal is live and `bytes` is readable for `bytes.len()`.
        unsafe { ffi::ghostty_terminal_vt_write(self.terminal, bytes.as_ptr(), bytes.len()) };
    }

    pub fn resize(&mut self, columns: usize, rows: usize, cell_width: usize, cell_height: usize) {
        let columns = clamp_dimension(columns);
        let rows = clamp_dimension(rows);
        let cell = (cell_width.max(1), cell_height.max(1));
        if columns == self.columns && rows == self.rows && cell == self.cell {
            return;
        }
        self.columns = columns;
        self.rows = rows;
        self.cell = cell;
        // SAFETY: the terminal is live; the dimensions are clamped to u16.
        unsafe {
            ffi::ghostty_terminal_resize(
                self.terminal,
                columns as u16,
                rows as u16,
                cell.0 as u32,
                cell.1 as u32,
            );
        }
    }

    pub fn encode_key(&mut self, key: &TerminalKey) -> Vec<u8> {
        self.keys.encode(key, self.terminal)
    }

    /// Paste payload framed for the current bracketed-paste mode, with control
    /// bytes that could escape the paste neutralised.
    pub fn encode_paste(&mut self, text: &str) -> Vec<u8> {
        if text.is_empty() {
            return Vec::new();
        }
        let bracketed = self.is_mode_set(2004, false);
        let mut data = text.as_bytes().to_vec();
        // SAFETY: `data` is a uniquely owned buffer the library rewrites in
        // place; the output buffer is sized from the reported requirement.
        unsafe {
            let mut needed = 0usize;
            ffi::ghostty_paste_encode(data.as_mut_ptr(), data.len(), bracketed, ptr::null_mut(), 0, &mut needed);
            if needed == 0 {
                return Vec::new();
            }
            let mut output = vec![0u8; needed];
            let mut written = 0usize;
            if ffi::ghostty_paste_encode(
                data.as_mut_ptr(),
                data.len(),
                bracketed,
                output.as_mut_ptr(),
                output.len(),
                &mut written,
            ) != ffi::GHOSTTY_SUCCESS
            {
                return Vec::new();
            }
            output.truncate(written);
            output
        }
    }

    // MARK: Viewport

    pub fn scroll(&mut self, rows: i32) {
        let mut behavior = ffi::GhosttyTerminalScrollViewport {
            tag: ffi::GHOSTTY_SCROLL_VIEWPORT_DELTA,
            value: ffi::GhosttyTerminalScrollViewportValue { delta: rows as i64, _padding: 0 },
        };
        // SAFETY: the terminal is live and `behavior` is the documented struct.
        unsafe { ffi::ghostty_terminal_scroll_viewport(self.terminal, behavior) };
        let _ = &mut behavior;
    }

    pub fn scroll_to_bottom(&mut self) {
        let behavior = ffi::GhosttyTerminalScrollViewport {
            tag: ffi::GHOSTTY_SCROLL_VIEWPORT_BOTTOM,
            value: ffi::GhosttyTerminalScrollViewportValue { delta: 0, _padding: 0 },
        };
        // SAFETY: as above.
        unsafe { ffi::ghostty_terminal_scroll_viewport(self.terminal, behavior) };
    }

    /// Full-screen programs (vim, less, htop) run on the alternate screen, which
    /// has no scrollback; wheel input then belongs to the program.
    pub fn is_alternate_screen(&self) -> bool {
        self.is_mode_set(1049, false) || self.is_mode_set(1047, false) || self.is_mode_set(47, false)
    }

    pub fn title(&self) -> String {
        let mut value = ffi::GhosttyString::default();
        // SAFETY: the terminal is live and `value` is the documented type; the
        // borrowed bytes are copied before the next mutating call.
        unsafe {
            if ffi::ghostty_terminal_get(self.terminal, ffi::DATA_TITLE, ptr::addr_of_mut!(value).cast())
                != ffi::GHOSTTY_SUCCESS
            {
                return String::new();
            }
            value.to_utf8().map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default()
        }
    }

    pub fn set_default_colors(&mut self, foreground: TerminalRGB, background: TerminalRGB) {
        let fg = ffi::GhosttyColorRgb { r: foreground.r, g: foreground.g, b: foreground.b };
        let bg = ffi::GhosttyColorRgb { r: background.r, g: background.g, b: background.b };
        // SAFETY: the terminal is live and both values are the documented type.
        unsafe {
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_COLOR_FOREGROUND, ptr::addr_of!(fg).cast());
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_COLOR_BACKGROUND, ptr::addr_of!(bg).cast());
        }
    }

    // MARK: Callbacks

    /// Replacement for the bytes the terminal answers on its own (DSR, DA, mode
    /// reports). The handler runs synchronously inside `feed`.
    pub fn set_reply_handler(&mut self, handler: ReplyHandler) {
        self.on_reply.set(Some(handler));
    }

    pub fn set_bell_handler(&mut self, handler: BellHandler) {
        self.on_bell.set(Some(handler));
    }

    /// Reports the new title after the program sets one.
    pub fn set_title_handler(&mut self, handler: TitleHandler) {
        self.on_title_change.set(Some(handler));
    }

    pub fn cell_size(&self) -> (usize, usize) {
        self.cell
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    // MARK: Snapshot

    pub fn snapshot(&mut self) -> TerminalFrame {
        let mut frame = self.read_frame();
        frame.scroll = self.scroll_position();
        let mut images = self.image_placements();
        images.extend(self.placeholder_placements(&frame.placeholders));
        frame.images = images;
        frame
    }

    fn scroll_position(&self) -> TerminalScrollPosition {
        let mut bar = ffi::GhosttyTerminalScrollbar { total: 0, offset: 0, len: 0 };
        // SAFETY: the terminal is live and `bar` is the documented type.
        let result = unsafe {
            ffi::ghostty_terminal_get(self.terminal, ffi::DATA_SCROLLBAR, ptr::addr_of_mut!(bar).cast())
        };
        if result != ffi::GHOSTTY_SUCCESS {
            return TerminalScrollPosition { total: self.rows, offset: 0, visible: self.rows };
        }
        TerminalScrollPosition {
            total: bar.total as usize,
            offset: bar.offset as usize,
            visible: bar.len as usize,
        }
    }

    // MARK: Private

    pub(crate) fn is_mode_set(&self, number: u16, ansi: bool) -> bool {
        let mut config = ffi::GhosttyTerminalModeConfig { mode: ffi::ghostty_mode_new(number, ansi), value: false };
        // SAFETY: the terminal is live and `config` is the documented type.
        let result = unsafe {
            ffi::ghostty_terminal_get(self.terminal, ffi::DATA_MODE, ptr::addr_of_mut!(config).cast())
        };
        result == ffi::GHOSTTY_SUCCESS && config.value
    }

    /// Kitty graphics are off until a storage limit is set. File and shared
    /// memory transmission stay disabled: only inline data is accepted.
    fn enable_images(&self) {
        install_png_decoder();
        let limit: u64 = 256 * 1024 * 1024;
        // SAFETY: the terminal is live and `limit` is a `uint64_t` as documented.
        unsafe {
            ffi::ghostty_terminal_set(
                self.terminal,
                ffi::OPT_KITTY_IMAGE_STORAGE_LIMIT,
                ptr::addr_of!(limit).cast(),
            );
        }
    }

    fn set_scrollback_lines(&self, lines: usize) {
        let mut lines = lines;
        // SAFETY: the terminal is live and `lines` is a `size_t` as documented.
        unsafe {
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_SCROLLBACK_MAX_LINES, ptr::addr_of!(lines).cast());
        }
        let _ = &mut lines;
    }

    fn set_default_cursor_blink(&self, blink: bool) {
        let mut blink = blink;
        // SAFETY: the terminal is live and `blink` is a `bool` as documented.
        unsafe {
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_DEFAULT_CURSOR_BLINK, ptr::addr_of!(blink).cast());
        }
        let _ = &mut blink;
    }

    fn install_callbacks(&self) {
        // SAFETY: every callback below reaches the engine through `ACTIVE_ENGINE`,
        // which `feed` publishes, and they are cleared before the terminal is
        // freed.
        unsafe {
            let reply: ffi::WritePtyFn = reply_callback;
            let bell: ffi::BellFn = bell_callback;
            let title: ffi::TitleChangedFn = title_callback;
            ffi::ghostty_terminal_set(
                self.terminal,
                ffi::OPT_WRITE_PTY,
                reply as *const std::ffi::c_void,
            );
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_BELL, bell as *const std::ffi::c_void);
            ffi::ghostty_terminal_set(
                self.terminal,
                ffi::OPT_TITLE_CHANGED,
                title as *const std::ffi::c_void,
            );
        }
    }

    /// The engine libghostty should report to, published by the call in progress.
    ///
    /// # Safety
    /// The returned reference is only valid for the duration of the call that
    /// published it, which is exactly the callback's own lifetime.
    unsafe fn active<'a>() -> Option<&'a Self> {
        let pointer = ACTIVE_ENGINE.with(std::cell::Cell::get);
        // SAFETY: only a live engine is published, and the guard that published
        // it outlives the callback.
        unsafe { pointer.as_ref() }
    }
}

impl Drop for TerminalEngine {
    fn drop(&mut self) {
        // SAFETY: the selection gesture borrows terminal state, so it is freed
        // first; the callbacks are cleared before the terminal goes away so no
        // late callback can reach the freed engine.
        unsafe {
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_WRITE_PTY, ptr::null());
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_BELL, ptr::null());
            ffi::ghostty_terminal_set(self.terminal, ffi::OPT_TITLE_CHANGED, ptr::null());
            self.selection.free(self.terminal);
            ffi::ghostty_terminal_free(self.terminal);
        }
    }
}

unsafe extern "C" fn reply_callback(
    _terminal: GhosttyTerminal,
    userdata: *mut std::ffi::c_void,
    data: *const u8,
    length: usize,
) {
    let _ = userdata;
    if data.is_null() || length == 0 {
        return;
    }
    // SAFETY: the feed that triggered this callback published the engine for its
    // duration, and `data` is readable for `length` bytes.
    let Some(engine) = (unsafe { TerminalEngine::active() }) else {
        return;
    };
    let bytes = unsafe { std::slice::from_raw_parts(data, length) };
    if let Some(handler) = engine.on_reply.take() {
        handler(bytes);
        engine.on_reply.set(Some(handler));
    }
}

unsafe extern "C" fn bell_callback(_terminal: GhosttyTerminal, userdata: *mut std::ffi::c_void) {
    let _ = userdata;
    // SAFETY: as in `reply_callback`.
    let Some(engine) = (unsafe { TerminalEngine::active() }) else {
        return;
    };
    if let Some(handler) = engine.on_bell.take() {
        handler();
        engine.on_bell.set(Some(handler));
    }
}

unsafe extern "C" fn title_callback(_terminal: GhosttyTerminal, userdata: *mut std::ffi::c_void) {
    let _ = userdata;
    // SAFETY: as in `reply_callback`.
    let Some(engine) = (unsafe { TerminalEngine::active() }) else {
        return;
    };
    if let Some(handler) = engine.on_title_change.take() {
        handler(&engine.title());
        engine.on_title_change.set(Some(handler));
    }
}

/// Process-wide PNG decoding for Kitty `f=100` images, backed by the shared
/// image decoder so a decoded PNG matches what the rest of the UI decodes.
fn install_png_decoder() {
    use std::sync::Once;
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // SAFETY: the callback is process-wide and installed once; it never
        // touches engine state.
        unsafe {
            let decode: ffi::DecodePngFn = decode_png_callback;
            ffi::ghostty_sys_set(ffi::SYS_OPT_DECODE_PNG, decode as *const std::ffi::c_void);
        }
    });
}

unsafe extern "C" fn decode_png_callback(
    _userdata: *mut std::ffi::c_void,
    allocator: *const ffi::GhosttyAllocator,
    data: *const u8,
    data_len: usize,
    out: *mut ffi::GhosttySysImage,
) -> bool {
    if data.is_null() || out.is_null() || data_len == 0 {
        return false;
    }
    // SAFETY: the library passes a readable PNG payload of `data_len` bytes and
    // an `out` slot it has zeroed; the pixel buffer is allocated with the
    // library's own allocator because it takes ownership on success.
    unsafe {
        let bytes = std::slice::from_raw_parts(data, data_len);
        let Ok(image) = cove_imaging::ImageDecoder::decode_data(bytes, None) else {
            return false;
        };
        let pixels = image.pixels();
        if pixels.len() != image.width() as usize * image.height() as usize * 4 {
            return false;
        }
        let buffer = ffi::ghostty_alloc(allocator, pixels.len());
        if buffer.is_null() {
            return false;
        }
        ptr::copy_nonoverlapping(pixels.as_ptr(), buffer, pixels.len());
        (*out).width = image.width();
        (*out).height = image.height();
        (*out).data = buffer;
        (*out).data_len = pixels.len();
        true
    }
}

fn clamp_dimension(value: usize) -> usize {
    value.clamp(1, u16::MAX as usize)
}
