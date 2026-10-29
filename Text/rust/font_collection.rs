//! Translation of CoveUIText/FontCollection.swift.
use crate::{font_face::scalar_script, FontAtlas, FontFace, FontFaceSelector, Script, ScriptTag};
use cove_core::{Color, DrawError, DrawList, Point, ShapedGlyph, TextLayoutOptions, TextureId, layout_text};
use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_segmentation::UnicodeSegmentation;
pub struct FontCollection { pub atlas: FontAtlas, faces: Vec<FontFace>, pub size: f32, pub raster_scale: f32 }
impl FontCollection {
    pub fn new(width: usize, height: usize) -> Self { let atlas = FontAtlas::new(width, height); atlas.reset(); Self { atlas, faces: vec![], size: 16.0, raster_scale: 1.0 } }
    pub fn font_count(&self) -> usize { self.faces.len() }
    pub fn append(&mut self, bytes: Vec<u8>) -> Option<i64> {
        let face = FontFace::from_bytes(bytes, self.size, self.raster_scale, FontFaceSelector::default(), Default::default()).ok()?;
        self.faces.push(face.clone()); let id = self.faces.len() as i64; self.atlas.register_face(face, id); Some(id)
    }
    pub fn configure(&mut self, size: f32, raster_scale: f32) {
        if !size.is_finite() || size <= 0.0 || size > 512.0 || !raster_scale.is_finite() || !(1.0..=4.0).contains(&raster_scale) { return; }
        self.size = size; self.raster_scale = raster_scale;
        for (index, face) in self.faces.iter().enumerate() { if face.applying(size, raster_scale) { self.atlas.register_face(face.clone(), index as i64 + 1); } }
    }
    pub fn shape(&self, text: &str) -> Vec<ShapedGlyph> {
        if self.faces.is_empty() { return vec![]; }
        struct Run { text: String, font: usize, script: ScriptTag, offset: u32 }
        let mut runs: Vec<Run> = Vec::new();
        for (offset, cluster) in text.grapheme_indices(true) {
            let Ok(offset) = u32::try_from(offset) else { return vec![]; };
            let scalars: Vec<_> = cluster.chars().filter(|c| get_general_category(*c) != GeneralCategory::Format
                && !matches!(*c as u32, 0xfe00..=0xfe0f | 0xe0100..=0xe01ef)).collect();
            let strong = scalars.iter().copied().map(scalar_script).find(|s| !matches!(*s, Script::COMMON | Script::INHERITED | Script::UNKNOWN));
            let script = strong.or_else(|| runs.last().map(|r| r.script)).unwrap_or(Script::COMMON);
            let covers = |index: usize| scalars.iter().all(|&c| self.faces[index].glyph_index(c) != 0);
            let previous = runs.last().map(|r| r.font);
            let font = if strong.is_none() && previous.is_some_and(covers) { previous.unwrap() } else { (0..self.faces.len()).find(|&i| covers(i)).unwrap_or(0) };
            if let Some(last) = runs.last_mut().filter(|r| r.font == font && r.script == script) { last.text.push_str(cluster); }
            else { runs.push(Run { text: cluster.into(), font, script, offset }); }
        }
        runs.iter().flat_map(|run| self.faces[run.font].shape(&run.text, None, Some(run.script), None, run.offset, run.font as i64 + 1)).collect()
    }
    #[allow(clippy::too_many_arguments, reason = "shared font drawing contract carries size, target and paint inputs")]
    pub fn draw(&mut self, text: &str, size: f32, raster_scale: f32, list: &mut DrawList, origin: Point, color: Color, texture_id: TextureId) -> Result<f32, DrawError> {
        if !size.is_finite() || size <= 0.0 || size > 512.0 || !raster_scale.is_finite() || !(1.0..=4.0).contains(&raster_scale) || !origin.x.is_finite() || !origin.y.is_finite() { return Ok(0.0); }
        self.configure(size, raster_scale); let glyphs = self.shape(text); let layout = layout_text(&glyphs, text, &self.atlas, TextLayoutOptions { line_height: size * 1.4, ..Default::default() });
        for line in &layout.lines { for glyph in &line.glyphs {
            let Some(info) = self.atlas.rasterize_glyph(glyph.glyph_id, glyph.font_id).filter(|i| i.width > 0.0 && i.height > 0.0) else { continue; };
            let snap = |value: f32| (value * self.raster_scale).round() / self.raster_scale;
            list.add_atlas_glyph(info, Point { x: snap(origin.x + glyph.x + info.bearing_x), y: snap(origin.y + glyph.y - info.bearing_y) }, color, texture_id)?;
        } } Ok(layout.total_width)
    }
}
impl Default for FontCollection { fn default() -> Self { Self::new(2048, 2048) } }
