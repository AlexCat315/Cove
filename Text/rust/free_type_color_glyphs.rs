//! Translation of CoveUIText/FreeTypeColorGlyphs.swift; pixel conversion/resampling remains in C.
use crate::{native, ColorGlyphBitmap, ColorGlyphSource, FontFace};
use cove_core::{GlyphLineMetrics, GlyphMetrics};
use std::rc::Rc;
#[derive(Clone)]
pub struct FreeTypeColorGlyphFace { face: FontFace, size: f32, scale: f32 }
impl FreeTypeColorGlyphFace {
    pub fn new(face: FontFace) -> Option<Self> { face.has_color_glyphs().then(|| Self { size: face.size(), scale: face.raster_scale(), face }) }
    pub fn source(self) -> ColorGlyphSource {
        let metrics = self.clone(); let lines = self.clone();
        ColorGlyphSource { size: self.size, raster_scale: self.scale, metrics: Rc::new(move |glyph| metrics.metrics(glyph)), line_metrics: Rc::new(move || lines.line_metrics()), rasterize: Rc::new(move |glyph| self.rasterize(glyph)) }
    }
    fn metrics(&self, glyph: u32) -> Option<GlyphMetrics> { self.rasterize(glyph).map(|bitmap| bitmap.metrics) }
    fn line_metrics(&self) -> GlyphLineMetrics {
        // SAFETY: native color metrics choose a strike on the owning render thread and return values by copy.
        unsafe { native::cove_text_line_metrics(self.face.pointer(), 1) }.into()
    }
    fn rasterize(&self, glyph: u32) -> Option<ColorGlyphBitmap> {
        // SAFETY: result owns a separate straight-RGBA buffer and is freed after copying it into Rust storage.
        let bitmap = unsafe { native::cove_text_glyph_bitmap(self.face.pointer(), glyph, 1) };
        Some(ColorGlyphBitmap { pixels: bitmap.pixels(4)?, width: bitmap.width as usize, height: bitmap.height as usize, metrics: bitmap.metrics.into() })
    }
    pub fn resample(pixels: &[u8], width: usize, height: usize, to_width: usize, to_height: usize) -> Option<Vec<u8>> {
        if width == 0 || height == 0 || to_width == 0 || to_height == 0 || width.checked_mul(height)?.checked_mul(4)? != pixels.len() { return None; }
        let mut output = vec![0; to_width.checked_mul(to_height)?.checked_mul(4)?];
        // SAFETY: source dimensions match its slice, destination has exactly the requested RGBA pixel count, and all dimensions are positive.
        unsafe { native::cove_text_resample_rgba(pixels.as_ptr(), width, height, output.as_mut_ptr(), to_width, to_height); } Some(output)
    }
}
