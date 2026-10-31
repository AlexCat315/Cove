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
thread_local! { static LIBRARY: RefCell<Weak<Library>> = RefCell::new(Weak::new()); }
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
        // SAFETY: native result owns its bitmap until the guard is dropped.
        let result = unsafe { native::cove_text_glyph_bitmap(self.pointer(), glyph, 0) };
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
