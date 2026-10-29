//! Translation of Theme/Tokens/TextEmphasis.swift.
use cove_core::Color;
/// Foregrounds for text that communicates availability rather than content.
/// `None` inherits the palette's muted foreground.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextEmphasis {
    pub placeholder: Option<Color>,
    pub disabled: Option<Color>,
}
impl TextEmphasis {
    pub fn new(configure: impl FnOnce(&mut Self)) -> Self {
        let mut value = Self::default();
        configure(&mut value);
        value
    }
}
