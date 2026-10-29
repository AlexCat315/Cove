//! Translation of Theme/Tokens/Typography.swift.
use cove_core::{Font, FontDesign};

/// One typography slot — pairs a `Font` with the line-height and letter
/// spacing that should travel with it.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleToken {
    pub font: Font,
    pub line_height: f32,
    pub letter_spacing: f32,
}
impl TextStyleToken {
    pub fn new(font: Font, line_height: f32, letter_spacing: f32) -> Self {
        Self { font, line_height, letter_spacing }
    }
    pub fn resolving_family(&self, family: Option<&str>) -> Self {
        let mut result = self.clone();
        result.font = resolving_font_family(&self.font, family);
        result
    }
    /// Line height for this slot, rescaled to `font_override` when a modifier
    /// substituted a different font size.
    pub fn line_height_for(&self, font_override: Option<&Font>) -> f32 {
        let size = font_override.map(Font::size).unwrap_or_else(|| self.font.size());
        self.line_height * size / self.font.size()
    }
}

fn resolving_font_family(font: &Font, family: Option<&str>) -> Font {
    let Some(family) = family else { return font.clone() };
    if font.family().is_some() || font.design() != FontDesign::Standard {
        return font.clone();
    }
    Font::named(family, font.size(), font.weight()).with_italic(font.is_italic())
}

/// Type scale slots covering display through caption plus a monospace slot
/// for consoles and code listings. Slot names describe role, not size.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    /// Applies to standard semantic roles unless a token names its own family.
    pub family: Option<String>,
    pub display: TextStyleToken,
    pub title: TextStyleToken,
    pub headline: TextStyleToken,
    pub body: TextStyleToken,
    pub body_strong: TextStyleToken,
    pub caption: TextStyleToken,
    pub label: TextStyleToken,
    pub mono: TextStyleToken,
}
impl Typography {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        family: Option<String>,
        display: TextStyleToken,
        title: TextStyleToken,
        headline: TextStyleToken,
        body: TextStyleToken,
        body_strong: TextStyleToken,
        caption: TextStyleToken,
        label: TextStyleToken,
        mono: TextStyleToken,
    ) -> Self {
        Self { family, display, title, headline, body, body_strong, caption, label, mono }
    }
    pub fn resolved_body(&self) -> TextStyleToken {
        self.body.resolving_family(self.family.as_deref())
    }
}
