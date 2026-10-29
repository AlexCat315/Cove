//! Translation of Theme/Styles/Buttons/ToggleButtonStyle.swift.
use crate::theme::SemanticColorRef;
use crate::AnyView;

use super::builtin_button_chrome::{BuiltinButtonChrome, BuiltinButtonChromeKind};
use super::button_style::{ButtonStyle, ButtonStyleConfiguration};

/// On/off chip for toolbars, segmented choices, and view-mode switches.
/// Reads `is_selected` (set via `Button(isSelected:)`): solid accent with
/// `on_accent` foreground when on — the theme guarantees that pairing's
/// contrast — transparent with state-layer washes when off.
///
/// ```ignore
/// Button::new(/* is_selected: */ mode == Mode::Grid, /* action: */ move || mode.set(Mode::Grid))
///     .button_style(toggle())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToggleButtonStyle {
    pub min_width: f32,
    pub height: f32,
}
impl ToggleButtonStyle {
    pub fn new(min_width: f32, height: f32) -> Self {
        Self { min_width, height }
    }
}
impl Default for ToggleButtonStyle {
    fn default() -> Self {
        Self::new(28.0, 26.0)
    }
}

impl ButtonStyle for ToggleButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        // Toolbar toggles often contain icon-only labels. Keep their resting
        // state at primary text contrast so white SVG masks remain legible on
        // light surfaces; selected controls use the paired on_accent token.
        let foreground = if configuration.is_selected {
            SemanticColorRef::on_accent()
        } else {
            SemanticColorRef::on_surface()
        };
        AnyView::new(BuiltinButtonChrome::new(
            BuiltinButtonChromeKind::Toggle { min_width: self.min_width, height: self.height },
            &configuration,
            foreground,
        ))
    }

    /// Swift's `Hashable` conformance folds both authored metrics into the
    /// style identity.
    fn style_identity(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for bits in [self.min_width.to_bits(), self.height.to_bits()] {
            hash ^= bits as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        hash
    }
}
