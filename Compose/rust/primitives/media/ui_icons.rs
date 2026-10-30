//! Translation of Primitives/Media/UIIconCatalog.swift.
//!
//! The bundled icon set facade. Every asset is authored on a 24x24 grid and
//! drawn through the alpha-mask rendering mode, so the alpha channel becomes
//! coverage and the foreground colour becomes the fill. Icons therefore follow
//! theme tokens instead of their authored colours, and one asset serves every
//! density.
//!
//! Glyphs that are routinely drawn at 8-14 px (`chevron-*`, `check`, `x`,
//! `plus`, `minus`) are authored with a heavier stroke so they stay legible when
//! scaled down; everything else uses the 2.0 default.
//!
//! The Rust port keeps the shared catalog helpers in `ui_icon_catalog`; this
//! facade exposes them under the reference name and grouped lookups.

use cove_imaging::BundleImageResource;

use super::ui_icon_catalog::{
    all, asset, categories, search, UIIconEntry,
};

/// Name lookup across the whole catalog, independent of grouping.
pub fn icon(named: &str) -> Option<BundleImageResource> {
    all()
        .into_iter()
        .find(|entry| entry.id == named || entry.name == named)
        .map(|entry| entry.resource)
}

/// The same grouped spellings the Swift catalog publishes.
pub struct UIIcons;

impl UIIcons {
    /// Resolve a bundled icon by asset slug.
    pub fn asset(name: impl Into<String>) -> BundleImageResource {
        asset(name)
    }

    pub fn named(name: &str) -> Option<BundleImageResource> {
        icon(name)
    }

    pub fn all() -> Vec<UIIconEntry> {
        all()
    }

    pub fn search(query: &str) -> Vec<UIIconEntry> {
        search(query)
    }

    pub fn categories() -> Vec<String> {
        categories()
    }

    /// Each group's `UIIconEntry` projection, in catalog order.
    pub fn entries(category: &str) -> Vec<UIIconEntry> {
        all()
            .into_iter()
            .filter(|entry| entry.category.eq_ignore_ascii_case(category))
            .collect()
    }
}

/// Navigation glyphs: arrows, chevrons, expand and window controls.
pub mod navigation {
    pub use super::super::ui_icon_catalog::navigation::*;
}

/// Action glyphs: check, copy, search, settings, zoom and edit verbs.
pub mod action {
    pub use super::super::ui_icon_catalog::action::*;
}

/// Status glyphs: alerts, badges, visibility and lifecycle.
pub mod status {
    pub use super::super::ui_icon_catalog::status::*;
}

/// File and folder glyphs.
pub mod file {
    pub use super::super::ui_icon_catalog::file::*;
}

/// Editor and tooling glyphs.
pub mod editor {
    pub use super::super::ui_icon_catalog::editor::*;
}

/// Layout glyphs: columns, panels, grids and rows.
pub mod layout {
    pub use super::super::ui_icon_catalog::layout::*;
}

/// Data glyphs: charts, calendar and time.
pub mod data {
    pub use super::super::ui_icon_catalog::data::*;
}

/// Media glyphs: camera, image, audio and video.
pub mod media {
    pub use super::super::ui_icon_catalog::media::*;
}

/// Device and connectivity glyphs.
pub mod device {
    pub use super::super::ui_icon_catalog::device::*;
}

/// Person and group glyphs.
pub mod user {
    pub use super::super::ui_icon_catalog::user::*;
}

/// Text-formatting glyphs.
pub mod text {
    pub use super::super::ui_icon_catalog::text::*;
}

/// Shape and object glyphs.
pub mod shape {
    pub use super::super::ui_icon_catalog::shape::*;
}
