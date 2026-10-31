//! Translation of Android/CoveAndroidApp/Validation/Showcase.swift.
//!
//! Swift compiled this file only into the showcase APK and reached the counter
//! and gallery products directly. The host-side responsibilities are kept here —
//! launch-mode selection, checkpoint restore semantics, page reporting and the
//! typography painter hook — with the products supplied through
//! [`ShowcaseProducts`] so `cove-hosts` carries no dependency on the example
//! crates.

use crate::android::contract::launch::{SceneLaunch, SceneLaunchMode};
use crate::android::sources::cove_android_app::content::TypographyDriver;
use cove_composition::AnyView;
use cove_core::{DrawList, Point};
use std::collections::BTreeMap;

/// The two validation products, supplied by the showcase build.
pub trait ShowcaseProducts {
    /// The counter product and the checkpoint keys it persists.
    fn counter(&mut self) -> Option<&mut dyn CounterProduct>;
    /// The gallery product, or `None` when this build omits it.
    fn gallery(&mut self) -> Option<&mut dyn GalleryProduct>;
    /// Typography driver for the counter, when the platform has system faces.
    fn typography_mut(&mut self) -> Option<&mut dyn TypographyDriver>;
}

/// A checkpoint-restorable counter surface.
pub trait CounterProduct {
    /// Restores authored values; `false` means the document was corrupt.
    fn restore(&mut self, values: &BTreeMap<String, String>) -> bool;
    fn checkpoint(&self) -> BTreeMap<String, String>;
    /// Root view with the caller's text painter installed, when the platform
    /// supplies one.
    fn root(&mut self, scale: f32, painter: Option<Box<dyn CounterTextPainter>>) -> AnyView;
}

/// A gallery surface with a named page catalogue.
pub trait GalleryProduct {
    /// Resolves a page by authored name; `None` leaves the current page.
    fn page_named(&self, name: &str) -> Option<String>;
    fn select_page(&mut self, page: &str);
    fn current_page(&self) -> String;
    fn root(&mut self) -> AnyView;
}

/// Paints one counter text run into the shared draw list.
pub trait CounterTextPainter {
    fn paint(&mut self, text: &str, list: &mut DrawList, origin: Point, scale: f32);
}

/// Error the Swift initializer raised through `CocoaError(.coderReadCorrupt)`.
pub const CHECKPOINT_CORRUPT: &str = "the checkpoint document is corrupt";

/// Validation content compiled only into the showcase build.
pub struct ShowcaseContent {
    mode: SceneLaunchMode,
    has_typography: bool,
}

impl ShowcaseContent {
    /// Mirrors the Swift initializer: select the authored page, restore the
    /// counter or the saved gallery page, and refuse a corrupt document.
    pub fn new(
        launch: &SceneLaunch,
        checkpoint: &str,
        products: &mut dyn ShowcaseProducts,
    ) -> Result<Self, String> {
        let mode = launch.mode;
        if let Some(gallery) = products.gallery() {
            if let Some(page) = gallery.page_named(&launch.component) {
                gallery.select_page(&page);
            }
        }
        let has_typography =
            mode == SceneLaunchMode::Counter && products.typography_mut().is_some();
        let content = Self {
            mode,
            has_typography,
        };
        if checkpoint.is_empty() {
            return Ok(content);
        }
        let values: BTreeMap<String, String> =
            serde_json::from_str(checkpoint).map_err(|_| CHECKPOINT_CORRUPT.to_owned())?;
        if mode == SceneLaunchMode::Counter {
            let restored = products
                .counter()
                .is_some_and(|counter| counter.restore(&values));
            if !restored {
                return Err(CHECKPOINT_CORRUPT.to_owned());
            }
        } else if let Some(name) = values.get("page")
            && let Some(gallery) = products.gallery()
            && let Some(page) = gallery.page_named(name)
        {
            gallery.select_page(&page);
        }
        Ok(content)
    }

    pub fn mode(&self) -> SceneLaunchMode {
        self.mode
    }

    /// Whether this build supplies counter typography.
    pub fn has_typography(&self) -> bool {
        self.has_typography
    }

    /// Swift fell back to the gallery when the counter had no typography.
    pub fn root(
        &self,
        scale: f32,
        products: &mut dyn ShowcaseProducts,
        painter: Option<Box<dyn CounterTextPainter>>,
    ) -> Option<AnyView> {
        if self.mode == SceneLaunchMode::Counter && self.has_typography {
            if let Some(counter) = products.counter() {
                return Some(counter.root(scale, painter));
            }
        }
        products.gallery().map(|gallery| gallery.root())
    }

    pub fn checkpoint(&self, products: &mut dyn ShowcaseProducts) -> BTreeMap<String, String> {
        if self.mode == SceneLaunchMode::Counter {
            return products
                .counter()
                .map(|counter| counter.checkpoint())
                .unwrap_or_default();
        }
        let mut values = BTreeMap::new();
        if let Some(gallery) = products.gallery() {
            values.insert("page".to_owned(), gallery.current_page());
        }
        values
    }

    /// Swift reported the page only for gallery launches; a counter launch has
    /// no page identity.
    pub fn page(&self, products: &mut dyn ShowcaseProducts) -> Option<String> {
        if self.mode != SceneLaunchMode::Gallery {
            return None;
        }
        products.gallery().map(|gallery| gallery.current_page())
    }
}
