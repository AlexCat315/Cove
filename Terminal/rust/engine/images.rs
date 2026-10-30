//! Kitty graphics: placement geometry and image pixels.
//!
//! Pixel values use the cell size last reported to the engine.

use crate::engine::TerminalEngine;
use crate::engine::ffi::{self, GhosttyKittyGraphics, GhosttyKittyGraphicsImage, GhosttyKittyGraphicsPlacementIterator};
use std::ptr;

/// Where a Kitty graphics placement sits in the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TerminalImagePlacement {
    pub image_id: u32,
    /// Changes whenever the image's pixels change (retransmission, animation).
    pub generation: u64,
    pub column: usize,
    pub row: usize,
    pub width: usize,
    pub height: usize,
    /// Visible part of the image, in image pixels.
    pub source: (usize, usize, usize, usize),
    pub image_size: (usize, usize),
    /// Negative z draws under text (`z<0` in the protocol).
    pub is_below_text: bool,
    /// Pixel offset inside the top-left cell (Unicode placeholders centre
    /// images).
    pub offset: (usize, usize),
}

/// The image handle plus the metadata every caller needs.
pub(super) struct KittyImage {
    pub width: u32,
    pub height: u32,
    pub generation: u64,
}

impl KittyImage {
    pub(super) fn lookup(graphics: GhosttyKittyGraphics, image_id: u32) -> Option<Self> {
        // SAFETY: `graphics` is a live handle borrowed from the terminal and the
        // returned image pointer stays valid until the next mutating call, which
        // is not made here.
        unsafe {
            let image = ffi::ghostty_kitty_graphics_image(graphics, image_id);
            if image.is_null() {
                return None;
            }
            Some(Self { width: Self::width(image), height: Self::height(image), generation: Self::generation(image) })
        }
    }

    unsafe fn width(image: GhosttyKittyGraphicsImage) -> u32 {
        let mut value: u32 = 0;
        // SAFETY: caller guarantees a live image handle.
        unsafe { ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_WIDTH, ptr::addr_of_mut!(value).cast()) };
        value
    }

    unsafe fn height(image: GhosttyKittyGraphicsImage) -> u32 {
        let mut value: u32 = 0;
        // SAFETY: caller guarantees a live image handle.
        unsafe { ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_HEIGHT, ptr::addr_of_mut!(value).cast()) };
        value
    }

    unsafe fn generation(image: GhosttyKittyGraphicsImage) -> u64 {
        let mut value: u64 = 0;
        // SAFETY: caller guarantees a live image handle.
        unsafe { ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_GENERATION, ptr::addr_of_mut!(value).cast()) };
        value
    }
}

impl TerminalEngine {
    /// The terminal's Kitty graphics storage, if graphics are enabled.
    pub(super) fn kitty_graphics(&self) -> Option<GhosttyKittyGraphics> {
        let mut graphics: GhosttyKittyGraphics = ptr::null_mut();
        // SAFETY: the terminal handle is live and `graphics` points at a slot of
        // the documented type.
        let result = unsafe {
            ffi::ghostty_terminal_get(self.terminal, ffi::DATA_KITTY_GRAPHICS, ptr::addr_of_mut!(graphics).cast())
        };
        (result == ffi::GHOSTTY_SUCCESS && !graphics.is_null()).then_some(graphics)
    }

    /// Visible placements, in protocol z order.
    pub(super) fn image_placements(&self) -> Vec<TerminalImagePlacement> {
        let Some(graphics) = self.kitty_graphics() else {
            return Vec::new();
        };
        let mut placements: Vec<(i32, usize, TerminalImagePlacement)> = Vec::new();
        // SAFETY: the iterator is created and freed here; `graphics` and the
        // terminal stay live for every call in between.
        unsafe {
            let mut iterator: GhosttyKittyGraphicsPlacementIterator = ptr::null_mut();
            if ffi::ghostty_kitty_graphics_placement_iterator_new(ptr::null(), &mut iterator) != ffi::GHOSTTY_SUCCESS
                || iterator.is_null()
            {
                return Vec::new();
            }
            let mut handle = iterator;
            if ffi::ghostty_kitty_graphics_get(graphics, ffi::KITTY_GRAPHICS_DATA_PLACEMENT_ITERATOR, ptr::addr_of_mut!(handle).cast())
                != ffi::GHOSTTY_SUCCESS
            {
                ffi::ghostty_kitty_graphics_placement_iterator_free(iterator);
                return Vec::new();
            }
            let mut order = 0usize;
            while ffi::ghostty_kitty_graphics_placement_next(iterator) {
                let offset = order;
                order += 1;
                let mut image_id: u32 = 0;
                let mut z: i32 = 0;
                ffi::ghostty_kitty_graphics_placement_get(iterator, ffi::PLACEMENT_DATA_IMAGE_ID, ptr::addr_of_mut!(image_id).cast());
                ffi::ghostty_kitty_graphics_placement_get(iterator, ffi::PLACEMENT_DATA_Z, ptr::addr_of_mut!(z).cast());
                let Some(image) = ffi::ghostty_kitty_graphics_image(graphics, image_id).as_ref() else {
                    continue;
                };
                let mut info = ffi::sized_placement_render_info();
                if ffi::ghostty_kitty_graphics_placement_render_info(iterator, image, self.terminal, &mut info) != ffi::GHOSTTY_SUCCESS
                    || !info.viewport_visible
                    || info.pixel_width == 0
                    || info.pixel_height == 0
                {
                    continue;
                }
                let mut generation: u64 = 0;
                let mut width: u32 = 0;
                let mut height: u32 = 0;
                ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_GENERATION, ptr::addr_of_mut!(generation).cast());
                ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_WIDTH, ptr::addr_of_mut!(width).cast());
                ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_HEIGHT, ptr::addr_of_mut!(height).cast());
                placements.push((
                    z,
                    offset,
                    TerminalImagePlacement {
                        image_id,
                        generation,
                        column: info.viewport_col.max(0) as usize,
                        row: info.viewport_row.max(0) as usize,
                        width: info.pixel_width as usize,
                        height: info.pixel_height as usize,
                        source: (
                            info.source_x as usize,
                            info.source_y as usize,
                            info.source_width as usize,
                            info.source_height as usize,
                        ),
                        image_size: (width as usize, height as usize),
                        is_below_text: z < 0,
                        offset: (0, 0),
                    },
                ));
            }
            ffi::ghostty_kitty_graphics_placement_iterator_free(iterator);
        }
        // Stable within a z layer: equal z keeps protocol order.
        placements.sort_by_key(|(z, order, _)| (*z, *order));
        placements.into_iter().map(|(_, _, placement)| placement).collect()
    }

    /// The image's current pixels as straight-alpha RGBA8.
    pub fn image_pixels(&self, image_id: u32) -> Option<cove_imaging::DecodedImage> {
        let graphics = self.kitty_graphics()?;
        // SAFETY: handles stay live for the duration of the reads below.
        unsafe {
            let image = ffi::ghostty_kitty_graphics_image(graphics, image_id);
            if image.is_null() {
                return None;
            }
            let mut width: u32 = 0;
            let mut height: u32 = 0;
            let mut length: usize = 0;
            let mut format: i32 = ffi::KITTY_IMAGE_FORMAT_RGBA;
            let mut data: *const u8 = ptr::null();
            ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_WIDTH, ptr::addr_of_mut!(width).cast());
            ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_HEIGHT, ptr::addr_of_mut!(height).cast());
            ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_FORMAT, ptr::addr_of_mut!(format).cast());
            ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_DATA_LEN, ptr::addr_of_mut!(length).cast());
            if ffi::ghostty_kitty_graphics_image_get(image, ffi::KITTY_IMAGE_DATA_DATA_PTR, ptr::addr_of_mut!(data).cast())
                != ffi::GHOSTTY_SUCCESS
                || data.is_null()
                || width == 0
                || height == 0
            {
                return None;
            }
            let pixels = width as usize * height as usize;
            let channels = match format {
                ffi::KITTY_IMAGE_FORMAT_RGB => 3,
                ffi::KITTY_IMAGE_FORMAT_GRAY_ALPHA => 2,
                ffi::KITTY_IMAGE_FORMAT_GRAY => 1,
                _ => 4,
            };
            if length < pixels * channels {
                return None;
            }
            let source = std::slice::from_raw_parts(data, pixels * channels);
            let mut rgba = vec![255u8; pixels * 4];
            for index in 0..pixels {
                let input = index * channels;
                let output = index * 4;
                match channels {
                    4 => {
                        rgba[output] = source[input];
                        rgba[output + 1] = source[input + 1];
                        rgba[output + 2] = source[input + 2];
                        rgba[output + 3] = source[input + 3];
                    }
                    3 => {
                        rgba[output] = source[input];
                        rgba[output + 1] = source[input + 1];
                        rgba[output + 2] = source[input + 2];
                    }
                    2 => {
                        rgba[output] = source[input];
                        rgba[output + 1] = source[input];
                        rgba[output + 2] = source[input];
                        rgba[output + 3] = source[input + 1];
                    }
                    _ => {
                        rgba[output] = source[input];
                        rgba[output + 1] = source[input];
                        rgba[output + 2] = source[input];
                    }
                }
            }
            cove_imaging::DecodedImage::new(width, height, rgba).ok()
        }
    }
}
