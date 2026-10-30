//! Kitty Unicode placeholder cells (U+10EEEE).
//!
//! Programs such as tmux-aware image viewers print these instead of placing
//! images by position: the foreground color carries the image id, the underline
//! color an optional placement id, and combining diacritics the row, column and
//! high id byte.

use crate::engine::TerminalEngine;
use crate::engine::frame::TerminalImagePlacement;
use crate::engine::placeholder_diacritics::PLACEHOLDER_DIACRITICS;

/// U+10EEEE, the placeholder base codepoint.
pub const PLACEHOLDER_CODEPOINT: u32 = 0x10EEEE;

/// A Kitty Unicode placeholder cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PlaceholderCell {
    pub column: usize,
    pub row: usize,
    pub image_id_low: u32,
    pub placement_id: u32,
    pub image_row: Option<usize>,
    pub image_column: Option<usize>,
    pub image_id_high: Option<u32>,
}

impl PlaceholderCell {
    /// Parses the diacritics of a placeholder grapheme; `None` for other text.
    ///
    /// Invalid diacritics count as missing, as in Kitty.
    pub fn new(text: &str, column: usize, row: usize, foreground: u32, underline: u32) -> Option<Self> {
        let mut scalars = text.chars();
        if scalars.next().map(u32::from) != Some(PLACEHOLDER_CODEPOINT) || foreground == 0 {
            return None;
        }
        Some(Self {
            column,
            row,
            image_id_low: foreground,
            placement_id: underline,
            image_row: scalars.next().and_then(|scalar| Self::index(u32::from(scalar))),
            image_column: scalars.next().and_then(|scalar| Self::index(u32::from(scalar))),
            image_id_high: scalars
                .next()
                .and_then(|scalar| Self::index(u32::from(scalar)))
                .and_then(|index| (index <= 255).then_some(index as u32)),
        })
    }

    /// Position of `codepoint` in the diacritic list, which is its encoded
    /// value. Binary search keeps this off the parser hot loop's allocation path.
    pub fn index(codepoint: u32) -> Option<usize> {
        PLACEHOLDER_DIACRITICS.binary_search(&codepoint).ok()
    }
}

/// A run of placeholder cells showing one horizontal strip of an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceholderRun {
    pub column: usize,
    pub row: usize,
    pub image_id: u32,
    pub placement_id: u32,
    pub image_column: usize,
    pub image_row: usize,
    pub width: usize,
}

impl PlaceholderRun {
    /// Groups placeholder cells into runs, row by row. A cell continues the run
    /// when it shows the same image and its omitted diacritics are implied by
    /// the cell to its left.
    pub fn runs(cells: &[PlaceholderCell]) -> Vec<Self> {
        let mut sorted = cells.to_vec();
        sorted.sort_by_key(|cell| (cell.row, cell.column));
        let mut result = Vec::new();
        // The open run and the high id byte it inherited from its first cell.
        let mut current: Option<(Self, Option<u32>)> = None;
        for cell in sorted {
            if let Some((open, high)) = current.as_mut()
                && open.row == cell.row
                && open.column + open.width == cell.column
                && open.image_id & 0xFF_FFFF == cell.image_id_low
                && open.placement_id == cell.placement_id
                && cell.image_row.is_none_or(|row| row == open.image_row)
                && cell.image_column.is_none_or(|column| column == open.image_column + open.width)
                && cell.image_id_high.is_none_or(|high_byte| Some(high_byte) == *high)
            {
                open.width += 1;
                continue;
            }
            if let Some((run, _)) = current.take() {
                result.push(run);
            }
            let high = cell.image_id_high;
            current = Some((
                Self {
                    column: cell.column,
                    row: cell.row,
                    image_id: cell.image_id_low | ((high.unwrap_or(0)) << 24),
                    placement_id: cell.placement_id,
                    image_column: cell.image_column.unwrap_or(0),
                    image_row: cell.image_row.unwrap_or(0),
                    width: 1,
                },
                high,
            ));
        }
        if let Some((run, _)) = current {
            result.push(run);
        }
        result
    }

    /// Where this strip of the image lands, fitting the whole image into the
    /// virtual placement's grid with its aspect ratio kept and centred.
    ///
    /// Mirrors Ghostty's `Placement.renderPlacement`.
    pub fn placement(
        &self,
        grid: (usize, usize),
        image: (usize, usize),
        generation: u64,
        cell: (usize, usize),
    ) -> Option<TerminalImagePlacement> {
        let (image_width, image_height) = (image.0 as f64, image.1 as f64);
        let (columns, rows) = (grid.0, grid.1);
        if image_width <= 0.0 || image_height <= 0.0 || columns == 0 || rows == 0 {
            return None;
        }
        let grid_width = (columns * cell.0) as f64;
        let grid_height = (rows * cell.1) as f64;

        let (scale, mut offset_x, mut offset_y);
        if image_width * grid_height > image_height * grid_width {
            scale = grid_width / image_width;
            offset_x = 0.0;
            offset_y = (grid_height - image_height * scale) / 2.0;
        } else {
            scale = grid_height / image_height;
            offset_x = (grid_width - image_width * scale) / 2.0;
            offset_y = 0.0;
        }
        let _ = &mut offset_x;
        // The image padded to the grid's aspect ratio, in image pixels.
        let pad_x = offset_x / scale;
        let pad_y = offset_y / scale;
        let padded_width = image_width + pad_x * 2.0;
        let padded_height = image_height + pad_y * 2.0;

        let mut source_x = padded_width * self.image_column as f64 / columns as f64;
        let mut source_y = padded_height * self.image_row as f64 / rows as f64;
        let mut source_width = padded_width * self.width as f64 / columns as f64;
        let mut source_height = padded_height / rows as f64;
        let mut dest_x = 0.0_f64;
        let mut dest_y = 0.0_f64;
        let mut dest_width = (self.width * cell.0) as f64;
        let mut dest_height = cell.1 as f64;

        if source_y < pad_y {
            let inset = pad_y - source_y;
            source_height -= inset;
            dest_y = inset;
            dest_height -= inset * scale;
            source_y = 0.0;
            if source_height > image_height {
                source_height = image_height;
                dest_height = image_height * scale;
            }
        } else if source_y + source_height > padded_height - pad_y {
            source_y -= pad_y;
            source_height = padded_height - pad_y - source_y - pad_y;
            dest_height = source_height * scale;
        } else {
            source_y -= pad_y;
        }
        if source_x < pad_x {
            let inset = pad_x - source_x;
            source_width -= inset;
            dest_x = inset;
            dest_width -= inset * scale;
            source_x = 0.0;
            if source_width > image_width {
                source_width = image_width;
                dest_width = image_width * scale;
            }
        } else if source_x + source_width > padded_width - pad_x {
            source_x -= pad_x;
            source_width = padded_width - pad_x - source_x - pad_x;
            dest_width = source_width * scale;
        } else {
            source_x -= pad_x;
        }
        if source_width <= 0.0 || source_height <= 0.0 {
            return None;
        }

        Some(TerminalImagePlacement {
            image_id: self.image_id,
            generation,
            column: self.column,
            row: self.row,
            width: dest_width.round() as usize,
            height: dest_height.round() as usize,
            source: (
                source_x.round() as usize,
                source_y.round() as usize,
                source_width.round() as usize,
                source_height.round() as usize,
            ),
            image_size: image,
            is_below_text: false,
            offset: ((dest_x * scale).round() as usize, (dest_y * scale).round() as usize),
        })
    }
}

impl TerminalEngine {
    /// Image strips drawn by placeholder cells in the captured frame.
    pub(super) fn placeholder_placements(&self, cells: &[PlaceholderCell]) -> Vec<TerminalImagePlacement> {
        if cells.is_empty() {
            return Vec::new();
        }
        let Some(graphics) = self.kitty_graphics() else {
            return Vec::new();
        };
        let targets = self.virtual_placements(graphics);
        let cell = self.cell_size();
        PlaceholderRun::runs(cells)
            .into_iter()
            .filter_map(|run| {
                // An explicit placement id must match; otherwise the image's
                // first virtual placement.
                let target = targets
                    .iter()
                    .find(|target| target.image_id == run.image_id && (run.placement_id == 0 || target.placement_id == run.placement_id))?;
                let image = crate::engine::images::KittyImage::lookup(graphics, run.image_id)?;
                // Unspecified rows or columns fit the image at its natural size.
                let columns = if target.columns > 0 {
                    target.columns
                } else {
                    (image.width as usize).div_ceil(cell.0)
                };
                let rows = if target.rows > 0 {
                    target.rows
                } else {
                    (image.height as usize).div_ceil(cell.1)
                };
                run.placement(
                    (columns, rows),
                    (image.width as usize, image.height as usize),
                    image.generation,
                    cell,
                )
            })
            .collect()
    }

    fn virtual_placements(&self, graphics: crate::engine::ffi::GhosttyKittyGraphics) -> Vec<VirtualPlacement> {
        let mut per_image = Vec::new();
        // SAFETY: `graphics` is a live handle borrowed from the terminal; the
        // iterator is created, used and freed here before any other call.
        unsafe {
            let mut iterator = std::ptr::null_mut();
            if crate::engine::ffi::ghostty_kitty_graphics_placement_iterator_new(std::ptr::null(), &mut iterator)
                != crate::engine::ffi::GHOSTTY_SUCCESS
                || iterator.is_null()
            {
                return per_image;
            }
            let mut handle = iterator;
            if crate::engine::ffi::ghostty_kitty_graphics_get(
                graphics,
                crate::engine::ffi::KITTY_GRAPHICS_DATA_PLACEMENT_ITERATOR,
                std::ptr::addr_of_mut!(handle).cast(),
            ) != crate::engine::ffi::GHOSTTY_SUCCESS
            {
                crate::engine::ffi::ghostty_kitty_graphics_placement_iterator_free(iterator);
                return per_image;
            }
            while crate::engine::ffi::ghostty_kitty_graphics_placement_next(iterator) {
                let mut is_virtual = false;
                crate::engine::ffi::ghostty_kitty_graphics_placement_get(
                    iterator,
                    crate::engine::ffi::PLACEMENT_DATA_IS_VIRTUAL,
                    std::ptr::addr_of_mut!(is_virtual).cast(),
                );
                if !is_virtual {
                    continue;
                }
                let mut image_id: u32 = 0;
                let mut placement_id: u32 = 0;
                let mut columns: u32 = 0;
                let mut rows: u32 = 0;
                crate::engine::ffi::ghostty_kitty_graphics_placement_get(
                    iterator,
                    crate::engine::ffi::PLACEMENT_DATA_IMAGE_ID,
                    std::ptr::addr_of_mut!(image_id).cast(),
                );
                crate::engine::ffi::ghostty_kitty_graphics_placement_get(
                    iterator,
                    crate::engine::ffi::PLACEMENT_DATA_PLACEMENT_ID,
                    std::ptr::addr_of_mut!(placement_id).cast(),
                );
                crate::engine::ffi::ghostty_kitty_graphics_placement_get(
                    iterator,
                    crate::engine::ffi::PLACEMENT_DATA_COLUMNS,
                    std::ptr::addr_of_mut!(columns).cast(),
                );
                crate::engine::ffi::ghostty_kitty_graphics_placement_get(
                    iterator,
                    crate::engine::ffi::PLACEMENT_DATA_ROWS,
                    std::ptr::addr_of_mut!(rows).cast(),
                );
                per_image.push(VirtualPlacement {
                    image_id,
                    placement_id,
                    columns: columns as usize,
                    rows: rows as usize,
                });
            }
            crate::engine::ffi::ghostty_kitty_graphics_placement_iterator_free(iterator);
        }
        per_image
    }
}

/// A virtual placement's declared grid size, before the image's own size is
/// consulted.
struct VirtualPlacement {
    image_id: u32,
    placement_id: u32,
    columns: usize,
    rows: usize,
}
