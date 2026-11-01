//! Copies one updated Ghostty render state into a `TerminalFrame`.
//!
//! This is the parser hot path: one row iterator pass per frame, one cells
//! iterator per row, and a small reused buffer for grapheme clusters so a full
//! repaint does not allocate per cell.

use crate::engine::TerminalEngine;
use crate::engine::ffi::{
    self, GhosttyRenderState, GhosttyRenderStateRowCells, GhosttyRenderStateRowIterator,
};
use crate::engine::frame::{
    CellWidth, CursorShape, TerminalCell, TerminalCellAttributes, TerminalCursor, TerminalFrame,
    TerminalRGB,
};
use crate::engine::placeholders::PlaceholderCell;
use std::ptr;

/// Owned render-state handles, reused across frames.
pub(crate) struct RenderState {
    pub state: GhosttyRenderState,
    pub row_iterator: GhosttyRenderStateRowIterator,
    pub row_cells: GhosttyRenderStateRowCells,
}

impl RenderState {
    pub(crate) fn new() -> Self {
        // SAFETY: all three handles are owned by this value and freed in `Drop`.
        unsafe {
            let mut state: GhosttyRenderState = ptr::null_mut();
            let mut row_iterator: GhosttyRenderStateRowIterator = ptr::null_mut();
            let mut row_cells: GhosttyRenderStateRowCells = ptr::null_mut();
            if ffi::ghostty_render_state_new(ptr::null(), &mut state) != ffi::GHOSTTY_SUCCESS
                || ffi::ghostty_render_state_row_iterator_new(ptr::null(), &mut row_iterator)
                    != ffi::GHOSTTY_SUCCESS
                || ffi::ghostty_render_state_row_cells_new(ptr::null(), &mut row_cells)
                    != ffi::GHOSTTY_SUCCESS
            {
                panic!("libghostty-vt: render state allocation failed");
            }
            Self {
                state,
                row_iterator,
                row_cells,
            }
        }
    }
}

impl Drop for RenderState {
    fn drop(&mut self) {
        // SAFETY: the handles were allocated by the library and are freed once,
        // cells before the iterator before the state that owns them.
        unsafe {
            ffi::ghostty_render_state_row_cells_free(self.row_cells);
            ffi::ghostty_render_state_row_iterator_free(self.row_iterator);
            ffi::ghostty_render_state_free(self.state);
        }
    }
}

/// Scratch buffers a frame read reuses.
#[derive(Default)]
pub(crate) struct FrameScratch {
    graphemes: Vec<u8>,
}

impl TerminalEngine {
    pub(crate) fn read_frame(&mut self) -> TerminalFrame {
        let state = &self.render;
        // SAFETY: the render state and terminal are live for the whole read; the
        // library only writes into the scratch buffer we own.
        unsafe {
            ffi::ghostty_render_state_update(state.state, self.terminal);
        }
        let mut frame = TerminalFrame::new(self.columns, self.rows);
        let scratch = &mut self.scratch.graphemes;
        // SAFETY: as above; every out pointer is a live local of the documented type.
        unsafe {
            let mut colors = ffi::sized_colors();
            if ffi::ghostty_render_state_get(
                state.state,
                ffi::RENDER_STATE_DATA_COLORS,
                ptr::addr_of_mut!(colors).cast(),
            ) == ffi::GHOSTTY_SUCCESS
            {
                frame.foreground = rgb(colors.foreground);
                frame.background = rgb(colors.background);
            }
            frame.cursor = Self::read_cursor(state.state);

            let mut iterator = state.row_iterator;
            if ffi::ghostty_render_state_get(
                state.state,
                ffi::RENDER_STATE_DATA_ROW_ITERATOR,
                ptr::addr_of_mut!(iterator).cast(),
            ) != ffi::GHOSTTY_SUCCESS
            {
                return frame;
            }
            let mut y = 0usize;
            while y < frame.rows && ffi::ghostty_render_state_row_iterator_next(state.row_iterator)
            {
                Self::read_row(state.row_iterator, state.row_cells, &mut frame, y, scratch);
                y += 1;
            }
            // Mark the dirty state consumed, exactly as the Swift reader does.
            let clean = 0i32;
            ffi::ghostty_render_state_set(
                state.state,
                ffi::RENDER_STATE_OPTION_DIRTY,
                ptr::addr_of!(clean).cast(),
            );
        }
        frame
    }

    unsafe fn read_row(
        iterator: GhosttyRenderStateRowIterator,
        rows_cells: GhosttyRenderStateRowCells,
        frame: &mut TerminalFrame,
        y: usize,
        scratch: &mut Vec<u8>,
    ) {
        // SAFETY: caller guarantees a live iterator positioned on row `y` and a
        // reusable row-cells handle that outlives the read.
        unsafe {
            let mut cells = rows_cells;
            if ffi::ghostty_render_state_row_get(
                iterator,
                ffi::RENDER_STATE_ROW_DATA_CELLS,
                ptr::addr_of_mut!(cells).cast(),
            ) != ffi::GHOSTTY_SUCCESS
            {
                return;
            }
            let mut selection = ffi::sized_row_selection();
            let selected = (ffi::ghostty_render_state_row_get(
                iterator,
                ffi::RENDER_STATE_ROW_DATA_SELECTION,
                ptr::addr_of_mut!(selection).cast(),
            ) == ffi::GHOSTTY_SUCCESS)
                .then_some(selection.start_x as usize..=selection.end_x as usize);

            let mut x = 0usize;
            while x < frame.columns && ffi::ghostty_render_state_row_cells_next(cells) {
                let (mut cell, style) = Self::read_cell(cells, scratch);
                cell.is_selected = selected.as_ref().is_some_and(|range| range.contains(&x));
                if let Some(style) = style
                    && cell.text.chars().next().map(u32::from)
                        == Some(crate::engine::placeholders::PLACEHOLDER_CODEPOINT)
                    && let Some(placeholder) = PlaceholderCell::new(
                        &cell.text,
                        x,
                        y,
                        style_id(style.fg_color),
                        style_id(style.underline_color),
                    )
                {
                    frame.placeholders.push(placeholder);
                }
                frame.cells[y * frame.columns + x] = cell;
                x += 1;
            }
        }
    }

    /// The cell and, when it has explicit styling, its raw style.
    unsafe fn read_cell(
        cells: GhosttyRenderStateRowCells,
        scratch: &mut Vec<u8>,
    ) -> (TerminalCell, Option<ffi::GhosttyStyle>) {
        let mut cell = TerminalCell::blank();
        // SAFETY: caller guarantees a live cells iterator that has been advanced.
        unsafe {
            let mut raw: u64 = 0;
            if ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_RAW,
                ptr::addr_of_mut!(raw).cast(),
            ) == ffi::GHOSTTY_SUCCESS
            {
                let mut wide: i32 = ffi::CELL_WIDE_NARROW;
                ffi::ghostty_cell_get(raw, ffi::CELL_DATA_WIDE, ptr::addr_of_mut!(wide).cast());
                let mut link = false;
                ffi::ghostty_cell_get(
                    raw,
                    ffi::CELL_DATA_HAS_HYPERLINK,
                    ptr::addr_of_mut!(link).cast(),
                );
                cell.is_hyperlink = link;
                cell.width = match wide {
                    ffi::CELL_WIDE_WIDE => CellWidth::Wide,
                    ffi::CELL_WIDE_SPACER_TAIL | ffi::CELL_WIDE_SPACER_HEAD => CellWidth::Spacer,
                    _ => CellWidth::Narrow,
                };
            }
            cell.text = Self::read_graphemes(cells, scratch);

            let mut color = ffi::GhosttyColorRgb::default();
            if ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_FG_COLOR,
                ptr::addr_of_mut!(color).cast(),
            ) == ffi::GHOSTTY_SUCCESS
            {
                cell.foreground = Some(rgb(color));
            }
            if ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_BG_COLOR,
                ptr::addr_of_mut!(color).cast(),
            ) == ffi::GHOSTTY_SUCCESS
            {
                cell.background = Some(rgb(color));
            }

            let mut styled = false;
            ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_HAS_STYLING,
                ptr::addr_of_mut!(styled).cast(),
            );
            if !styled {
                return (cell, None);
            }
            let mut style = ffi::sized_style();
            if ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_STYLE,
                ptr::addr_of_mut!(style).cast(),
            ) != ffi::GHOSTTY_SUCCESS
            {
                return (cell, None);
            }
            let mut attributes = TerminalCellAttributes::empty();
            if style.bold {
                attributes |= TerminalCellAttributes::BOLD;
            }
            if style.italic {
                attributes |= TerminalCellAttributes::ITALIC;
            }
            if style.faint {
                attributes |= TerminalCellAttributes::FAINT;
            }
            if style.underline != 0 {
                attributes |= TerminalCellAttributes::UNDERLINE;
            }
            if style.strikethrough {
                attributes |= TerminalCellAttributes::STRIKETHROUGH;
            }
            if style.inverse {
                attributes |= TerminalCellAttributes::INVERSE;
            }
            if style.invisible {
                attributes |= TerminalCellAttributes::INVISIBLE;
            }
            if style.blink {
                attributes |= TerminalCellAttributes::BLINK;
            }
            cell.attributes = attributes;
            (cell, Some(style))
        }
    }

    /// The cell's grapheme cluster, decoded into `scratch` in place.
    ///
    /// `GRAPHEMES_UTF8` reports the required size with a null buffer: success
    /// with `len == 0` means the cell has no text, and `OUT_OF_SPACE` with `len`
    /// set means `len` bytes are needed.
    unsafe fn read_graphemes(cells: GhosttyRenderStateRowCells, scratch: &mut Vec<u8>) -> String {
        // SAFETY: caller guarantees a live, advanced cells iterator.
        unsafe {
            let mut probe = ffi::GhosttyBuffer {
                ptr: ptr::null_mut(),
                cap: 0,
                len: 0,
            };
            let result = ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_GRAPHEMES_UTF8,
                ptr::addr_of_mut!(probe).cast(),
            );
            if result == ffi::GHOSTTY_SUCCESS || probe.len == 0 {
                return String::new();
            }
            if scratch.len() < probe.len {
                scratch.resize(probe.len, 0);
            }
            let mut buffer = ffi::GhosttyBuffer {
                ptr: scratch.as_mut_ptr(),
                cap: scratch.len(),
                len: 0,
            };
            if ffi::ghostty_render_state_row_cells_get(
                cells,
                ffi::ROW_CELLS_DATA_GRAPHEMES_UTF8,
                ptr::addr_of_mut!(buffer).cast(),
            ) != ffi::GHOSTTY_SUCCESS
                || buffer.len == 0
            {
                return String::new();
            }
            // Cells hold whole grapheme clusters, which are always valid UTF-8; a
            // replacement character still shows something rather than dropping
            // the cell.
            String::from_utf8_lossy(&scratch[..buffer.len]).into_owned()
        }
    }

    unsafe fn read_cursor(state: GhosttyRenderState) -> TerminalCursor {
        let mut cursor = ffi::sized_cursor();
        // SAFETY: the render state is live and `cursor` is the documented type.
        unsafe {
            if ffi::ghostty_render_state_get(
                state,
                ffi::RENDER_STATE_DATA_CURSOR,
                ptr::addr_of_mut!(cursor).cast(),
            ) != ffi::GHOSTTY_SUCCESS
            {
                return TerminalCursor {
                    is_visible: false,
                    ..TerminalCursor::default()
                };
            }
        }
        let shape = match cursor.visual_style {
            ffi::GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BAR => CursorShape::Bar,
            ffi::GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_UNDERLINE => CursorShape::Underline,
            ffi::GHOSTTY_RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK_HOLLOW => CursorShape::HollowBlock,
            _ => CursorShape::Block,
        };
        TerminalCursor {
            is_visible: cursor.visible && cursor.viewport_has_value,
            column: cursor.viewport_x as usize,
            row: cursor.viewport_y as usize,
            shape,
            is_blinking: cursor.blinking,
        }
    }
}

fn rgb(color: ffi::GhosttyColorRgb) -> TerminalRGB {
    TerminalRGB {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

/// Kitty placeholder ids: a palette index, or 24-bit RGB read as an integer.
fn style_id(color: ffi::GhosttyStyleColor) -> u32 {
    match color.tag {
        ffi::GHOSTTY_STYLE_COLOR_PALETTE => (color.value.bits & 0xFF) as u32,
        ffi::GHOSTTY_STYLE_COLOR_RGB => {
            let bits = color.value.bits;
            (((bits & 0xFF) as u32) << 16)
                | ((((bits >> 8) & 0xFF) as u32) << 8)
                | ((bits >> 16) & 0xFF) as u32
        }
        _ => 0,
    }
}
