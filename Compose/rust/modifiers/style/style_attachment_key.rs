//! Translation of the `StyleAttachmentKey` constants in
//! Modifiers/Style/StyleModifiers.swift.
pub mod style_attachment_key {
    pub const FONT: &str = "__font";
    pub const LINE_HEIGHT: &str = "__line_height";
    pub const LETTER_SPACING: &str = "__letter_spacing";
    pub const TEXT_STYLE_SCALER: &str = "__text_scaler";
}
pub use style_attachment_key::*;
