//! Translation of Theme/Styles/Buttons/SecondaryButtonStyle.swift.
use crate::theme::SemanticColorRef;
use crate::AnyView;

use super::builtin_button_chrome::{BuiltinButtonChrome, BuiltinButtonChromeKind};
use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// Tonal / surface-variant button. Lower visual weight than primary.
/// Reads from the theme's state-layer ramp for hover/press so palette changes
/// don't require recomputing `lighter`/`darker` mixes here.
#[derive(Clone, Copy, Debug, Default)]
pub struct SecondaryButtonStyle;

impl ButtonStyle for SecondaryButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let foreground = if configuration.is_enabled {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_disabled()
        };
        AnyView::new(BuiltinButtonChrome::new(
            BuiltinButtonChromeKind::Secondary,
            &configuration,
            foreground,
        ))
    }
}
