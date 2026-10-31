//! Translation of CoveUIText/TextShaper.swift.
use crate::{FontFace, Script, ScriptTag};
use cove_core::ShapedGlyph;
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDirection {
    Ltr = 4,
    Rtl = 5,
    Ttb = 6,
    Btt = 7,
}
#[derive(Clone, Default)]
pub struct TextShaper {
    face: Option<FontFace>,
}
impl TextShaper {
    pub fn set_face(&mut self, face: FontFace, _size: f32) {
        self.face = Some(face);
    }
    pub fn shape(
        &self,
        text: &str,
        direction: Option<TextDirection>,
        script: Option<ScriptTag>,
        language: Option<&str>,
    ) -> Vec<ShapedGlyph> {
        self.face.as_ref().map_or_else(Vec::new, |face| {
            face.shape(text, direction, script, language, 0, 0)
        })
    }
    pub fn shape_latin(&self, text: &str) -> Vec<ShapedGlyph> {
        self.shape(
            text,
            Some(TextDirection::Ltr),
            Some(Script::LATIN),
            Some("en"),
        )
    }
}
