//! Translation of Theme/Styles/Buttons/PrimaryButtonStyle.swift.
use crate::AnyView;
use crate::theme::SemanticColorRef;

use super::builtin_button_chrome::{BuiltinButtonChrome, BuiltinButtonChromeKind};
use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// Filled, accent-coloured button. Default style for `Button`.
///
/// Visual recipe (consumes the theme's accent ramp directly):
/// - Rest:     fill = `accent`,         border = transparent.
/// - Hover:    fill = `accent_hover`.
/// - Pressed:  fill = `accent_pressed`.
/// - Disabled: fill = `surface`, foreground = `on_surface_disabled`.
/// - Focused:  2px `focus_ring` border replaces the highlight ring.
///
/// No drop shadow at this layer — primary buttons live on top of `surface`
/// (Layer 1); they don't need to "lift" off it. Use `.shadow(...)` on the
/// owning button if a popover-style elevation is required.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrimaryButtonStyle;

impl ButtonStyle for PrimaryButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let foreground = if configuration.is_enabled {
            SemanticColorRef::on_accent()
        } else {
            SemanticColorRef::on_surface_disabled()
        };
        AnyView::new(BuiltinButtonChrome::new(
            BuiltinButtonChromeKind::Primary,
            &configuration,
            foreground,
        ))
    }
}
