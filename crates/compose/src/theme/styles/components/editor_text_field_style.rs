//! Translation of Theme/Styles/Components/EditorTextFieldStyle.swift.
//!
//! A flat document surface for text/code editors whose caller owns the chrome.
//! Focus stays visible through the caret, selection and active code line.
use crate::AnyView;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::theme::SemanticColorRef;

use super::text_field_style::{TextFieldStyle, TextFieldStyleConfiguration};

#[derive(Clone, Copy, Debug, Default)]
pub struct EditorTextFieldStyle;

impl TextFieldStyle for EditorTextFieldStyle {
    fn make_body(&self, configuration: TextFieldStyleConfiguration) -> AnyView {
        let alpha =
            if configuration.is_enabled || configuration.theme.text_emphasis.disabled.is_some() {
                1.0
            } else {
                0.55
            };
        AnyView::new(
            configuration
                .content
                .clone()
                .semantic_background(SemanticColorRef::surface_sunken())
                .opacity(alpha),
        )
    }
}
