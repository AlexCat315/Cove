//! Translation of Theme/Styles/Buttons/DestructiveButtonStyle.swift.
use crate::theme::SemanticColorRef;
use crate::AnyView;

use super::builtin_button_chrome::{BuiltinButtonChrome, BuiltinButtonChromeKind};
use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// Filled button using the `error` color slot. Used for irreversible actions
/// (delete, discard). Same chrome as `PrimaryButtonStyle` but keyed off the
/// `error` colour. State variants are produced by compositing the theme's
/// state-layer overlays so the destructive ramp tracks the rest of the system
/// without per-style colour math.
#[derive(Clone, Copy, Debug, Default)]
pub struct DestructiveButtonStyle;

impl ButtonStyle for DestructiveButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let foreground = if configuration.is_enabled {
            SemanticColorRef::on_accent()
        } else {
            SemanticColorRef::on_surface_muted()
        };
        AnyView::new(BuiltinButtonChrome::new(
            BuiltinButtonChromeKind::Destructive,
            &configuration,
            foreground,
        ))
    }
}
