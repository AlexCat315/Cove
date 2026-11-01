//! Translation of Theme/Styles/Buttons/GhostButtonStyle.swift.
use crate::AnyView;
use crate::theme::SemanticColorRef;

use super::builtin_button_chrome::{BuiltinButtonChrome, BuiltinButtonChromeKind};
use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// Background-less button. At rest only the label is drawn; hover/press fade
/// in a state-layer overlay. Used for tertiary actions and toolbar items.
/// Focus ring still appears on keyboard focus.
#[derive(Clone, Copy, Debug, Default)]
pub struct GhostButtonStyle;

impl ButtonStyle for GhostButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let foreground = if configuration.is_enabled {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_disabled()
        };
        AnyView::new(BuiltinButtonChrome::new(
            BuiltinButtonChromeKind::Ghost,
            &configuration,
            foreground,
        ))
    }
}
