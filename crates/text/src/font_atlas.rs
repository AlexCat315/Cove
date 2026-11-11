//! Translation of CoveUIText/FontAtlas.swift.
use crate::{
    ColorGlyphSource, FontFace, FontFaceSelector, color_glyph_atlas::ColorGlyphAtlas, native,
};
use cove_core::{
    GlyphAtlasFormat, GlyphAtlasInfo, GlyphLineMetrics, GlyphMetrics, GlyphMetricsProvider,
};
use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtyRegion {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
impl DirtyRegion {
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: (self.x + self.width).max(other.x + other.width) - x,
            height: (self.y + self.height).max(other.y + other.height) - y,
        }
    }
}
#[derive(Clone, Debug)]
pub struct UploadPayload {
    pub region: DirtyRegion,
    pub pixels: Vec<u8>,
    pub texture_width: usize,
    pub texture_height: usize,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_id: i64,
    glyph: u32,
    size: u32,
    scale: u32,
    /// Horizontal offset in quarter pixels. Layout metrics use phase zero.
    phase: u8,
}
impl GlyphKey {
    fn new(font_id: i64, glyph: u32, size: f32, scale: f32, phase: u8) -> Self {
        Self {
            font_id,
            glyph,
            size: size.to_bits(),
            scale: scale.to_bits(),
            phase: phase & 3,
        }
    }
}
struct AtlasData {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    loaded: Option<FontFace>,
    font_size: f32,
    scale: f32,
    alpha_dirty: bool,
    alpha_full: bool,
    dirty: Option<DirtyRegion>,
    shelf_x: usize,
    shelf_y: usize,
    shelf_height: usize,
    metrics: HashMap<GlyphKey, GlyphMetrics>,
    raster: HashMap<GlyphKey, GlyphAtlasInfo>,
    registered: HashMap<i64, FontFace>,
    colors: HashMap<i64, ColorGlyphSource>,
    color_atlas: Option<ColorGlyphAtlas>,
}
#[derive(Clone)]
pub struct FontAtlas(Rc<RefCell<AtlasData>>);
impl FontAtlas {
    pub fn new(width: usize, height: usize) -> Self {
        assert!(
            width > 0 && height > 0 && width <= 16384 && height <= 16384,
            "atlas dimensions must be in 1..=16384"
        );
        Self(Rc::new(RefCell::new(AtlasData {
            width,
            height,
            pixels: vec![0; width * height],
            loaded: None,
            font_size: 0.0,
            scale: 1.0,
            alpha_dirty: false,
            alpha_full: false,
            dirty: None,
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
            metrics: HashMap::new(),
            raster: HashMap::new(),
            registered: HashMap::new(),
            colors: HashMap::new(),
            color_atlas: None,
        })))
    }
    pub fn identity(&self) -> u64 {
        Rc::as_ptr(&self.0) as usize as u64
    }
    pub fn width(&self) -> usize {
        self.0.borrow().width
    }
    pub fn height(&self) -> usize {
        self.0.borrow().height
    }
    pub fn font_size(&self) -> f32 {
        self.0.borrow().font_size
    }
    pub fn raster_scale(&self) -> f32 {
        self.0.borrow().scale
    }
    pub fn font_face(&self) -> Option<FontFace> {
        self.0.borrow().loaded.clone()
    }
    pub fn alpha_pixels(&self) -> Vec<u8> {
        self.0.borrow().pixels.clone()
    }
    pub fn is_dirty(&self) -> bool {
        let d = self.0.borrow();
        d.alpha_dirty
            || d.color_atlas
                .as_ref()
                .is_some_and(ColorGlyphAtlas::is_dirty)
    }
    pub fn is_full(&self) -> bool {
        let d = self.0.borrow();
        d.alpha_full || d.color_atlas.as_ref().is_some_and(|atlas| atlas.is_full)
    }
    pub fn load_font_path(&self, path: &Path, size: f32, scale: f32) -> bool {
        let Ok(face) = FontFace::from_path(
            path,
            size,
            scale,
            FontFaceSelector::default(),
            Default::default(),
        ) else {
            return false;
        };
        let mut d = self.0.borrow_mut();
        d.loaded = Some(face);
        d.font_size = size;
        d.scale = scale;
        d.pixels.fill(0);
        d.shelf_x = 0;
        d.shelf_y = 0;
        d.shelf_height = 0;
        d.metrics.clear();
        d.raster.clear();
        d.dirty = None;
        d.alpha_dirty = false;
        d.alpha_full = false;
        if let Some(color) = &mut d.color_atlas {
            color.reset();
        }
        true
    }
    pub fn load_font_bytes(&self, bytes: Vec<u8>, size: f32, scale: f32) -> bool {
        let Ok(face) = FontFace::from_bytes(
            bytes,
            size,
            scale,
            FontFaceSelector::default(),
            Default::default(),
        ) else {
            return false;
        };
        {
            let mut d = self.0.borrow_mut();
            d.loaded = Some(face);
            d.font_size = size;
            d.scale = scale;
            d.metrics.clear();
        }
        self.reset();
        true
    }
    pub fn register_face(&self, face: FontFace, font_id: i64) {
        self.0.borrow_mut().registered.insert(font_id, face);
    }
    pub fn register_color_source(&self, source: ColorGlyphSource, font_id: i64) {
        self.0.borrow_mut().colors.insert(font_id, source);
    }
    fn face(&self, font_id: i64) -> Option<FontFace> {
        let d = self.0.borrow();
        d.registered
            .get(&font_id)
            .cloned()
            .or_else(|| if font_id == 0 { d.loaded.clone() } else { None })
    }
    fn key(&self, glyph: u32, font_id: i64) -> Option<GlyphKey> {
        self.key_at(glyph, font_id, 0)
    }
    fn key_at(&self, glyph: u32, font_id: i64, phase: u8) -> Option<GlyphKey> {
        if let Some(source) = self.0.borrow().colors.get(&font_id) {
            // Colour glyphs come from embedded bitmap strikes, which have a
            // fixed pixel grid; they cannot be shifted by a quarter pixel.
            return Some(GlyphKey::new(
                font_id,
                glyph,
                source.size,
                source.raster_scale,
                0,
            ));
        }
        let face = self.face(font_id)?;
        Some(GlyphKey::new(
            font_id,
            glyph,
            face.size(),
            face.raster_scale().max(1.0),
            phase,
        ))
    }
    pub fn cached_glyph_info(&self, glyph: u32, font_id: i64) -> Option<GlyphAtlasInfo> {
        self.0
            .borrow()
            .raster
            .get(&self.key(glyph, font_id)?)
            .copied()
    }
    pub fn glyph_metrics(&self, glyph: u32, font_id: i64) -> Option<GlyphMetrics> {
        let key = self.key(glyph, font_id)?;
        if let Some(metrics) = self.0.borrow().metrics.get(&key).copied() {
            return Some(metrics);
        }
        let source = self.0.borrow().colors.get(&font_id).cloned();
        let metrics = if let Some(source) = source {
            (source.metrics)(glyph)
        } else {
            self.face(font_id)?.layout_metrics(glyph)
        }?;
        self.0.borrow_mut().metrics.insert(key, metrics);
        Some(metrics)
    }
    pub fn line_metrics(&self, font_id: i64) -> Option<GlyphLineMetrics> {
        let source = self.0.borrow().colors.get(&font_id).cloned();
        if let Some(source) = source {
            Some((source.line_metrics)())
        } else {
            self.face(font_id)?.layout_line_metrics()
        }
    }
    pub fn rasterize_glyph(&self, glyph: u32, font_id: i64) -> Option<GlyphAtlasInfo> {
        self.rasterize_glyph_at(glyph, font_id, 0)
    }
    /// Caches an outline glyph with the requested horizontal offset in quarter
    /// pixels (0–3). Use its bearings relative to a whole-pixel pen position.
    pub fn rasterize_glyph_at(
        &self,
        glyph: u32,
        font_id: i64,
        phase: u8,
    ) -> Option<GlyphAtlasInfo> {
        let key = self.key_at(glyph, font_id, phase)?;
        let phase = key.phase;
        if let Some(info) = self.0.borrow().raster.get(&key).copied() {
            return Some(info);
        }
        let source = self.0.borrow().colors.get(&font_id).cloned();
        if let Some(source) = source {
            let bitmap = (source.rasterize)(glyph)?;
            let mut d = self.0.borrow_mut();
            if d.color_atlas.is_none() {
                d.color_atlas = Some(ColorGlyphAtlas::new(d.width, d.height));
            }
            let info = d.color_atlas.as_mut()?.insert(&bitmap)?;
            d.metrics.insert(key, bitmap.metrics);
            d.raster.insert(key, info);
            return Some(info);
        }
        let _ = self.glyph_metrics(glyph, font_id);
        let face = self.face(font_id)?;
        let bitmap = face.rasterize_at(glyph, phase)?;
        let (w, h) = (bitmap.width, bitmap.height);
        let mut d = self.0.borrow_mut();
        if w > 0 && h > 0 && d.shelf_x + w + 1 > d.width {
            d.shelf_y += d.shelf_height + 1;
            d.shelf_x = 0;
            d.shelf_height = 0;
        }
        if w > 0 && h > 0 && (w + 1 > d.width || d.shelf_y + h + 1 > d.height) {
            d.alpha_full = true;
            return None;
        }
        let (x, y) = (d.shelf_x, d.shelf_y);
        if w > 0 && h > 0 {
            d.shelf_x += w + 1;
            d.shelf_height = d.shelf_height.max(h);
            let width = d.width;
            native::copy_rows(
                &bitmap.pixels,
                w,
                &mut d.pixels[y * width + x..],
                width,
                w,
                h,
            );
            let next = DirtyRegion {
                x,
                y,
                width: w,
                height: h,
            };
            d.dirty = Some(d.dirty.map_or(next, |old| old.union(next)));
        }
        let metrics = bitmap.metrics;
        let info = GlyphAtlasInfo {
            glyph_index: glyph,
            width: metrics.width,
            height: metrics.height,
            bearing_x: metrics.bearing_x,
            bearing_y: metrics.bearing_y,
            advance: metrics.advance,
            uv_min_x: x as f32 / d.width as f32,
            uv_min_y: y as f32 / d.height as f32,
            uv_max_x: (x + w) as f32 / d.width as f32,
            uv_max_y: (y + h) as f32 / d.height as f32,
            format: GlyphAtlasFormat::Alpha,
        };
        d.raster.insert(key, info);
        d.alpha_dirty = true;
        Some(info)
    }
    pub fn rasterize_codepoint(&self, codepoint: char, font_id: i64) -> Option<GlyphAtlasInfo> {
        let glyph = self.face(font_id)?.glyph_index(codepoint);
        if glyph == 0 {
            None
        } else {
            self.rasterize_glyph(glyph, font_id)
        }
    }
    pub fn dirty_upload_payload(&self) -> Option<UploadPayload> {
        let d = self.0.borrow();
        let region = d.dirty?;
        if region.width == 0 || region.height == 0 {
            return None;
        }
        let mut pixels = vec![0; region.width * region.height];
        native::copy_rows(
            &d.pixels[region.y * d.width + region.x..],
            d.width,
            &mut pixels,
            region.width,
            region.width,
            region.height,
        );
        Some(UploadPayload {
            region,
            pixels,
            texture_width: d.width,
            texture_height: d.height,
        })
    }
    pub fn color_dirty_upload_payload(&self) -> Option<UploadPayload> {
        self.0.borrow().color_atlas.as_ref()?.payload()
    }
    pub fn reset(&self) {
        let mut d = self.0.borrow_mut();
        d.pixels.fill(0);
        d.raster.clear();
        d.alpha_full = false;
        d.alpha_dirty = true;
        d.shelf_x = 0;
        d.shelf_y = 0;
        d.shelf_height = 0;
        d.dirty = Some(DirtyRegion {
            x: 0,
            y: 0,
            width: d.width,
            height: d.height,
        });
        if let Some(color) = &mut d.color_atlas {
            color.reset();
        }
    }
    pub fn mark_clean(&self) {
        let mut d = self.0.borrow_mut();
        d.alpha_dirty = false;
        d.dirty = None;
        if let Some(color) = &mut d.color_atlas {
            color.mark_clean();
        }
    }
}
impl Default for FontAtlas {
    fn default() -> Self {
        Self::new(1024, 1024)
    }
}
impl GlyphMetricsProvider for FontAtlas {
    fn glyph_metrics(&self, glyph_index: u32, font_id: i64) -> Option<GlyphMetrics> {
        FontAtlas::glyph_metrics(self, glyph_index, font_id)
    }
    fn line_metrics(&self, font_id: i64) -> Option<GlyphLineMetrics> {
        FontAtlas::line_metrics(self, font_id)
    }
}
