//! Owned FFI values for the private FreeType/HarfBuzz/CoreText bridge.
use cove_core::{GlyphLineMetrics, GlyphMetrics, ShapedGlyph};
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
};
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct Metrics {
    pub glyph_index: u32,
    pub width: f32,
    pub height: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
}
impl From<Metrics> for GlyphMetrics {
    fn from(m: Metrics) -> Self {
        Self {
            glyph_index: m.glyph_index,
            width: m.width,
            height: m.height,
            bearing_x: m.bearing_x,
            bearing_y: m.bearing_y,
            advance: m.advance,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}
impl From<LineMetrics> for GlyphLineMetrics {
    fn from(m: LineMetrics) -> Self {
        Self {
            ascent: m.ascent,
            descent: m.descent,
            line_height: m.line_height,
        }
    }
}
#[repr(C)]
pub(crate) struct Bitmap {
    pub pixels: *mut u8,
    pub length: usize,
    pub width: u32,
    pub height: u32,
    pub metrics: Metrics,
    pub valid: c_int,
}
impl Bitmap {
    pub fn pixels(&self, channels: usize) -> Option<Vec<u8>> {
        if self.valid == 0
            || (self.width as usize)
                .checked_mul(self.height as usize)?
                .checked_mul(channels)?
                != self.length
        {
            return None;
        }
        if self.length == 0 {
            return Some(vec![]);
        }
        if self.pixels.is_null() {
            return None;
        }
        // SAFETY: a valid bridge result owns length initialized bytes, retained until this value is dropped.
        Some(unsafe { std::slice::from_raw_parts(self.pixels, self.length) }.to_vec())
    }
}
impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: the native allocation is uniquely owned by this FFI value; free accepts null.
        unsafe {
            cove_text_free(self.pixels.cast());
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Glyph {
    glyph_id: u32,
    x_offset: f32,
    y_offset: f32,
    x_advance: f32,
    y_advance: f32,
    cluster: u32,
    font_id: i64,
}
#[repr(C)]
pub(crate) struct ShapedRun {
    glyphs: *mut Glyph,
    length: usize,
}
impl ShapedRun {
    pub fn into_glyphs(self) -> Vec<ShapedGlyph> {
        if self.length == 0 || self.glyphs.is_null() {
            return vec![];
        }
        // SAFETY: bridge returns length initialized Glyph entries with this exact repr(C) layout.
        unsafe { std::slice::from_raw_parts(self.glyphs, self.length) }
            .iter()
            .map(|g| ShapedGlyph {
                glyph_id: g.glyph_id,
                x_offset: g.x_offset,
                y_offset: g.y_offset,
                x_advance: g.x_advance,
                y_advance: g.y_advance,
                cluster: g.cluster,
                font_id: g.font_id,
            })
            .collect()
    }
}
impl Drop for ShapedRun {
    fn drop(&mut self) {
        // SAFETY: the bridge allocated this array and this value is its unique owner.
        unsafe {
            cove_text_free(self.glyphs.cast());
        }
    }
}
#[repr(C)]
pub(crate) struct Axis {
    pub tag: u32,
    pub name: *mut c_char,
    pub minimum: f64,
    pub default_value: f64,
    pub maximum: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Axes {
    pub axes: *mut Axis,
    pub length: usize,
}
pub(crate) struct OwnedAxes(pub Axes);
impl Drop for OwnedAxes {
    fn drop(&mut self) {
        // SAFETY: this guard owns the complete native axes/name allocation exactly once.
        unsafe {
            cove_text_axes_free(self.0);
        }
    }
}
pub(crate) unsafe fn borrowed_string(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        // SAFETY: caller guarantees a live NUL-terminated native string.
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned()
    }
}
pub(crate) unsafe fn take_string(value: *mut c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    // SAFETY: the bridge returned an owned NUL-terminated allocation.
    let result = unsafe { borrowed_string(value) };
    // SAFETY: allocation has been copied and is no longer borrowed.
    unsafe {
        cove_text_free(value.cast());
    }
    Some(result)
}
pub(crate) fn copy_rows(
    source: &[u8],
    source_stride: usize,
    target: &mut [u8],
    target_stride: usize,
    bytes: usize,
    rows: usize,
) {
    if rows == 0 || bytes == 0 {
        return;
    }
    assert!(source_stride >= bytes && target_stride >= bytes);
    assert!(
        source_stride
            .checked_mul(rows - 1)
            .and_then(|n| n.checked_add(bytes))
            .is_some_and(|n| n <= source.len())
    );
    assert!(
        target_stride
            .checked_mul(rows - 1)
            .and_then(|n| n.checked_add(bytes))
            .is_some_and(|n| n <= target.len())
    );
    // SAFETY: checked strides/ranges cover every row; slices are live and have exclusive destination access.
    unsafe {
        cove_text_copy_rows(
            source.as_ptr(),
            source_stride,
            target.as_mut_ptr(),
            target_stride,
            bytes,
            rows,
        );
    }
}
unsafe extern "C" {
    pub fn cove_text_library_new() -> *mut c_void;
    pub fn cove_text_library_free(library: *mut c_void);
    pub fn cove_text_face_new(
        library: *mut c_void,
        bytes: *const u8,
        length: usize,
        selector: u32,
        size: f32,
        scale: f32,
    ) -> *mut c_void;
    pub fn cove_text_face_free(face: *mut c_void);
    pub fn cove_text_face_name(face: *mut c_void) -> *const c_char;
    pub fn cove_text_face_color(face: *mut c_void) -> c_int;
    pub fn cove_text_face_size(face: *mut c_void, size: f32, scale: f32) -> c_int;
    pub fn cove_text_glyph_index(face: *mut c_void, scalar: u32) -> u32;
    pub fn cove_text_probe(
        library: *mut c_void,
        bytes: *const u8,
        length: usize,
        name: *const c_char,
    ) -> i64;
    pub fn cove_text_axes(face: *mut c_void) -> Axes;
    pub fn cove_text_axes_free(axes: Axes);
    pub fn cove_text_apply_axes(
        face: *mut c_void,
        tags: *const u32,
        values: *const f64,
        count: usize,
    ) -> c_int;
    pub fn cove_text_glyph_metrics(
        face: *mut c_void,
        glyph: u32,
        layout: c_int,
        metrics: *mut Metrics,
    ) -> c_int;
    pub fn cove_text_line_metrics(face: *mut c_void, color: c_int) -> LineMetrics;
    pub fn cove_text_glyph_bitmap(face: *mut c_void, glyph: u32, color: c_int) -> Bitmap;
    pub fn cove_text_shape(
        face: *mut c_void,
        text: *const u8,
        length: usize,
        direction: u32,
        script: u32,
        language: *const c_char,
        offset: u32,
        font_id: i64,
    ) -> ShapedRun;
    pub fn cove_text_script(scalar: u32) -> u32;
    pub fn cove_text_free(memory: *mut c_void);
    fn cove_text_copy_rows(
        source: *const u8,
        source_stride: usize,
        target: *mut u8,
        target_stride: usize,
        bytes: usize,
        rows: usize,
    );
    pub fn cove_text_resample_rgba(
        source: *const u8,
        width: usize,
        height: usize,
        target: *mut u8,
        target_width: usize,
        target_height: usize,
    );
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
unsafe extern "C" {
    pub fn cove_apple_font_new(
        name: *const c_char,
        size: f32,
        weight: f32,
        italic: c_int,
    ) -> *mut c_void;
    pub fn cove_apple_font_copy(font: *mut c_void, size: f32) -> *mut c_void;
    pub fn cove_apple_font_free(font: *mut c_void);
    pub fn cove_apple_font_name(font: *mut c_void, family: c_int) -> *mut c_char;
    pub fn cove_apple_font_path(font: *mut c_void) -> *mut c_char;
    pub fn cove_apple_font_variations(
        font: *mut c_void,
        tags: *mut u32,
        values: *mut f64,
        capacity: usize,
    ) -> usize;
    pub fn cove_apple_font_fallback(
        font: *mut c_void,
        text: *const u8,
        length: usize,
        location: usize,
        range_length: usize,
    ) -> *mut c_void;
    pub fn cove_apple_color_metrics(
        font: *mut c_void,
        scale: f32,
        glyph: u32,
        metrics: *mut Metrics,
    ) -> c_int;
    pub fn cove_apple_line_metrics(font: *mut c_void) -> LineMetrics;
    pub fn cove_apple_color_bitmap(font: *mut c_void, scale: f32, glyph: u32) -> Bitmap;
    pub fn cove_apple_shape(
        font: *mut c_void,
        text: *const u8,
        length: usize,
        offset: u32,
        font_id: i64,
    ) -> ShapedRun;
}
pub(crate) const NO_LANGUAGE: *const c_char = ptr::null();
