//! Translation of CoveUIText/ColorGlyphAtlas.swift.
use crate::{ColorGlyphBitmap, DirtyRegion, UploadPayload, native};
use cove_core::{GlyphAtlasFormat, GlyphAtlasInfo};
pub struct ColorGlyphAtlas {
    pub width: usize,
    pub height: usize,
    pixels: Vec<u8>,
    shelf_x: usize,
    shelf_y: usize,
    shelf_height: usize,
    dirty: Option<DirtyRegion>,
    pub is_full: bool,
}
impl ColorGlyphAtlas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
            dirty: None,
            is_full: false,
        }
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty.is_some()
    }
    pub fn insert(&mut self, bitmap: &ColorGlyphBitmap) -> Option<GlyphAtlasInfo> {
        let (w, h) = (bitmap.width, bitmap.height);
        if w == 0
            || h == 0
            || w >= self.width
            || h >= self.height
            || w.checked_mul(h)?.checked_mul(4)? != bitmap.pixels.len()
        {
            if w >= self.width || h >= self.height {
                self.is_full = true;
            }
            return None;
        }
        if self.shelf_x + w + 1 > self.width {
            self.shelf_y += self.shelf_height + 1;
            self.shelf_x = 0;
            self.shelf_height = 0;
        }
        if self.shelf_y + h + 1 > self.height {
            self.is_full = true;
            return None;
        }
        let (x, y) = (self.shelf_x, self.shelf_y);
        self.shelf_x += w + 1;
        self.shelf_height = self.shelf_height.max(h);
        native::copy_rows(
            &bitmap.pixels,
            w * 4,
            &mut self.pixels[(y * self.width + x) * 4..],
            self.width * 4,
            w * 4,
            h,
        );
        let next = DirtyRegion {
            x,
            y,
            width: w,
            height: h,
        };
        self.dirty = Some(self.dirty.map_or(next, |previous| previous.union(next)));
        let metrics = bitmap.metrics;
        Some(GlyphAtlasInfo {
            glyph_index: metrics.glyph_index,
            width: metrics.width,
            height: metrics.height,
            bearing_x: metrics.bearing_x,
            bearing_y: metrics.bearing_y,
            advance: metrics.advance,
            uv_min_x: x as f32 / self.width as f32,
            uv_min_y: y as f32 / self.height as f32,
            uv_max_x: (x + w) as f32 / self.width as f32,
            uv_max_y: (y + h) as f32 / self.height as f32,
            format: GlyphAtlasFormat::Color,
        })
    }
    pub fn payload(&self) -> Option<UploadPayload> {
        let region = self.dirty?;
        let mut pixels = vec![0; region.width * region.height * 4];
        native::copy_rows(
            &self.pixels[(region.y * self.width + region.x) * 4..],
            self.width * 4,
            &mut pixels,
            region.width * 4,
            region.width * 4,
            region.height,
        );
        Some(UploadPayload {
            region,
            pixels,
            texture_width: self.width,
            texture_height: self.height,
        })
    }
    pub fn mark_clean(&mut self) {
        self.dirty = None;
    }
    pub fn reset(&mut self) {
        self.pixels.fill(0);
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;
        self.is_full = false;
        self.dirty = Some(DirtyRegion {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });
    }
}
