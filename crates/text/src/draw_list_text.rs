//! Translation of CoveUIText/DrawList+Text.swift.
use crate::FontAtlas;
use cove_core::{
    Color, DrawError, DrawList, GlyphAtlasFormat, Point, PositionedGlyph, TextLayoutResult,
    TextureId, content_scale,
};

/// Maps a positioned glyph to a per-glyph colour override.
pub type GlyphColorSource<'a> = &'a dyn Fn(&PositionedGlyph) -> Option<Color>;

pub trait DrawListText {
    #[allow(
        clippy::too_many_arguments,
        reason = "translated text emitter separates layout, placement, tint and atlas ownership"
    )]
    fn add_text(
        &mut self,
        layout: &TextLayoutResult,
        origin: Point,
        color: Color,
        texture_id: TextureId,
        atlas: Option<&FontAtlas>,
        color_for_glyph: Option<GlyphColorSource<'_>>,
    ) -> Result<(), DrawError>;
}
impl DrawListText for DrawList {
    fn add_text(
        &mut self,
        layout: &TextLayoutResult,
        origin: Point,
        color: Color,
        texture_id: TextureId,
        atlas: Option<&FontAtlas>,
        color_for_glyph: Option<GlyphColorSource<'_>>,
    ) -> Result<(), DrawError> {
        add_text_at_scale(
            self,
            layout,
            origin,
            color,
            texture_id,
            atlas,
            color_for_glyph,
            content_scale(),
        )
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared text emitter also accepts the collection's explicit raster scale"
)]
pub(crate) fn add_text_at_scale(
    list: &mut DrawList,
    layout: &TextLayoutResult,
    origin: Point,
    color: Color,
    texture_id: TextureId,
    atlas: Option<&FontAtlas>,
    color_for_glyph: Option<GlyphColorSource<'_>>,
    scale: f32,
) -> Result<(), DrawError> {
    let snap = |value: f32| (value * scale).round() / scale;
    for glyph in layout.lines.iter().flat_map(|line| &line.glyphs) {
        let x = origin.x + glyph.x;
        let y = origin.y + glyph.y;
        // Split the quantized position at the pixel boundary, including for
        // negative origins. FreeType applies the fractional part to the ink.
        let quarters = (x * scale * 4.0).round();
        let pixel_x = (quarters * 0.25).floor();
        let phase = (quarters - pixel_x * 4.0) as u8;
        let info = glyph.atlas_info.or_else(|| {
            atlas.and_then(|atlas| atlas.rasterize_glyph_at(glyph.glyph_id, glyph.font_id, phase))
        });
        let Some(info) = info.filter(|info| info.width > 0.0 && info.height > 0.0) else {
            continue;
        };
        let position = if glyph.atlas_info.is_none() && info.format == GlyphAtlasFormat::Alpha {
            Point {
                x: pixel_x / scale + info.bearing_x,
                // Snap the baseline once before applying pixel bearings.
                y: snap(y) - info.bearing_y,
            }
        } else {
            Point {
                x: snap(x + info.bearing_x),
                y: snap(y - info.bearing_y),
            }
        };
        list.add_atlas_glyph(
            info,
            position,
            color_for_glyph
                .and_then(|resolve| resolve(glyph))
                .unwrap_or(color),
            texture_id,
        )?;
    }
    Ok(())
}
