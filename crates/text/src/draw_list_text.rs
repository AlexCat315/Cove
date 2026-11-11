//! Translation of CoveUIText/DrawList+Text.swift.
use crate::FontAtlas;
use cove_core::{
    Color, DrawError, DrawList, Point, PositionedGlyph, TextLayoutResult, TextureId, content_scale,
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
        let scale = content_scale();
        let snap = |value: f32| {
            if scale.is_finite() && scale > 0.0 {
                (value * scale).round() / scale
            } else {
                value.round()
            }
        };
        for line in &layout.lines {
            // Advances are fractional — a 21px face steps by something like
            // 8.4px — while the ink is snapped to whole pixels. Rounding each
            // glyph's position on its own then leaves the gaps between letters
            // flipping between one and two pixels as that fraction drifts, and
            // the line reads as unevenly tracked. Accumulating the pen in
            // device pixels keeps every advance an integer, so a given pair of
            // letters always sits the same distance apart.
            let mut pen_device = None::<f32>;
            let mut previous_x = 0.0_f32;
            for glyph in &line.glyphs {
                let advance = glyph.x - previous_x;
                previous_x = glyph.x;
                let device_x = match pen_device {
                    Some(previous) if scale.is_finite() && scale > 0.0 => {
                        previous + (advance * scale).round()
                    }
                    _ => (origin.x + glyph.x) * scale,
                };
                pen_device = Some(device_x);
                let info = glyph.atlas_info.or_else(|| {
                    atlas.and_then(|atlas| atlas.rasterize_glyph(glyph.glyph_id, glyph.font_id))
                });
                let Some(info) = info.filter(|info| info.width > 0.0 && info.height > 0.0) else {
                    continue;
                };
                let pen_x = if scale.is_finite() && scale > 0.0 {
                    device_x / scale
                } else {
                    origin.x + glyph.x
                };
                self.add_atlas_glyph(
                    info,
                    Point {
                        x: snap(pen_x + info.bearing_x),
                        y: snap(origin.y + glyph.y - info.bearing_y),
                    },
                    color_for_glyph
                        .and_then(|resolve| resolve(glyph))
                        .unwrap_or(color),
                    texture_id,
                )?;
            }
        }
        Ok(())
    }
}
