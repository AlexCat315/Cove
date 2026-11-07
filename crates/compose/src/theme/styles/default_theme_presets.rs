//! Named theme presets, mirroring Theme/Styles/Default{Dark,Light}Theme.swift.
//!
//! Swift publishes each preset as a `public enum` holding one static `value`.
//! Rust has no type-member constants, so the same spellings become unit structs
//! with a `value()` associated function, alongside the already-exported
//! `Theme::default_dark()` / `Theme::default_light()` shorthands.

use crate::theme::Theme;

/// Default dark theme bundled with CoveUI.
///
/// macOS dark palette: graphite content, a lighter sidebar and groups, a single
/// blue accent ramp and spring-driven motion.
pub struct DefaultDarkTheme;

impl DefaultDarkTheme {
    /// Value-stable across calls; safe to compare for identity within a frame.
    pub fn value() -> Theme {
        crate::theme::styles::default_dark_theme::value()
    }
}

/// Default light theme.
///
/// macOS light palette: white content, a light-gray sidebar and groups, a
/// single blue accent ramp and spring-driven motion.
pub struct DefaultLightTheme;

impl DefaultLightTheme {
    /// Value-stable across calls; safe to compare for identity within a frame.
    pub fn value() -> Theme {
        crate::theme::styles::default_light_theme::value()
    }
}
