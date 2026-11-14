//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/VectorImageRaster.swift.
//!
//! A vector image follows its actual draw size and the current drawable
//! density. The registry keeps each raster size, so moving between screens
//! needs no recompose.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use cove_imaging::{
    ImageAsset, ImageAssetRegistry, ImageAssetRegistryHolder, ImageDecoder, PixelSize,
};

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

/// Where a vector's bytes come from: a file on disk, or the binary itself.
#[derive(Clone)]
enum VectorSource {
    Path(PathBuf),
    Bytes(&'static [u8]),
}

impl VectorSource {
    /// The name used for extension checks and diagnostics.
    fn hint_path(&self) -> PathBuf {
        match self {
            Self::Path(path) => path.clone(),
            Self::Bytes(_) => PathBuf::from("embedded.svg"),
        }
    }

    fn raster_target(&self, width: f32, height: f32) -> Option<PixelSize> {
        Image::raster_target(&self.hint_path(), width, height, cove_core::content_scale())
    }

    fn rasterize(
        &self,
        registry: Option<Arc<ImageAssetRegistry>>,
        target: Option<PixelSize>,
    ) -> Result<Arc<ImageAsset>, String> {
        match self {
            Self::Path(path) => Image::rasterize(registry, path, target),
            Self::Bytes(data) => ImageDecoder::decode_with_hint(data, "svg", target)
                .and_then(ImageAsset::new)
                .map_err(|error| error.to_string()),
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
    source: VectorSource,
    key: RefCell<Key>,
    asset: RefCell<ImageSource>,
    pending: RefCell<Option<Key>>,
}

impl VectorImageRaster {
    pub fn new(path: impl Into<PathBuf>, initial: ImageSource, width: f32, height: f32) -> Self {
        Self::with_source(VectorSource::Path(path.into()), initial, width, height)
    }

    /// An icon that ships inside the binary; the raster still follows the
    /// draw size, so no file has to exist at run time.
    pub fn from_bytes(data: &'static [u8], initial: ImageSource, width: f32, height: f32) -> Self {
        Self::with_source(VectorSource::Bytes(data), initial, width, height)
    }

    fn with_source(source: VectorSource, initial: ImageSource, width: f32, height: f32) -> Self {
        Self {
            source,
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
            let target = self.source.raster_target(width, height);
            let raster = self
                .source
                .rasterize(ImageAssetRegistryHolder::current(), target);
            *self.asset.borrow_mut() = Image::source_for(&self.source.hint_path(), raster);
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
        let source = self.source.clone();
        let target = source.raster_target(width, height);
        let registry = ImageAssetRegistryHolder::current();
        let raster = Rc::downgrade(self);
        let job_source = source.clone();
        cove_core::work::spawn_then(
            move || job_source.rasterize(registry, target),
            move |result| {
                let Some(raster) = raster.upgrade() else {
                    return;
                };
                // A newer size superseded this one; its own job will land.
                if raster.pending.borrow().as_ref() != Some(&next) {
                    return;
                }
                raster.pending.borrow_mut().take();
                *raster.asset.borrow_mut() = Image::source_for(&source.hint_path(), result);
                *raster.key.borrow_mut() = next;
                on_ready();
            },
        );
        self.asset.borrow().clone()
    }

    /// The file this raster follows, when it is file-backed.
    pub fn path(&self) -> Option<&Path> {
        match &self.source {
            VectorSource::Path(path) => Some(path),
            VectorSource::Bytes(_) => None,
        }
    }
}
