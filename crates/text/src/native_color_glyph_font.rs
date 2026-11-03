//! Translation of CoveUIText/NativeColorGlyphFont.swift and the CoreText handle operations used by FontProvider.swift.
#![cfg(any(target_os = "macos", target_os = "ios"))]
use crate::{ColorGlyphBitmap, ColorGlyphSource, FontAxisId, native};
use cove_core::{FontWeight, GlyphLineMetrics, GlyphMetrics, ShapedGlyph};
use std::{
    collections::BTreeMap,
    ffi::{CString, c_void},
    path::PathBuf,
    ptr::{self, NonNull},
    rc::Rc,
};
struct AppleFontData(NonNull<c_void>);
impl Drop for AppleFontData {
    fn drop(&mut self) {
        // SAFETY: the handle is a uniquely retained CTFont reference owned by this Rc allocation.
        unsafe {
            native::cove_apple_font_free(self.0.as_ptr());
        }
    }
}
#[derive(Clone)]
pub(crate) struct AppleFont(Rc<AppleFontData>);
impl AppleFont {
    pub fn new(name: Option<&str>, size: f32, weight: FontWeight, italic: bool) -> Option<Self> {
        let name = name.map(CString::new).transpose().ok()?;
        let weight = crate::FontWeightTextExt::core_text_weight(weight);
        // SAFETY: optional NUL-terminated name remains live; the bridge returns a retained CTFont or null.
        Self::owned(unsafe {
            native::cove_apple_font_new(
                name.as_ref().map_or(ptr::null(), |n| n.as_ptr()),
                size,
                weight,
                i32::from(italic),
            )
        })
    }
    /// Match a family by weight and slant without pinning a PostScript face name.
    pub fn family(name: &str, size: f32, weight: FontWeight, italic: bool) -> Option<Self> {
        type Ref = *const c_void;
        unsafe extern "C" {
            fn CFStringCreateWithBytes(allocator: Ref, bytes: *const u8, count: isize, encoding: u32, external: bool) -> Ref;
            fn CFNumberCreate(allocator: Ref, kind: isize, value: Ref) -> Ref;
            fn CFDictionaryCreate(allocator: Ref, keys: *const Ref, values: *const Ref, count: isize, key_callbacks: Ref, value_callbacks: Ref) -> Ref;
            fn CFRelease(value: Ref);
            fn CTFontDescriptorCreateWithAttributes(attributes: Ref) -> Ref;
            fn CTFontCreateWithFontDescriptor(descriptor: Ref, size: f64, matrix: Ref) -> *mut c_void;
            static kCTFontFamilyNameAttribute: Ref;
            static kCTFontTraitsAttribute: Ref;
            static kCTFontWeightTrait: Ref;
            static kCTFontSymbolicTrait: Ref;
        }
        let weight = crate::FontWeightTextExt::core_text_weight(weight);
        let symbolic: i32 = i32::from(italic);
        // SAFETY: all dictionaries reference live CF objects until the descriptor
        // has copied them; every create reference is released once below.
        unsafe {
            let family = CFStringCreateWithBytes(ptr::null(), name.as_ptr(), name.len() as isize, 0x08000100, false);
            let weight = CFNumberCreate(ptr::null(), 5, (&weight as *const f32).cast());
            let symbolic = CFNumberCreate(ptr::null(), 3, (&symbolic as *const i32).cast());
            let keys = [kCTFontWeightTrait, kCTFontSymbolicTrait];
            let values = [weight, symbolic];
            let traits = CFDictionaryCreate(ptr::null(), keys.as_ptr(), values.as_ptr(), 2, ptr::null(), ptr::null());
            let keys = [kCTFontFamilyNameAttribute, kCTFontTraitsAttribute];
            let values = [family, traits];
            let attributes = CFDictionaryCreate(ptr::null(), keys.as_ptr(), values.as_ptr(), 2, ptr::null(), ptr::null());
            let descriptor = CTFontDescriptorCreateWithAttributes(attributes);
            let font = if descriptor.is_null() { ptr::null_mut() } else { CTFontCreateWithFontDescriptor(descriptor, size as f64, ptr::null()) };
            for value in [descriptor, attributes, traits, symbolic, weight, family] {
                if !value.is_null() { CFRelease(value); }
            }
            Self::owned(font)
        }
    }
    fn owned(value: *mut c_void) -> Option<Self> {
        Some(Self(Rc::new(AppleFontData(NonNull::new(value)?))))
    }
    fn pointer(&self) -> *mut c_void {
        self.0.0.as_ptr()
    }
    pub fn resized(&self, size: f32) -> Option<Self> {
        // SAFETY: copies a live CTFont reference, returning a separate retained object.
        Self::owned(unsafe { native::cove_apple_font_copy(self.pointer(), size) })
    }
    pub fn name(&self, family: bool) -> String {
        // SAFETY: bridge returns a malloc-owned UTF-8 string; take_string copies and frees it.
        unsafe {
            native::take_string(native::cove_apple_font_name(
                self.pointer(),
                i32::from(family),
            ))
        }
        .unwrap_or_default()
    }
    pub fn path(&self) -> Option<PathBuf> {
        // SAFETY: bridge returns a malloc-owned UTF-8 path or null.
        unsafe { native::take_string(native::cove_apple_font_path(self.pointer())) }
            .map(PathBuf::from)
    }
    pub fn subfamily(&self) -> String {
        // SAFETY: kind 2 returns an owned UTF-8 subfamily name or null; take_string releases it.
        unsafe { native::take_string(native::cove_apple_font_name(self.pointer(), 2)) }
            .unwrap_or_default()
    }
    pub fn variations(&self) -> BTreeMap<FontAxisId, f64> {
        // SAFETY: null buffers request the immutable variation count without writing.
        let count = unsafe {
            native::cove_apple_font_variations(self.pointer(), ptr::null_mut(), ptr::null_mut(), 0)
        };
        let mut tags = vec![0; count];
        let mut values = vec![0.0; count];
        // SAFETY: both output arrays contain count writable entries, and the bridge respects capacity.
        let written = unsafe {
            native::cove_apple_font_variations(
                self.pointer(),
                tags.as_mut_ptr(),
                values.as_mut_ptr(),
                count,
            )
        }
        .min(count);
        tags.into_iter()
            .zip(values)
            .take(written)
            .map(|(tag, value)| (FontAxisId(tag), value))
            .collect()
    }
    pub fn fallback(&self, text: &str, utf16_location: usize, utf16_length: usize) -> Option<Self> {
        // SAFETY: live font and UTF-8 bytes; bridge validates the requested UTF-16 range and returns an owned CTFont.
        Self::owned(unsafe {
            native::cove_apple_font_fallback(
                self.pointer(),
                text.as_ptr(),
                text.len(),
                utf16_location,
                utf16_length,
            )
        })
    }
}
#[derive(Clone)]
pub struct NativeColorGlyphFont {
    font: AppleFont,
    pub size: f32,
    pub raster_scale: f32,
}
impl NativeColorGlyphFont {
    pub(crate) fn new(font: AppleFont, size: f32, raster_scale: f32) -> Option<Self> {
        Some(Self {
            font: font.resized(size)?,
            size,
            raster_scale,
        })
    }
    pub(crate) fn resized(&self, size: f32, raster_scale: f32) -> Option<Self> {
        Self::new(self.font.clone(), size, raster_scale)
    }
    pub fn source(&self) -> ColorGlyphSource {
        let metrics = self.clone();
        let lines = self.clone();
        let raster = self.clone();
        ColorGlyphSource {
            size: self.size,
            raster_scale: self.raster_scale,
            metrics: Rc::new(move |glyph| metrics.metrics(glyph)),
            line_metrics: Rc::new(move || lines.line_metrics()),
            rasterize: Rc::new(move |glyph| raster.rasterize(glyph)),
        }
    }
    pub fn metrics(&self, glyph: u32) -> Option<GlyphMetrics> {
        let mut metrics = native::Metrics::default();
        // SAFETY: live font reference, checked glyph range in C, and writable repr(C) metrics output.
        (unsafe {
            native::cove_apple_color_metrics(
                self.font.pointer(),
                self.raster_scale,
                glyph,
                &mut metrics,
            )
        } != 0)
            .then(|| metrics.into())
    }
    pub fn line_metrics(&self) -> GlyphLineMetrics {
        // SAFETY: read-only CoreText query returns copied metrics.
        unsafe { native::cove_apple_line_metrics(self.font.pointer()) }.into()
    }
    pub fn rasterize(&self, glyph: u32) -> Option<ColorGlyphBitmap> {
        // SAFETY: the returned bitmap owns its pixels until its guard is dropped.
        let result = unsafe {
            native::cove_apple_color_bitmap(self.font.pointer(), self.raster_scale, glyph)
        };
        Some(ColorGlyphBitmap {
            pixels: result.pixels(4)?,
            width: result.width as usize,
            height: result.height as usize,
            metrics: result.metrics.into(),
        })
    }
    pub fn shape(&self, text: &str, font_id: i64, utf8_offset: u32) -> Vec<ShapedGlyph> {
        // SAFETY: UTF-8 bytes/font remain live and the returned owned glyph array is copied before freeing.
        unsafe {
            native::cove_apple_shape(
                self.font.pointer(),
                text.as_ptr(),
                text.len(),
                utf8_offset,
                font_id,
            )
        }
        .into_glyphs()
    }
}
