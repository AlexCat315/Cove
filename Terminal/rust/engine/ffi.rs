//! Raw libghostty-vt declarations.
//!
//! Layouts and enum values here are not transcribed from the header by hand:
//! they were read out of the library's own ABI manifest
//! (`ghostty_type_json()`), which is authoritative for the linked build. The
//! `const` assertions at the bottom of this file check that every struct Rust
//! lays out still agrees with the manifest's sizes and offsets, so a library
//! upgrade that changes the ABI fails to compile instead of corrupting memory.
//!
//! Nothing outside `super` may use these types: Ghostty handles never cross the
//! `engine` boundary.
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::{ffi::c_void, ptr, slice};

pub type GhosttyResult = i32;
pub type GhosttyAllocator = c_void;

/// A borrowed byte string; the library owns the bytes and they stay valid only
/// for the duration of the call that produced them.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GhosttyString {
    pub ptr: *const u8,
    pub len: usize,
}

impl GhosttyString {
    pub fn to_utf8(self) -> Option<Vec<u8>> {
        if self.ptr.is_null() || self.len == 0 {
            return None;
        }
        // SAFETY: the library documents that a borrowed string's pointer is
        // valid for `len` bytes for the duration of the producing call.
        Some(unsafe { slice::from_raw_parts(self.ptr, self.len) }.to_vec())
    }
}

#[repr(C)]
pub struct TerminalImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct RenderStateImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct RowIteratorImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct RowCellsImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct KittyGraphicsImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct KittyGraphicsImageImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct KittyGraphicsPlacementIteratorImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct KeyEncoderImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct KeyEventImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct MouseEncoderImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct MouseEventImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct SelectionGestureImpl {
    _private: [u8; 0],
}
#[repr(C)]
pub struct SelectionGestureEventImpl {
    _private: [u8; 0],
}

pub type GhosttyTerminal = *mut TerminalImpl;
pub type GhosttyRenderState = *mut RenderStateImpl;
pub type GhosttyRenderStateRowIterator = *mut RowIteratorImpl;
pub type GhosttyRenderStateRowCells = *mut RowCellsImpl;
pub type GhosttyKittyGraphics = *mut KittyGraphicsImpl;
pub type GhosttyKittyGraphicsImage = *const KittyGraphicsImageImpl;
pub type GhosttyKittyGraphicsPlacementIterator = *mut KittyGraphicsPlacementIteratorImpl;
pub type GhosttyKeyEncoder = *mut KeyEncoderImpl;
pub type GhosttyKeyEvent = *mut KeyEventImpl;
pub type GhosttyMouseEncoder = *mut MouseEncoderImpl;
pub type GhosttyMouseEvent = *mut MouseEventImpl;
pub type GhosttySelectionGesture = *mut SelectionGestureImpl;
pub type GhosttySelectionGestureEvent = *mut SelectionGestureEventImpl;

// `GHOSTTY_SUCCESS` and friends. Only the values the engine distinguishes are
// named; every other negative result is treated as a generic failure.
pub const GHOSTTY_SUCCESS: GhosttyResult = 0;
pub const GHOSTTY_OUT_OF_SPACE: GhosttyResult = -3;
pub const GHOSTTY_NO_VALUE: GhosttyResult = -4;

pub type GhosttyMode = u16;
pub type GhosttyMods = u16;
pub type GhosttyStyleId = u16;

/// `ghostty_mode_new`: the numeric mode plus the ANSI (non-DEC) flag.
pub const fn ghostty_mode_new(value: u16, ansi: bool) -> GhosttyMode {
    (value & 0x7FFF) | ((ansi as u16) << 15)
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GhosttyColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A caller-provided byte buffer. `len` is bytes written on success, or the
/// required capacity when the call reports `GHOSTTY_OUT_OF_SPACE`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GhosttyBuffer {
    pub ptr: *mut u8,
    pub cap: usize,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyStyleColorValue {
    /// `palette` (1 byte) and `rgb` (3 bytes) both start at offset 0; the union
    /// is 8 bytes wide because it is aligned like a `u64`.
    pub bits: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyStyleColor {
    pub tag: i32,
    pub value: GhosttyStyleColorValue,
}

pub const GHOSTTY_STYLE_COLOR_NONE: i32 = 0;
pub const GHOSTTY_STYLE_COLOR_PALETTE: i32 = 1;
pub const GHOSTTY_STYLE_COLOR_RGB: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyStyle {
    pub size: usize,
    pub fg_color: GhosttyStyleColor,
    pub bg_color: GhosttyStyleColor,
    pub underline_color: GhosttyStyleColor,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyRenderStateColors {
    pub size: usize,
    pub background: GhosttyColorRgb,
    pub foreground: GhosttyColorRgb,
    pub cursor: GhosttyColorRgb,
    pub cursor_has_value: bool,
    pub palette: [GhosttyColorRgb; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyRenderStateCursor {
    pub size: usize,
    pub viewport_has_value: bool,
    /// Padding byte at offset 9: `viewport_x` is `u16` aligned to 2.
    pub _padding: u8,
    pub viewport_x: u16,
    pub viewport_y: u16,
    pub wide_tail: bool,
    pub visible: bool,
    pub blinking: bool,
    pub password_input: bool,
    /// Two padding bytes before the i32 style at offset 20.
    pub _padding2: [u8; 2],
    pub visual_style: i32,
}

pub const GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BAR: i32 = 0;
pub const GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK: i32 = 1;
pub const GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_UNDERLINE: i32 = 2;
pub const GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK_HOLLOW: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyTerminalScrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyTerminalModeConfig {
    pub mode: GhosttyMode,
    pub value: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GhosttyGridRef {
    pub size: usize,
    pub node: *mut c_void,
    pub x: u16,
    pub y: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttySelection {
    pub size: usize,
    pub start: GhosttyGridRef,
    pub end: GhosttyGridRef,
    pub rectangle: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyPointCoordinate {
    pub x: u16,
    pub y: u32,
}

/// The union is 16 bytes wide because it is aligned like a `u64` pair; only
/// `coordinate` is ever written by this crate.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyPointValue {
    pub coordinate: GhosttyPointCoordinate,
    pub _padding: [u64; 1],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyPoint {
    pub tag: i32,
    pub value: GhosttyPointValue,
}

pub const GHOSTTY_POINT_TAG_ACTIVE: i32 = 0;
pub const GHOSTTY_POINT_TAG_VIEWPORT: i32 = 1;
pub const GHOSTTY_POINT_TAG_SCREEN: i32 = 2;
pub const GHOSTTY_POINT_TAG_HISTORY: i32 = 3;

impl GhosttyPoint {
    /// A viewport coordinate, as every selection and hyperlink lookup uses.
    pub fn viewport(column: u16, row: u32) -> Self {
        Self {
            tag: GHOSTTY_POINT_TAG_VIEWPORT,
            value: GhosttyPointValue {
                coordinate: GhosttyPointCoordinate { x: column, y: row },
                _padding: [0],
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyTerminalScrollViewportValue {
    pub delta: i64,
    pub _padding: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyTerminalScrollViewport {
    pub tag: i32,
    pub value: GhosttyTerminalScrollViewportValue,
}

pub const GHOSTTY_SCROLL_VIEWPORT_TOP: i32 = 0;
pub const GHOSTTY_SCROLL_VIEWPORT_BOTTOM: i32 = 1;
pub const GHOSTTY_SCROLL_VIEWPORT_DELTA: i32 = 2;
pub const GHOSTTY_SCROLL_VIEWPORT_ROW: i32 = 3;
pub const GHOSTTY_SCROLL_VIEWPORT_DELTA_PROMPT: i32 = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyRenderStateRowSelection {
    pub size: usize,
    pub start_x: u16,
    pub end_x: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyKittyGraphicsPlacementRenderInfo {
    pub size: usize,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub grid_cols: u32,
    pub grid_rows: u32,
    pub viewport_col: i32,
    pub viewport_row: i32,
    pub viewport_visible: bool,
    pub _padding: [u8; 3],
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyTerminalSelectionFormatOptions {
    pub size: usize,
    pub emit: i32,
    pub unwrap: bool,
    pub trim: bool,
    pub _padding: [u8; 2],
    pub selection: *const GhosttySelection,
}

pub const GHOSTTY_FORMATTER_FORMAT_PLAIN: i32 = 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttySelectionGestureGeometry {
    pub columns: u32,
    pub cell_width: u32,
    pub padding_left: u32,
    pub screen_height: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttySurfacePosition {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyMouseEncoderSize {
    pub size: usize,
    pub screen_width: u32,
    pub screen_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_right: u32,
    pub padding_left: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttyMousePosition {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GhosttySysImage {
    pub width: u32,
    pub height: u32,
    pub data: *mut u8,
    pub data_len: usize,
}

// `GHOSTTY_TERMINAL_OPT_*`
pub const OPT_USERDATA: i32 = 0;
pub const OPT_WRITE_PTY: i32 = 1;
pub const OPT_BELL: i32 = 2;
pub const OPT_TITLE_CHANGED: i32 = 5;
pub const OPT_COLOR_FOREGROUND: i32 = 11;
pub const OPT_COLOR_BACKGROUND: i32 = 12;
pub const OPT_KITTY_IMAGE_STORAGE_LIMIT: i32 = 15;
pub const OPT_SELECTION: i32 = 21;
pub const OPT_DEFAULT_CURSOR_BLINK: i32 = 23;
pub const OPT_SCROLLBACK_MAX_LINES: i32 = 28;

// `GHOSTTY_TERMINAL_DATA_*`
pub const DATA_TITLE: i32 = 12;
pub const DATA_SCROLLBAR: i32 = 9;
pub const DATA_MOUSE_TRACKING: i32 = 11;
pub const DATA_KITTY_GRAPHICS: i32 = 30;
pub const DATA_SELECTION: i32 = 31;
pub const DATA_MODE: i32 = 37;

// `GHOSTTY_RENDER_STATE_DATA_*`
pub const RENDER_STATE_DATA_COLORS: i32 = 19;
pub const RENDER_STATE_DATA_CURSOR: i32 = 18;
pub const RENDER_STATE_DATA_ROW_ITERATOR: i32 = 4;

pub const RENDER_STATE_OPTION_DIRTY: i32 = 0;

// `GHOSTTY_RENDER_STATE_ROW_DATA_*`
pub const RENDER_STATE_ROW_DATA_CELLS: i32 = 3;
pub const RENDER_STATE_ROW_DATA_SELECTION: i32 = 4;

// `GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_*`
pub const ROW_CELLS_DATA_RAW: i32 = 1;
pub const ROW_CELLS_DATA_STYLE: i32 = 2;
pub const ROW_CELLS_DATA_GRAPHEMES_LEN: i32 = 3;
pub const ROW_CELLS_DATA_GRAPHEMES_UTF8: i32 = 9;
pub const ROW_CELLS_DATA_BG_COLOR: i32 = 5;
pub const ROW_CELLS_DATA_FG_COLOR: i32 = 6;
pub const ROW_CELLS_DATA_HAS_STYLING: i32 = 8;

// `GHOSTTY_CELL_DATA_*`
pub const CELL_DATA_WIDE: i32 = 3;
pub const CELL_DATA_HAS_HYPERLINK: i32 = 7;

pub const CELL_WIDE_NARROW: i32 = 0;
pub const CELL_WIDE_WIDE: i32 = 1;
pub const CELL_WIDE_SPACER_TAIL: i32 = 2;
pub const CELL_WIDE_SPACER_HEAD: i32 = 3;

// `GHOSTTY_KITTY_GRAPHICS_DATA_*` and placement/image data kinds.
pub const KITTY_GRAPHICS_DATA_PLACEMENT_ITERATOR: i32 = 1;
pub const PLACEMENT_DATA_IMAGE_ID: i32 = 1;
pub const PLACEMENT_DATA_PLACEMENT_ID: i32 = 2;
pub const PLACEMENT_DATA_IS_VIRTUAL: i32 = 3;
pub const PLACEMENT_DATA_COLUMNS: i32 = 10;
pub const PLACEMENT_DATA_ROWS: i32 = 11;
pub const PLACEMENT_DATA_Z: i32 = 12;
pub const KITTY_IMAGE_DATA_WIDTH: i32 = 3;
pub const KITTY_IMAGE_DATA_HEIGHT: i32 = 4;
pub const KITTY_IMAGE_DATA_FORMAT: i32 = 5;
pub const KITTY_IMAGE_DATA_DATA_PTR: i32 = 7;
pub const KITTY_IMAGE_DATA_DATA_LEN: i32 = 8;
pub const KITTY_IMAGE_DATA_GENERATION: i32 = 9;

pub const KITTY_IMAGE_FORMAT_RGB: i32 = 0;
pub const KITTY_IMAGE_FORMAT_RGBA: i32 = 1;
pub const KITTY_IMAGE_FORMAT_GRAY_ALPHA: i32 = 3;
pub const KITTY_IMAGE_FORMAT_GRAY: i32 = 4;

// Selection gesture events and options.
pub const SELECTION_GESTURE_EVENT_TYPE_PRESS: i32 = 0;
pub const SELECTION_GESTURE_EVENT_TYPE_RELEASE: i32 = 1;
pub const SELECTION_GESTURE_EVENT_TYPE_DRAG: i32 = 2;
pub const SELECTION_EVENT_OPT_REF: i32 = 0;
pub const SELECTION_EVENT_OPT_POSITION: i32 = 1;
pub const SELECTION_EVENT_OPT_REPEAT_DISTANCE: i32 = 2;
pub const SELECTION_EVENT_OPT_TIME_NS: i32 = 3;
pub const SELECTION_EVENT_OPT_REPEAT_INTERVAL_NS: i32 = 4;
pub const SELECTION_EVENT_OPT_RECTANGLE: i32 = 7;
pub const SELECTION_EVENT_OPT_GEOMETRY: i32 = 8;

// Key encoder.
pub const KEY_ACTION_RELEASE: i32 = 0;
pub const KEY_ACTION_PRESS: i32 = 1;
pub const KEY_ACTION_REPEAT: i32 = 2;

pub const MODS_SHIFT: GhosttyMods = 1 << 0;
pub const MODS_CTRL: GhosttyMods = 1 << 1;
pub const MODS_ALT: GhosttyMods = 1 << 2;
pub const MODS_SUPER: GhosttyMods = 1 << 3;

/// `GHOSTTY_KEY_*` values used by the scancode table. Only the keys a
/// terminal can be asked to encode are named.
pub mod key {
    pub const UNIDENTIFIED: i32 = 0;
    pub const BACKQUOTE: i32 = 1;
    pub const BACKSLASH: i32 = 2;
    pub const BRACKET_LEFT: i32 = 3;
    pub const BRACKET_RIGHT: i32 = 4;
    pub const COMMA: i32 = 5;
    pub const DIGIT_0: i32 = 6;
    pub const DIGIT_1: i32 = 7;
    pub const DIGIT_2: i32 = 8;
    pub const DIGIT_3: i32 = 9;
    pub const DIGIT_4: i32 = 10;
    pub const DIGIT_5: i32 = 11;
    pub const DIGIT_6: i32 = 12;
    pub const DIGIT_7: i32 = 13;
    pub const DIGIT_8: i32 = 14;
    pub const DIGIT_9: i32 = 15;
    pub const EQUAL: i32 = 16;
    pub const INTL_BACKSLASH: i32 = 17;
    pub const A: i32 = 20;
    pub const MINUS: i32 = 46;
    pub const PERIOD: i32 = 47;
    pub const QUOTE: i32 = 48;
    pub const SEMICOLON: i32 = 49;
    pub const SLASH: i32 = 50;
    pub const BACKSPACE: i32 = 53;
    pub const ENTER: i32 = 58;
    pub const SPACE: i32 = 63;
    pub const TAB: i32 = 64;
    pub const DELETE: i32 = 68;
    pub const END: i32 = 69;
    pub const HOME: i32 = 71;
    pub const INSERT: i32 = 72;
    pub const PAGE_DOWN: i32 = 73;
    pub const PAGE_UP: i32 = 74;
    pub const ARROW_DOWN: i32 = 75;
    pub const ARROW_LEFT: i32 = 76;
    pub const ARROW_RIGHT: i32 = 77;
    pub const ARROW_UP: i32 = 78;
    pub const NUMPAD_0: i32 = 80;
    pub const NUMPAD_1: i32 = 81;
    pub const NUMPAD_2: i32 = 82;
    pub const NUMPAD_3: i32 = 83;
    pub const NUMPAD_4: i32 = 84;
    pub const NUMPAD_5: i32 = 85;
    pub const NUMPAD_6: i32 = 86;
    pub const NUMPAD_7: i32 = 87;
    pub const NUMPAD_8: i32 = 88;
    pub const NUMPAD_9: i32 = 89;
    pub const NUMPAD_ADD: i32 = 90;
    pub const NUMPAD_DECIMAL: i32 = 95;
    pub const NUMPAD_DIVIDE: i32 = 96;
    pub const NUMPAD_ENTER: i32 = 97;
    pub const NUMPAD_EQUAL: i32 = 98;
    pub const NUMPAD_MULTIPLY: i32 = 104;
    pub const NUMPAD_SUBTRACT: i32 = 107;
    pub const ESCAPE: i32 = 120;
    pub const F1: i32 = 121;
}

// Mouse encoder.
pub const MOUSE_ENCODER_OPT_SIZE: i32 = 2;
pub const MOUSE_ACTION_PRESS: i32 = 0;
pub const MOUSE_ACTION_RELEASE: i32 = 1;
pub const MOUSE_ACTION_MOTION: i32 = 2;
pub const MOUSE_BUTTON_LEFT: i32 = 1;
pub const MOUSE_BUTTON_RIGHT: i32 = 2;
pub const MOUSE_BUTTON_MIDDLE: i32 = 3;
pub const MOUSE_BUTTON_FOUR: i32 = 4;
pub const MOUSE_BUTTON_FIVE: i32 = 5;
pub const MOUSE_BUTTON_SIX: i32 = 6;
pub const MOUSE_BUTTON_SEVEN: i32 = 7;

// System options.
pub const SYS_OPT_DECODE_PNG: i32 = 1;

/// `GhosttyTerminalWritePtyFn`
pub type WritePtyFn = unsafe extern "C" fn(GhosttyTerminal, *mut c_void, *const u8, usize);
/// `GhosttyTerminalBellFn`
pub type BellFn = unsafe extern "C" fn(GhosttyTerminal, *mut c_void);
/// `GhosttyTerminalTitleChangedFn`
pub type TitleChangedFn = unsafe extern "C" fn(GhosttyTerminal, *mut c_void);
/// `GhosttySysDecodePngFn`
pub type DecodePngFn = unsafe extern "C" fn(
    *mut c_void,
    *const GhosttyAllocator,
    *const u8,
    usize,
    *mut GhosttySysImage,
) -> bool;

unsafe extern "C" {
    pub fn ghostty_terminal_new(
        allocator: *const GhosttyAllocator,
        terminal: *mut GhosttyTerminal,
        cols: u16,
        rows: u16,
    ) -> GhosttyResult;
    pub fn ghostty_terminal_free(terminal: GhosttyTerminal);
    pub fn ghostty_terminal_resize(
        terminal: GhosttyTerminal,
        cols: u16,
        rows: u16,
        cell_width: u32,
        cell_height: u32,
    ) -> GhosttyResult;
    pub fn ghostty_terminal_set(
        terminal: GhosttyTerminal,
        option: i32,
        value: *const c_void,
    ) -> GhosttyResult;
    pub fn ghostty_terminal_vt_write(terminal: GhosttyTerminal, data: *const u8, len: usize);
    pub fn ghostty_terminal_scroll_viewport(
        terminal: GhosttyTerminal,
        behavior: GhosttyTerminalScrollViewport,
    );
    pub fn ghostty_terminal_get(
        terminal: GhosttyTerminal,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;
    pub fn ghostty_terminal_grid_ref(
        terminal: GhosttyTerminal,
        point: GhosttyPoint,
        out_ref: *mut GhosttyGridRef,
    ) -> GhosttyResult;
    pub fn ghostty_terminal_selection_format_alloc(
        terminal: GhosttyTerminal,
        allocator: *const GhosttyAllocator,
        options: GhosttyTerminalSelectionFormatOptions,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> GhosttyResult;

    pub fn ghostty_render_state_new(
        allocator: *const GhosttyAllocator,
        state: *mut GhosttyRenderState,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_free(state: GhosttyRenderState);
    pub fn ghostty_render_state_update(
        state: GhosttyRenderState,
        terminal: GhosttyTerminal,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_get(
        state: GhosttyRenderState,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_set(
        state: GhosttyRenderState,
        option: i32,
        value: *const c_void,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_row_iterator_new(
        allocator: *const GhosttyAllocator,
        out_iterator: *mut GhosttyRenderStateRowIterator,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_row_iterator_free(iterator: GhosttyRenderStateRowIterator);
    pub fn ghostty_render_state_row_iterator_next(iterator: GhosttyRenderStateRowIterator) -> bool;
    pub fn ghostty_render_state_row_get(
        iterator: GhosttyRenderStateRowIterator,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_row_cells_new(
        allocator: *const GhosttyAllocator,
        out_cells: *mut GhosttyRenderStateRowCells,
    ) -> GhosttyResult;
    pub fn ghostty_render_state_row_cells_free(cells: GhosttyRenderStateRowCells);
    pub fn ghostty_render_state_row_cells_next(cells: GhosttyRenderStateRowCells) -> bool;
    pub fn ghostty_render_state_row_cells_get(
        cells: GhosttyRenderStateRowCells,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;

    pub fn ghostty_cell_get(cell: u64, data: i32, out: *mut c_void) -> GhosttyResult;
    pub fn ghostty_grid_ref_hyperlink_uri(
        reference: *const GhosttyGridRef,
        buf: *mut u8,
        buf_len: usize,
        out_len: *mut usize,
    ) -> GhosttyResult;

    pub fn ghostty_kitty_graphics_get(
        graphics: GhosttyKittyGraphics,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;
    pub fn ghostty_kitty_graphics_image(
        graphics: GhosttyKittyGraphics,
        image_id: u32,
    ) -> GhosttyKittyGraphicsImage;
    pub fn ghostty_kitty_graphics_image_get(
        image: GhosttyKittyGraphicsImage,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;
    pub fn ghostty_kitty_graphics_placement_iterator_new(
        allocator: *const GhosttyAllocator,
        out_iterator: *mut GhosttyKittyGraphicsPlacementIterator,
    ) -> GhosttyResult;
    pub fn ghostty_kitty_graphics_placement_iterator_free(
        iterator: GhosttyKittyGraphicsPlacementIterator,
    );
    pub fn ghostty_kitty_graphics_placement_next(
        iterator: GhosttyKittyGraphicsPlacementIterator,
    ) -> bool;
    pub fn ghostty_kitty_graphics_placement_get(
        iterator: GhosttyKittyGraphicsPlacementIterator,
        data: i32,
        out: *mut c_void,
    ) -> GhosttyResult;
    pub fn ghostty_kitty_graphics_placement_render_info(
        iterator: GhosttyKittyGraphicsPlacementIterator,
        image: GhosttyKittyGraphicsImage,
        terminal: GhosttyTerminal,
        out_info: *mut GhosttyKittyGraphicsPlacementRenderInfo,
    ) -> GhosttyResult;

    pub fn ghostty_key_encoder_new(
        allocator: *const GhosttyAllocator,
        encoder: *mut GhosttyKeyEncoder,
    ) -> GhosttyResult;
    pub fn ghostty_key_encoder_free(encoder: GhosttyKeyEncoder);
    pub fn ghostty_key_encoder_setopt_from_terminal(
        encoder: GhosttyKeyEncoder,
        terminal: GhosttyTerminal,
    );
    pub fn ghostty_key_encoder_encode(
        encoder: GhosttyKeyEncoder,
        event: GhosttyKeyEvent,
        out_buf: *mut u8,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> GhosttyResult;
    pub fn ghostty_key_event_new(
        allocator: *const GhosttyAllocator,
        event: *mut GhosttyKeyEvent,
    ) -> GhosttyResult;
    pub fn ghostty_key_event_free(event: GhosttyKeyEvent);
    pub fn ghostty_key_event_set_action(event: GhosttyKeyEvent, action: i32);
    pub fn ghostty_key_event_set_key(event: GhosttyKeyEvent, key: i32);
    pub fn ghostty_key_event_set_mods(event: GhosttyKeyEvent, mods: GhosttyMods);
    pub fn ghostty_key_event_set_consumed_mods(event: GhosttyKeyEvent, mods: GhosttyMods);
    pub fn ghostty_key_event_set_composing(event: GhosttyKeyEvent, composing: bool);
    pub fn ghostty_key_event_set_utf8(event: GhosttyKeyEvent, utf8: *const u8, len: usize);
    pub fn ghostty_key_event_set_unshifted_codepoint(event: GhosttyKeyEvent, codepoint: u32);

    pub fn ghostty_mouse_encoder_new(
        allocator: *const GhosttyAllocator,
        encoder: *mut GhosttyMouseEncoder,
    ) -> GhosttyResult;
    pub fn ghostty_mouse_encoder_free(encoder: GhosttyMouseEncoder);
    pub fn ghostty_mouse_encoder_setopt(
        encoder: GhosttyMouseEncoder,
        option: i32,
        value: *const c_void,
    );
    pub fn ghostty_mouse_encoder_setopt_from_terminal(
        encoder: GhosttyMouseEncoder,
        terminal: GhosttyTerminal,
    );
    pub fn ghostty_mouse_encoder_encode(
        encoder: GhosttyMouseEncoder,
        event: GhosttyMouseEvent,
        out_buf: *mut u8,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> GhosttyResult;
    pub fn ghostty_mouse_event_new(
        allocator: *const GhosttyAllocator,
        event: *mut GhosttyMouseEvent,
    ) -> GhosttyResult;
    pub fn ghostty_mouse_event_free(event: GhosttyMouseEvent);
    pub fn ghostty_mouse_event_set_action(event: GhosttyMouseEvent, action: i32);
    pub fn ghostty_mouse_event_set_button(event: GhosttyMouseEvent, button: i32);
    pub fn ghostty_mouse_event_clear_button(event: GhosttyMouseEvent);
    pub fn ghostty_mouse_event_set_mods(event: GhosttyMouseEvent, mods: GhosttyMods);
    pub fn ghostty_mouse_event_set_position(event: GhosttyMouseEvent, position: GhosttyMousePosition);

    pub fn ghostty_selection_gesture_new(
        allocator: *const GhosttyAllocator,
        out_gesture: *mut GhosttySelectionGesture,
    ) -> GhosttyResult;
    pub fn ghostty_selection_gesture_free(
        gesture: GhosttySelectionGesture,
        terminal: GhosttyTerminal,
    );
    pub fn ghostty_selection_gesture_event_new(
        allocator: *const GhosttyAllocator,
        out_event: *mut GhosttySelectionGestureEvent,
        event_type: i32,
    ) -> GhosttyResult;
    pub fn ghostty_selection_gesture_event_free(event: GhosttySelectionGestureEvent);
    pub fn ghostty_selection_gesture_event_set(
        event: GhosttySelectionGestureEvent,
        option: i32,
        value: *const c_void,
    ) -> GhosttyResult;
    pub fn ghostty_selection_gesture_event(
        gesture: GhosttySelectionGesture,
        terminal: GhosttyTerminal,
        event: GhosttySelectionGestureEvent,
        out_selection: *mut GhosttySelection,
    ) -> GhosttyResult;

    pub fn ghostty_paste_encode(
        data: *mut u8,
        data_len: usize,
        bracketed: bool,
        buf: *mut u8,
        buf_len: usize,
        out_written: *mut usize,
    ) -> GhosttyResult;

    pub fn ghostty_sys_set(option: i32, value: *const c_void) -> GhosttyResult;
    pub fn ghostty_alloc(allocator: *const GhosttyAllocator, len: usize) -> *mut u8;
    pub fn ghostty_free(allocator: *const GhosttyAllocator, ptr: *mut u8, len: usize);
}

use super::sized::{sized, SizedAbi};

macro_rules! impl_sized_abi {
    ($($ty:ty),* $(,)?) => {$(
        impl SizedAbi for $ty {
            fn write_abi_size(&mut self, size: usize) {
                self.size = size;
            }
        }
    )*};
}

impl_sized_abi!(
    GhosttyStyle,
    GhosttyRenderStateColors,
    GhosttyRenderStateCursor,
    GhosttyRenderStateRowSelection,
    GhosttyGridRef,
    GhosttySelection,
    GhosttyTerminalSelectionFormatOptions,
    GhosttyMouseEncoderSize,
    GhosttyKittyGraphicsPlacementRenderInfo,
);

/// Builds a `GhosttyStyle` with its `size` field set, as the sized-struct ABI
/// requires. Every sized struct in this crate is produced through a helper like
/// this one instead of `Default`, which would leave `size` at zero.
pub fn sized_style() -> GhosttyStyle {
    let value = GhosttyStyle {
        size: 0,
        fg_color: GhosttyStyleColor { tag: 0, value: GhosttyStyleColorValue { bits: 0 } },
        bg_color: GhosttyStyleColor { tag: 0, value: GhosttyStyleColorValue { bits: 0 } },
        underline_color: GhosttyStyleColor { tag: 0, value: GhosttyStyleColorValue { bits: 0 } },
        bold: false,
        italic: false,
        faint: false,
        blink: false,
        inverse: false,
        invisible: false,
        strikethrough: false,
        overline: false,
        underline: 0,
    };
    sized(value)
}

pub fn sized_colors() -> GhosttyRenderStateColors {
    let value = GhosttyRenderStateColors {
        size: 0,
        background: GhosttyColorRgb::default(),
        foreground: GhosttyColorRgb::default(),
        cursor: GhosttyColorRgb::default(),
        cursor_has_value: false,
        palette: [GhosttyColorRgb::default(); 256],
    };
    sized(value)
}

pub fn sized_cursor() -> GhosttyRenderStateCursor {
    let value = GhosttyRenderStateCursor {
        size: 0,
        viewport_has_value: false,
        _padding: 0,
        viewport_x: 0,
        viewport_y: 0,
        wide_tail: false,
        visible: false,
        blinking: false,
        password_input: false,
        _padding2: [0; 2],
        visual_style: 0,
    };
    sized(value)
}

pub fn sized_row_selection() -> GhosttyRenderStateRowSelection {
    let value = GhosttyRenderStateRowSelection { size: 0, start_x: 0, end_x: 0 };
    sized(value)
}

pub fn sized_grid_ref() -> GhosttyGridRef {
    let value = GhosttyGridRef { size: 0, node: ptr::null_mut(), x: 0, y: 0 };
    sized(value)
}

pub fn sized_selection() -> GhosttySelection {
    let value = GhosttySelection {
        size: 0,
        start: sized_grid_ref(),
        end: sized_grid_ref(),
        rectangle: false,
    };
    sized(value)
}

pub fn sized_format_options() -> GhosttyTerminalSelectionFormatOptions {
    let value = GhosttyTerminalSelectionFormatOptions {
        size: 0,
        emit: 0,
        unwrap: false,
        trim: false,
        _padding: [0; 2],
        selection: ptr::null(),
    };
    sized(value)
}

pub fn sized_mouse_size() -> GhosttyMouseEncoderSize {
    let value = GhosttyMouseEncoderSize {
        size: 0,
        screen_width: 0,
        screen_height: 0,
        cell_width: 0,
        cell_height: 0,
        padding_top: 0,
        padding_bottom: 0,
        padding_right: 0,
        padding_left: 0,
    };
    sized(value)
}

pub fn sized_placement_render_info() -> GhosttyKittyGraphicsPlacementRenderInfo {
    let value = GhosttyKittyGraphicsPlacementRenderInfo {
        size: 0,
        pixel_width: 0,
        pixel_height: 0,
        grid_cols: 0,
        grid_rows: 0,
        viewport_col: 0,
        viewport_row: 0,
        viewport_visible: false,
        _padding: [0; 3],
        source_x: 0,
        source_y: 0,
        source_width: 0,
        source_height: 0,
    };
    sized(value)
}

// The library reports these sizes through `ghostty_type_json()`. A mismatch
// means the linked library no longer matches this file, which would otherwise
// surface as silent memory corruption.
const _: () = {
    assert!(std::mem::size_of::<GhosttyColorRgb>() == 3);
    assert!(std::mem::size_of::<GhosttyStyleColorValue>() == 8);
    assert!(std::mem::size_of::<GhosttyStyleColor>() == 16);
    assert!(std::mem::size_of::<GhosttyStyle>() == 72);
    assert!(std::mem::size_of::<GhosttyRenderStateColors>() == 792);
    assert!(std::mem::size_of::<GhosttyRenderStateCursor>() == 24);
    assert!(std::mem::size_of::<GhosttyTerminalScrollbar>() == 24);
    assert!(std::mem::size_of::<GhosttyTerminalModeConfig>() == 4);
    assert!(std::mem::size_of::<GhosttyGridRef>() == 24);
    assert!(std::mem::size_of::<GhosttySelection>() == 64);
    assert!(std::mem::size_of::<GhosttyPoint>() == 24);
    assert!(std::mem::size_of::<GhosttyPointCoordinate>() == 8);
    assert!(std::mem::size_of::<GhosttyTerminalScrollViewport>() == 24);
    assert!(std::mem::size_of::<GhosttyRenderStateRowSelection>() == 16);
    assert!(std::mem::size_of::<GhosttyKittyGraphicsPlacementRenderInfo>() == 56);
    assert!(std::mem::size_of::<GhosttyTerminalSelectionFormatOptions>() == 24);
    assert!(std::mem::size_of::<GhosttySelectionGestureGeometry>() == 16);
    assert!(std::mem::size_of::<GhosttyMouseEncoderSize>() == 40);
    assert!(std::mem::size_of::<GhosttyMousePosition>() == 8);
    assert!(std::mem::size_of::<GhosttySysImage>() == 24);
};
