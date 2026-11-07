//! Translation of Primitives/Feedback/StatusTone.swift.
use crate::theme::{SemanticColorRef, Theme};
use cove_core::Color;
use cove_imaging::BundleImageResource;

/// Semantic intent shared by feedback components; callers select meaning and
/// let the active theme derive its color and icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatusTone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

impl StatusTone {
    pub const ALL: [Self; 5] = [
        Self::Neutral,
        Self::Info,
        Self::Success,
        Self::Warning,
        Self::Danger,
    ];

    pub fn color(self) -> SemanticColorRef {
        match self {
            Self::Neutral => SemanticColorRef::on_surface_variant(),
            Self::Info => SemanticColorRef::info(),
            Self::Success => SemanticColorRef::success(),
            Self::Warning => SemanticColorRef::warning(),
            Self::Danger => SemanticColorRef::error(),
        }
    }

    pub fn icon(self) -> BundleImageResource {
        use crate::primitives::media::ui_icon_catalog::{shape, status};
        match self {
            Self::Neutral => shape::circle(),
            Self::Info => status::info(),
            Self::Success => status::check_circle(),
            Self::Warning => status::alert_triangle(),
            Self::Danger => status::x_circle(),
        }
    }

    /// Solid glyph with the mark knocked out of the shape, the way SF Symbols
    /// draws its filled status variants. Reads as a single mark at small sizes.
    pub fn filled_icon(self) -> BundleImageResource {
        use crate::primitives::media::ui_icon_catalog::asset;
        match self {
            Self::Neutral => asset("circle-fill"),
            Self::Info => asset("info-circle-fill"),
            Self::Success => asset("check-circle-fill"),
            Self::Warning => asset("alert-triangle-fill"),
            Self::Danger => asset("x-circle-fill"),
        }
    }

    pub fn ink_on_solid(self, theme: &Theme) -> Color {
        let color = self.color().resolve(theme);
        let luminance = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
        if luminance > 0.62 {
            Color::from_rgba8(26, 28, 33, 255)
        } else {
            Color::WHITE
        }
    }
}
