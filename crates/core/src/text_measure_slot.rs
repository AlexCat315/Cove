//! Translation of CoveUICore/TextMeasureSlot.swift.
use crate::{Font, TextAlignment, TextLayoutResult};
#[derive(Clone, Debug, PartialEq)]
pub struct TextMeasureInputs {
    pub text: String,
    pub alignment: TextAlignment,
    pub line_limit: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct TextLayoutCacheKey {
    pub text: String,
    pub font: Font,
    pub line_height: f32,
    pub alignment: TextAlignment,
    pub max_width: f32,
    pub letter_spacing: f32,
    pub atlas_id: u64,
}
fn float_key(value: f32) -> u32 {
    if value == 0.0 { 0 } else { value.to_bits() }
}
impl PartialEq for TextLayoutCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font == other.font
            && self.alignment == other.alignment
            && self.atlas_id == other.atlas_id
            && float_key(self.line_height) == float_key(other.line_height)
            && float_key(self.max_width) == float_key(other.max_width)
            && float_key(self.letter_spacing) == float_key(other.letter_spacing)
    }
}
impl Eq for TextLayoutCacheKey {}
impl std::hash::Hash for TextLayoutCacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font.hash(state);
        self.alignment.hash(state);
        self.atlas_id.hash(state);
        float_key(self.line_height).hash(state);
        float_key(self.max_width).hash(state);
        float_key(self.letter_spacing).hash(state);
    }
}
#[derive(Clone, Debug)]
pub struct TextLayoutCacheEntry {
    pub key: TextLayoutCacheKey,
    pub result: TextLayoutResult,
}
