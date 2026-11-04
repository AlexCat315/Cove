//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/VectorImageRaster.swift.
//!
//! A vector image follows its actual draw size and the current drawable
//! density. The registry keeps each raster size, so moving between screens
//! needs no recompose.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
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
        let density = if scale.is_finite() {
            scale.max(1.0)
        } else {
            1.0
        };
        let registry = ImageAssetRegistryHolder::current();
        Self {
            width_bits: width.to_bits(),
            height_bits: height.to_bits(),
            density_bits: density.to_bits(),
            registry: registry
                .as_ref()
                .map(|registry| Arc::as_ptr(registry) as *const () as usize),
            revision: registry.as_ref().map_or(0, |registry| registry.revision()),
        }
    }
}

/// Cache of the raster variant used at the current draw size.
///
/// The source it starts with was rasterized at the authored size. A later size
/// or density change keeps drawing the previous raster (scaled) and
/// rasterizes the new one on the worker pool; `on_ready` asks for a repaint
/// once it lands.
pub struct VectorImageRaster {
    path: PathBuf,
    key: RefCell<Key>,
    asset: RefCell<ImageSource>,
    pending: RefCell<Option<Key>>,
}

impl VectorImageRaster {
    pub fn new(path: impl Into<PathBuf>, initial: ImageSource, width: f32, height: f32) -> Self {
        Self {
            path: path.into(),
            key: RefCell::new(Key::new(width, height)),
            asset: RefCell::new(initial),
            // The initial source is already rasterized at this size.
            pending: RefCell::new(None),
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

    /// Like [`resolve`](Self::resolve), but a re-raster at a new size runs in
    /// the background while the current raster keeps painting.
    pub fn resolve_async(
        self: &Rc<Self>,
        width: f32,
        height: f32,
        on_ready: impl Fn() + 'static,
    ) -> ImageSource {
        let next = Key::new(width, height);
        if next == *self.key.borrow() || self.pending.borrow().as_ref() == Some(&next) {
            return self.asset.borrow().clone();
        }
        *self.pending.borrow_mut() = Some(next);
        let path = self.path.clone();
        let target = Image::raster_target(&path, width, height, cove_core::content_scale());
        let registry = ImageAssetRegistryHolder::current();
        let raster = Rc::downgrade(self);
        let job_path = path.clone();
        cove_core::work::spawn_then(
            move || Image::rasterize(registry, &job_path, target),
            move |result| {
                let Some(raster) = raster.upgrade() else {
                    return;
                };
                // A newer size superseded this one; its own job will land.
                if raster.pending.borrow().as_ref() != Some(&next) {
                    return;
                }
                raster.pending.borrow_mut().take();
                *raster.asset.borrow_mut() = Image::source_for(&path, result);
                *raster.key.borrow_mut() = next;
                on_ready();
            },
        );
        self.asset.borrow().clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
