//! Named theme presets, mirroring Theme/Styles/Default{Dark,Light}Theme.swift.
//!
//! Swift publishes each preset as a `public enum` holding one static `value`.
//! Rust has no type-member constants, so the same spellings become unit structs
//! with a `value()` associated function, alongside the already-exported
//! `Theme::default_dark()` / `Theme::default_light()` shorthands.

use crate::theme::Theme;

/// Default dark theme bundled with CoveUI.
///
/// Neutral-zinc palette matching the reference design: a zinc-900 window,
/// zinc-800 control faces, a single blue accent ramp (lighter on hover than at
/// rest, as the reference), and flat zinc borders.
pub struct DefaultDarkTheme;

impl DefaultDarkTheme {
    /// Value-stable across calls; safe to compare for identity within a frame.
    pub fn value() -> Theme {
        crate::theme::styles::default_dark_theme::value()
    }
}

/// Default light theme.
///
/// Neutral-zinc palette matching the reference design: a pure white window with
/// zinc control faces (the face of buttons, inputs, sidebars and chips), a
/// single blue accent ramp, and subtle borders that separate flat regions rather
/// than translucent state washes.
pub struct DefaultLightTheme;

impl DefaultLightTheme {
    /// Value-stable across calls; safe to compare for identity within a frame.
    pub fn value() -> Theme {
        crate::theme::styles::default_light_theme::value()
    }
}
