//! Translation of Theme/Styles/AppearanceModifier.swift.
use crate::theme::Theme;
use crate::{AnyView, View};

use super::circular_appearance::{CircularThemeHost, ThemeTransition};

/// Coarse light / dark switch. Set via `.appearance(_:)` to swap the active
/// theme for an entire subtree without spelling out a custom `Theme`.
///
/// (The token slot bag in this codebase is already named `ColorScheme`, so the
/// SwiftUI-flavoured `ColorScheme` enum is spelled `Appearance` here to avoid
/// the collision.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Appearance {
    Dark,
    Light,
}
impl Appearance {
    /// The built-in preset this appearance selects.
    pub fn theme(self) -> Theme {
        match self {
            Self::Dark => Theme::default_dark(),
            Self::Light => Theme::default_light(),
        }
    }
}

pub trait AppearanceViewExt: View + Sized + 'static {
    /// Apply one of the built-in `Theme::default_dark` / `default_light`
    /// presets to the receiver subtree. Equivalent to `.theme(Theme::default_light)`
    /// for `Light`, etc. Composes naturally with `.theme(_)` — whichever
    /// modifier appears last (innermost) wins for descendants.
    fn appearance(self, appearance: Appearance, transition: ThemeTransition) -> CircularThemeHost<Appearance> {
        CircularThemeHost::new(appearance.theme(), appearance, transition, AnyView::new(self))
    }
}
impl<T: View + Sized + 'static> AppearanceViewExt for T {}

// The Rust `theme(_:transition:value:)` counterpart is expressed directly with
// `CircularThemeHost::new(theme, value, transition, content)`; Swift's generic
// `theme(_:transition:value:)` extension collapses to that initializer because
// Rust has no trailing generic argument inference on extension methods.
