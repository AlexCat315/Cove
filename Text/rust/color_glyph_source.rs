//! Translation of CoveUIText/ColorGlyphSource.swift.
use cove_core::{GlyphLineMetrics, GlyphMetrics};
use std::rc::Rc;
#[derive(Clone)]
pub struct ColorGlyphSource { pub size: f32, pub raster_scale: f32, pub metrics: Rc<dyn Fn(u32) -> Option<GlyphMetrics>>, pub line_metrics: Rc<dyn Fn() -> GlyphLineMetrics>, pub rasterize: Rc<dyn Fn(u32) -> Option<ColorGlyphBitmap>> }
#[derive(Clone, Debug)]
pub struct ColorGlyphBitmap { pub pixels: Vec<u8>, pub width: usize, pub height: usize, pub metrics: GlyphMetrics }
