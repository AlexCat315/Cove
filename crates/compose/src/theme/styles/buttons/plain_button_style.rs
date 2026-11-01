//! Translation of Theme/Styles/Buttons/PlainButtonStyle.swift.
use crate::AnyView;

use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// Zero-chrome button style. Returns the label exactly as given without any
/// padding, background, or focus ring. Useful for tappable affordances that
/// must blend into custom container chrome (tree disclosures, list rows that
/// own their own selection fill, icon-only toolbar buttons drawn as glyphs).
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainButtonStyle;

impl ButtonStyle for PlainButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        configuration.label
    }
}
