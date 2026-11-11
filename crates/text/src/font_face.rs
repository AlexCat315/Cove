//! Translation of CoveUIText/FontFace.swift.
use crate::{ColorGlyphSource, TextDirection, native};
use cove_core::{GlyphAtlasFormat, GlyphLineMetrics, GlyphMetrics, ShapedGlyph};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::{CString, c_void},
    path::Path,
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
};
pub type ScriptTag = u32;
pub struct Script;
impl Script {
    pub const LATIN: ScriptTag = 0x4c61746e;
    pub const COMMON: ScriptTag = 0x5a797979;
    pub const INHERITED: ScriptTag = 0x5a696e68;
    pub const UNKNOWN: ScriptTag = 0x5a7a7a7a;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontAxisId(pub u32);
#[derive(Clone, Debug, PartialEq)]
pub struct FontAxis {
    pub id: FontAxisId,
    pub name: String,
    pub minimum: f64,
    pub default_value: f64,
    pub maximum: f64,
}
#[derive(Clone, Debug)]
pub struct GlyphBitmap {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub metrics: GlyphMetrics,
    pub format: GlyphAtlasFormat,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontFaceSelector(pub u32);
impl FontFaceSelector {
    pub const fn new(face_index: u16, named_instance: u16) -> Self {
        Self(face_index as u32 | (named_instance as u32) << 16)
    }
    pub const fn face_index(self) -> u16 {
        (self.0 & 0xffff) as u16
    }
    pub const fn named_instance(self) -> u16 {
        (self.0 >> 16) as u16
    }
}
#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("invalid font size, scale, selector, or font data")]
    InvalidFont,
    #[error("FreeType could not initialize")]
    FreeTypeUnavailable,
    #[error("font variation update failed")]
    Variation,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
struct Library(NonNull<c_void>);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: this Rc owner outlives all faces created from the library; destruction runs on its owning render thread.
        unsafe {
            native::cove_text_library_free(self.0.as_ptr());
        }
    }
}
thread_local! { static LIBRARY: RefCell<Weak<Library>> = const { RefCell::new(Weak::new()) }; }
fn library() -> Result<Rc<Library>, TextError> {
    LIBRARY.with(|slot| {
        if let Some(existing) = slot.borrow().upgrade() {
            return Ok(existing);
        }
        // SAFETY: returns a new, uniquely owned FreeType library or null.
        let library = Rc::new(Library(
            NonNull::new(unsafe { native::cove_text_library_new() })
                .ok_or(TextError::FreeTypeUnavailable)?,
        ));
        *slot.borrow_mut() = Rc::downgrade(&library);
        Ok(library)
    })
}
struct FaceData {
    pointer: NonNull<c_void>,
    _library: Rc<Library>,
    storage: Arc<[u8]>,
    selector: FontFaceSelector,
    coordinates: BTreeMap<FontAxisId, f64>,
    name: String,
    size: Cell<f32>,
    scale: Cell<f32>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    color_font: RefCell<Option<crate::NativeColorGlyphFont>>,
}
impl Drop for FaceData {
    fn drop(&mut self) {
        // SAFETY: this is the unique native face owner; storage and library are still retained until after Drop.
        unsafe {
            native::cove_text_face_free(self.pointer.as_ptr());
        }
    }
}
#[derive(Clone)]
pub struct FontFace(Rc<FaceData>);
impl FontFace {
    pub fn from_bytes(
        bytes: impl Into<Arc<[u8]>>,
        size: f32,
        scale: f32,
        selector: FontFaceSelector,
        coordinates: BTreeMap<FontAxisId, f64>,
    ) -> Result<Self, TextError> {
        let storage = bytes.into();
        let library = library()?;
        // SAFETY: immutable storage is retained by FaceData for the entire native face lifetime, including the borrowed FT buffer.
        let pointer = NonNull::new(unsafe {
            native::cove_text_face_new(
                library.0.as_ptr(),
                storage.as_ptr(),
                storage.len(),
                selector.0,
                size,
                scale,
            )
        })
        .ok_or(TextError::InvalidFont)?;
        // SAFETY: face owns a live NUL-terminated name; copy it before changing variations.
        let name =
            unsafe { native::borrowed_string(native::cove_text_face_name(pointer.as_ptr())) };
        let face = Self(Rc::new(FaceData {
            pointer,
            _library: library,
            storage,
            selector,
            coordinates,
            name,
            size: Cell::new(size),
            scale: Cell::new(scale),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            color_font: RefCell::new(None),
        }));
        let tags = face
            .0
            .coordinates
            .keys()
            .map(|tag| tag.0)
            .collect::<Vec<_>>();
        let values = face.0.coordinates.values().copied().collect::<Vec<_>>();
        // SAFETY: paired arrays have identical lengths and remain live; mutation is confined to this render thread.
        if unsafe {
            native::cove_text_apply_axes(face.pointer(), tags.as_ptr(), values.as_ptr(), tags.len())
        } == 0
        {
            return Err(TextError::Variation);
        }
        Ok(face)
    }
    pub fn from_path(
        path: &Path,
        size: f32,
        scale: f32,
        selector: FontFaceSelector,
        coordinates: BTreeMap<FontAxisId, f64>,
    ) -> Result<Self, TextError> {
        Self::from_bytes(std::fs::read(path)?, size, scale, selector, coordinates)
    }
    pub(crate) fn pointer(&self) -> *mut c_void {
        self.0.pointer.as_ptr()
    }
    /// Resolved OpenType traits, including variation coordinates, for portable
    /// fallback matching. Unknown font containers retain the requested style.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub(crate) fn resolved_style(&self) -> Option<(cove_core::FontWeight, bool)> {
        resolved_font_style(
            &self.0.storage,
            self.0.selector.face_index(),
            &self.0.coordinates,
        )
    }

    pub fn post_script_name(&self) -> &str {
        &self.0.name
    }
    pub fn size(&self) -> f32 {
        self.0.size.get()
    }
    pub fn raster_scale(&self) -> f32 {
        self.0.scale.get()
    }
    pub fn face_index(&self) -> u16 {
        self.0.selector.face_index()
    }
    pub fn applied_design_coordinates(&self) -> &BTreeMap<FontAxisId, f64> {
        &self.0.coordinates
    }
    pub fn probe_selector(name: &str, bytes: &[u8]) -> Option<FontFaceSelector> {
        let library = library().ok()?;
        let name = CString::new(name).ok()?;
        // SAFETY: library, immutable font bytes, and NUL-terminated name remain live for the complete probe.
        let selector = unsafe {
            native::cove_text_probe(
                library.0.as_ptr(),
                bytes.as_ptr(),
                bytes.len(),
                name.as_ptr(),
            )
        };
        (selector >= 0).then_some(FontFaceSelector(selector as u32))
    }
    pub fn design_axes(&self) -> Vec<FontAxis> {
        // SAFETY: the face is live and the returned owned array is guarded until its names are copied.
        let axes = native::OwnedAxes(unsafe { native::cove_text_axes(self.pointer()) });
        if axes.0.length == 0 || axes.0.axes.is_null() {
            return vec![];
        }
        // SAFETY: a successful axes result contains exactly length initialized entries.
        unsafe { std::slice::from_raw_parts(axes.0.axes, axes.0.length) }
            .iter()
            .map(|axis| FontAxis {
                id: FontAxisId(axis.tag),
                // SAFETY: each name is a live native NUL-terminated allocation, or null.
                name: unsafe { native::borrowed_string(axis.name) },
                minimum: axis.minimum,
                default_value: axis.default_value,
                maximum: axis.maximum,
            })
            .collect()
    }
    pub fn resolved(&self, axes: BTreeMap<FontAxisId, f64>) -> Result<Self, TextError> {
        if axes.is_empty() {
            return Ok(self.clone());
        }
        let mut coordinates = self.0.coordinates.clone();
        coordinates.extend(axes);
        Self::from_bytes(
            self.0.storage.clone(),
            self.size(),
            self.raster_scale(),
            self.0.selector,
            coordinates,
        )
    }
    pub fn applying(&self, size: f32, scale: f32) -> bool {
        // SAFETY: only this render thread can access the opaque face; native code validates both parameters.
        if unsafe { native::cove_text_face_size(self.pointer(), size, scale) } == 0 {
            return false;
        }
        self.0.size.set(size);
        self.0.scale.set(scale);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let current = self.0.color_font.borrow().clone();
            if let Some(font) = current {
                *self.0.color_font.borrow_mut() = font.resized(size, scale);
            }
        }
        true
    }
    pub fn has_color_glyphs(&self) -> bool {
        // SAFETY: borrowed query against a live face.
        unsafe { native::cove_text_face_color(self.pointer()) != 0 }
    }
    pub fn color_source(&self) -> Option<ColorGlyphSource> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Some(font) = self.native_color_font() {
            return Some(font.source());
        }
        crate::free_type_color_glyphs::FreeTypeColorGlyphFace::new(self.clone())
            .map(|face| face.source())
    }
    pub fn has_color_source(&self) -> bool {
        self.color_source().is_some()
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn native_color_font(&self) -> Option<crate::NativeColorGlyphFont> {
        self.0.color_font.borrow().clone()
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub(crate) fn set_native_color_font(&self, font: crate::NativeColorGlyphFont) {
        *self.0.color_font.borrow_mut() = Some(font);
    }
    pub fn glyph_index(&self, scalar: char) -> u32 {
        // SAFETY: scalar is a valid Unicode scalar and the face is live on its owner thread.
        unsafe { native::cove_text_glyph_index(self.pointer(), scalar as u32) }
    }
    pub fn can_render(&self, text: &str) -> bool {
        text.chars().all(|c| self.glyph_index(c) != 0)
    }
    pub fn metrics(&self, glyph: u32) -> Option<GlyphMetrics> {
        self.measure(glyph, false)
    }
    pub(crate) fn layout_metrics(&self, glyph: u32) -> Option<GlyphMetrics> {
        self.measure(glyph, true)
    }
    fn measure(&self, glyph: u32, layout: bool) -> Option<GlyphMetrics> {
        let mut result = native::Metrics::default();
        // SAFETY: writable ABI-compatible metrics output and a live native face.
        (unsafe {
            native::cove_text_glyph_metrics(self.pointer(), glyph, i32::from(layout), &mut result)
        } != 0)
            .then(|| result.into())
    }
    pub fn line_metrics(&self) -> GlyphLineMetrics {
        // SAFETY: native metrics are returned by value for a live face.
        unsafe { native::cove_text_line_metrics(self.pointer(), 0) }.into()
    }
    pub(crate) fn layout_line_metrics(&self) -> Option<GlyphLineMetrics> {
        // SAFETY: mode 2 requests the original atlas's raw font line height.
        let metrics: GlyphLineMetrics =
            unsafe { native::cove_text_line_metrics(self.pointer(), 2) }.into();
        (metrics.ascent > 0.0 || metrics.descent > 0.0 || metrics.line_height > 0.0)
            .then_some(metrics)
    }
    pub fn rasterize(&self, glyph: u32) -> Option<GlyphBitmap> {
        self.rasterize_at(glyph, 0)
    }
    /// Rasterizes with a horizontal offset of `phase` quarter pixels (0–3).
    pub fn rasterize_at(&self, glyph: u32, phase: u8) -> Option<GlyphBitmap> {
        // SAFETY: native result owns its bitmap until the guard is dropped.
        let result = unsafe {
            native::cove_text_glyph_bitmap(self.pointer(), glyph, 0, i32::from(phase & 3))
        };
        Some(GlyphBitmap {
            pixels: result.pixels(1)?,
            width: result.width as usize,
            height: result.height as usize,
            metrics: result.metrics.into(),
            format: GlyphAtlasFormat::Alpha,
        })
    }
    pub fn shape(
        &self,
        text: &str,
        direction: Option<TextDirection>,
        script: Option<ScriptTag>,
        language: Option<&str>,
        utf8_offset: u32,
        font_id: i64,
    ) -> Vec<ShapedGlyph> {
        let language = language.and_then(|language| CString::new(language).ok());
        // SAFETY: face, UTF-8 bytes and optional language remain live; returned glyph storage is copied and freed by its guard.
        unsafe {
            native::cove_text_shape(
                self.pointer(),
                text.as_ptr(),
                text.len(),
                direction.map_or(0, |d| d as u32),
                script.unwrap_or(0),
                language
                    .as_ref()
                    .map_or(native::NO_LANGUAGE, |s| s.as_ptr()),
                utf8_offset,
                font_id,
            )
        }
        .into_glyphs()
    }
}
pub(crate) fn scalar_script(scalar: char) -> ScriptTag {
    // SAFETY: HarfBuzz's immutable default Unicode tables accept a Unicode scalar value.
    unsafe { native::cove_text_script(scalar as u32) }
}

#[cfg(any(test, not(any(target_os = "macos", target_os = "ios"))))]
fn resolved_font_style(
    bytes: &[u8],
    face: u16,
    coordinates: &BTreeMap<FontAxisId, f64>,
) -> Option<(cove_core::FontWeight, bool)> {
    let u16_at = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes(
            bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
        ))
    };
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(
            bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let base = if bytes.get(..4)? == b"ttcf" {
        if u32::from(face) >= u32_at(8)? {
            return None;
        }
        u32_at(12 + usize::from(face) * 4)? as usize
    } else {
        0
    };
    let tables = usize::from(u16_at(base.checked_add(4)?)?);
    let mut weight = None;
    let mut italic = false;
    for i in 0..tables {
        let entry = base.checked_add(12)?.checked_add(i.checked_mul(16)?)?;
        let tag = bytes.get(entry..entry.checked_add(4)?)?;
        let offset = u32_at(entry.checked_add(8)?)? as usize;
        let length = u32_at(entry.checked_add(12)?)? as usize;
        bytes.get(offset..offset.checked_add(length)?)?;
        if tag == b"OS/2" && length >= 6 {
            weight = Some(f64::from(u16_at(offset + 4)?));
            if length >= 64 {
                italic |= u16_at(offset + 62)? & 1 != 0;
            }
        } else if tag == b"head" && length >= 46 {
            let style = u16_at(offset + 44)?;
            if weight.is_none() {
                weight = Some(if style & 1 != 0 { 700.0 } else { 400.0 });
            }
            italic |= style & 2 != 0;
        }
    }
    let weight = coordinates
        .get(&FontAxisId(u32::from_be_bytes(*b"wght")))
        .copied()
        .or(weight)?;
    let ital = coordinates.get(&FontAxisId(u32::from_be_bytes(*b"ital")));
    let slnt = coordinates.get(&FontAxisId(u32::from_be_bytes(*b"slnt")));
    if ital.is_some() || slnt.is_some() {
        italic = ital.is_some_and(|v| *v > 0.0) || slnt.is_some_and(|v| *v != 0.0);
    }
    let weight = if weight >= 650.0 {
        cove_core::FontWeight::Bold
    } else if weight >= 550.0 {
        cove_core::FontWeight::Semibold
    } else if weight >= 450.0 {
        cove_core::FontWeight::Medium
    } else {
        cove_core::FontWeight::Regular
    };
    Some((weight, italic))
}

#[cfg(test)]
mod resolved_style_tests {
    use super::*;
    #[test]
    fn fallback_style_reads_static_and_variable_opentype_traits() {
        let mut bytes = vec![0; 92];
        bytes[4..6].copy_from_slice(&1_u16.to_be_bytes());
        bytes[12..16].copy_from_slice(b"OS/2");
        bytes[20..24].copy_from_slice(&28_u32.to_be_bytes());
        bytes[24..28].copy_from_slice(&64_u32.to_be_bytes());
        bytes[32..34].copy_from_slice(&700_u16.to_be_bytes());
        bytes[90..92].copy_from_slice(&1_u16.to_be_bytes());
        assert_eq!(
            resolved_font_style(&bytes, 0, &BTreeMap::new()),
            Some((cove_core::FontWeight::Bold, true))
        );
        let coordinates = BTreeMap::from([(FontAxisId(u32::from_be_bytes(*b"wght")), 500.0)]);
        assert_eq!(
            resolved_font_style(&bytes, 0, &coordinates),
            Some((cove_core::FontWeight::Medium, true))
        );
        assert_eq!(resolved_font_style(&bytes[..40], 0, &coordinates), None);
        let mut collection = vec![0; bytes.len() + 16];
        collection[..4].copy_from_slice(b"ttcf");
        collection[8..12].copy_from_slice(&1_u32.to_be_bytes());
        collection[12..16].copy_from_slice(&16_u32.to_be_bytes());
        collection[16..].copy_from_slice(&bytes);
        collection[36..40].copy_from_slice(&44_u32.to_be_bytes());
        assert_eq!(
            resolved_font_style(&collection, 0, &BTreeMap::new()),
            Some((cove_core::FontWeight::Bold, true))
        );
        assert_eq!(resolved_font_style(&collection, 1, &BTreeMap::new()), None);
        let coordinates = BTreeMap::from([(FontAxisId(u32::from_be_bytes(*b"ital")), 0.0)]);
        assert_eq!(
            resolved_font_style(&bytes, 0, &coordinates),
            Some((cove_core::FontWeight::Bold, false))
        );
    }
}
