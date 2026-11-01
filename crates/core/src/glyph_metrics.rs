//! Translation of CoveUICore/GlyphMetrics.swift.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GlyphAtlasFormat {
    #[default]
    Alpha,
    Color,
}
impl GlyphAtlasFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Alpha => 1,
            Self::Color => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphAtlasInfo {
    pub glyph_index: u32,
    pub width: f32,
    pub height: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
    pub uv_min_x: f32,
    pub uv_min_y: f32,
    pub uv_max_x: f32,
    pub uv_max_y: f32,
    pub format: GlyphAtlasFormat,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphMetrics {
    pub glyph_index: u32,
    pub width: f32,
    pub height: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphLineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}
pub trait GlyphMetricsProvider {
    fn glyph_metrics(&self, glyph_index: u32, font_id: i64) -> Option<GlyphMetrics>;
    fn line_metrics(&self, font_id: i64) -> Option<GlyphLineMetrics>;
}
