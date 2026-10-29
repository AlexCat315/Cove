//! Translation of Theme/Tokens/SemanticColor.swift.
use cove_core::Color;
use std::rc::Rc;
use std::sync::Arc;
use super::Theme;

/// Late-bound color reference resolved against the active `Theme`.
#[derive(Clone)]
pub struct SemanticColorRef {
    resolve: Arc<dyn Fn(&Theme) -> Color>,
}
impl SemanticColorRef {
    pub fn new(resolve: impl Fn(&Theme) -> Color + 'static) -> Self {
        Self { resolve: Arc::new(resolve) }
    }
    pub fn resolve(&self, theme: &Theme) -> Color {
        (self.resolve)(theme)
    }
    /// A translucent variant of this token — e.g. `.warning.opacity(0.16)` for
    /// a tinted icon chip.
    pub fn opacity(&self, opacity: f32) -> Self {
        let base = self.clone();
        Self::new(move |theme| base.resolve(theme).multiplied_alpha(opacity))
    }
    pub fn clear() -> Self { Self::new(|_| Color::CLEAR) }
    pub fn background() -> Self { Self::new(|theme| theme.colors.background) }
    pub fn surface() -> Self { Self::new(|theme| theme.colors.surface) }
    pub fn surface_variant() -> Self { Self::new(|theme| theme.colors.surface_variant) }
    pub fn surface_sunken() -> Self { Self::new(|theme| theme.colors.surface_sunken) }
    pub fn surface_raised() -> Self { Self::new(|theme| theme.colors.surface_raised) }
    pub fn surface_floating() -> Self { Self::new(|theme| theme.colors.surface_floating) }
    pub fn surface_overlay() -> Self { Self::new(|theme| theme.colors.surface_overlay) }

    pub fn on_background() -> Self { Self::new(|theme| theme.colors.on_background) }
    pub fn on_surface() -> Self { Self::new(|theme| theme.colors.on_surface) }
    pub fn on_surface_variant() -> Self { Self::new(|theme| theme.colors.on_surface_variant) }
    pub fn on_surface_muted() -> Self { Self::new(|theme| theme.colors.on_surface_muted) }
    pub fn on_surface_disabled() -> Self {
        Self::new(|theme| theme.text_emphasis.disabled.unwrap_or(theme.colors.on_surface_muted))
    }
    pub fn placeholder() -> Self {
        Self::new(|theme| theme.text_emphasis.placeholder.unwrap_or(theme.colors.on_surface_muted))
    }

    pub fn accent() -> Self { Self::new(|theme| theme.colors.accent) }
    pub fn accent_hover() -> Self { Self::new(|theme| theme.colors.accent_hover) }
    pub fn accent_pressed() -> Self { Self::new(|theme| theme.colors.accent_pressed) }
    pub fn on_accent() -> Self { Self::new(|theme| theme.colors.on_accent) }
    pub fn accent_muted() -> Self { Self::new(|theme| theme.colors.accent_muted) }
    pub fn accent_secondary() -> Self { Self::new(|theme| theme.colors.accent_secondary) }

    pub fn state_layer_hover() -> Self { Self::new(|theme| theme.colors.state_layer_hover) }
    pub fn state_layer_pressed() -> Self { Self::new(|theme| theme.colors.state_layer_pressed) }
    pub fn state_layer_selected() -> Self { Self::new(|theme| theme.colors.state_layer_selected) }

    pub fn success() -> Self { Self::new(|theme| theme.colors.success) }
    pub fn warning() -> Self { Self::new(|theme| theme.colors.warning) }
    pub fn error() -> Self { Self::new(|theme| theme.colors.error) }
    pub fn info() -> Self { Self::new(|theme| theme.colors.info) }

    pub fn border() -> Self { Self::new(|theme| theme.colors.border) }
    pub fn border_strong() -> Self { Self::new(|theme| theme.colors.border_strong) }
    pub fn divider() -> Self { Self::new(|theme| theme.colors.divider) }
    pub fn focus_ring() -> Self { Self::new(|theme| theme.colors.focus_ring) }
    pub fn selection() -> Self { Self::new(|theme| theme.colors.selection) }
    pub fn overlay() -> Self { Self::new(|theme| theme.colors.overlay) }
}

/// Late-bound text-style reference resolved against the active `Typography`.
#[derive(Clone)]
pub struct SemanticFontRef {
    resolve: Arc<dyn Fn(&Theme) -> super::TextStyleToken>,
}
impl SemanticFontRef {
    pub fn new(resolve: impl Fn(&Theme) -> super::TextStyleToken + 'static) -> Self {
        Self { resolve: Arc::new(resolve) }
    }
    pub fn resolve(&self, theme: &Theme) -> super::TextStyleToken {
        (self.resolve)(theme)
    }
    pub fn display() -> Self { Self::new(|theme| theme.typography.display.clone()) }
    pub fn title() -> Self { Self::new(|theme| theme.typography.title.clone()) }
    pub fn headline() -> Self { Self::new(|theme| theme.typography.headline.clone()) }
    pub fn body() -> Self { Self::new(|theme| theme.typography.body.clone()) }
    pub fn body_strong() -> Self { Self::new(|theme| theme.typography.body_strong.clone()) }
    pub fn caption() -> Self { Self::new(|theme| theme.typography.caption.clone()) }
    pub fn label() -> Self { Self::new(|theme| theme.typography.label.clone()) }
    pub fn mono() -> Self { Self::new(|theme| theme.typography.mono.clone()) }
}

/// Compatibility alias for call sites that store the reference behind an `Rc`.
pub type SharedSemanticColorRef = Rc<SemanticColorRef>;
