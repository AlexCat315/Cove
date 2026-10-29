//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/VectorImageRaster.swift.
//!
//! A vector image follows its actual draw size and the current drawable
//! density. The registry keeps each raster size, so moving between screens
//! needs no recompose.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cove_imaging::ImageAssetRegistryHolder;

use super::image::{Image, ImageSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    width_bits: u32,
    height_bits: u32,
    density_bits: u32,
    registry: Option<usize>,
    revision: u64,
}

impl Key {
    fn new(width: f32, height: f32) -> Self {
        let scale = cove_core::content_scale();
        let density = if scale.is_finite() { scale.max(1.0) } else { 1.0 };
        let registry = ImageAssetRegistryHolder::current();
        Self {
            width_bits: width.to_bits(),
            height_bits: height.to_bits(),
            density_bits: density.to_bits(),
            registry: registry.as_ref().map(|registry| Arc::as_ptr(registry) as *const () as usize),
            revision: registry.as_ref().map_or(0, |registry| registry.revision()),
        }
    }
}

/// Cache of the raster variant used at the current draw size.
pub struct VectorImageRaster {
    path: PathBuf,
    key: RefCell<Key>,
    asset: RefCell<ImageSource>,
}

impl VectorImageRaster {
    pub fn new(path: impl Into<PathBuf>, initial: ImageSource, width: f32, height: f32) -> Self {
        Self {
            path: path.into(),
            key: RefCell::new(Key::new(width, height)),
            asset: RefCell::new(initial),
        }
    }

    /// Re-rasterises when the logical size, density or registry revision
    /// changes; otherwise returns the retained raster.
    pub fn resolve(&self, width: f32, height: f32) -> ImageSource {
        let next = Key::new(width, height);
        if next != *self.key.borrow() {
            *self.asset.borrow_mut() = Image::resolve_path(&self.path, width, height);
            *self.key.borrow_mut() = next;
        }
        self.asset.borrow().clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
