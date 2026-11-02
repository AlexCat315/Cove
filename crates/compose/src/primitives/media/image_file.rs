//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/Image+File.swift.
//!
//! File, URL and bundle-resource constructors for `Image`, plus load-failure
//! diagnostics.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use cove_imaging::{
    BundleImageResource, ImageAsset, ImageAssetRegistryHolder, ImageDecodeError, ImageDecoder,
    PixelSize,
};

use super::image::Image;
use super::image::ImageSource;
use super::image_geometry::{ImageContentMode, ImageRenderingMode};
use super::vector_image_raster::VectorImageRaster;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageLoadFailureReason {
    DecodeFailed,
    ResourceNotFound,
}

impl ImageLoadFailureReason {
    pub fn description(self) -> &'static str {
        match self {
            Self::DecodeFailed => "decode failed",
            Self::ResourceNotFound => "resource not found",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageLoadDiagnostic {
    pub path: String,
    pub reason: ImageLoadFailureReason,
    pub details: String,
}

impl ImageLoadDiagnostic {
    pub fn new(
        path: impl Into<String>,
        reason: ImageLoadFailureReason,
        details: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            reason,
            details: details.into(),
        }
    }
}

thread_local! {
    static DIAGNOSTIC_HANDLER: RefCell<Option<Rc<dyn Fn(&ImageLoadDiagnostic)>>> = const { RefCell::new(None) };
}

/// Optional process hook mirroring the Swift static `onEvent` callback.
pub struct ImageLoadDiagnostics;
impl ImageLoadDiagnostics {
    pub fn set_on_event(handler: Option<Rc<dyn Fn(&ImageLoadDiagnostic)>>) {
        DIAGNOSTIC_HANDLER.with(|slot| *slot.borrow_mut() = handler);
    }

    pub fn emit(
        path: impl Into<String>,
        reason: ImageLoadFailureReason,
        details: impl Into<String>,
    ) {
        let event = ImageLoadDiagnostic::new(path, reason, details);
        DIAGNOSTIC_HANDLER.with(|slot| {
            if let Some(handler) = slot.borrow().as_ref() {
                handler(&event);
            }
        });
    }
}

impl Image {
    /// Loads through `ImageAssetRegistryHolder::current` when installed, or
    /// decodes directly otherwise. Vector formats rasterise at physical-pixel
    /// resolution so they stay crisp on HiDPI displays.
    pub fn resolve_path(path: impl AsRef<Path>, width: f32, height: f32) -> ImageSource {
        let path = path.as_ref();
        let target = Self::raster_target(path, width, height, cove_core::content_scale());
        Self::source_for(
            path,
            Self::rasterize(ImageAssetRegistryHolder::current(), path, target),
        )
    }

    /// The pixel size a vector source rasterizes to at `density`; `None` for
    /// bitmaps, which decode at their own size.
    pub(crate) fn raster_target(
        path: &Path,
        width: f32,
        height: f32,
        density: f32,
    ) -> Option<PixelSize> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if extension != "svg" && extension != "pdf" {
            return None;
        }
        let scale = if density.is_finite() {
            density.max(1.0)
        } else {
            1.0
        };
        let width = ((width * scale).round().max(1.0) as u32).min(16_384);
        let height = ((height * scale).round().max(1.0) as u32).min(16_384);
        PixelSize::new(width, height).ok()
    }

    /// Decodes or rasterizes `path`. Thread-safe, so it may run on a worker.
    pub(crate) fn rasterize(
        registry: Option<Arc<cove_imaging::ImageAssetRegistry>>,
        path: &Path,
        target: Option<PixelSize>,
    ) -> Result<Arc<ImageAsset>, String> {
        match registry {
            Some(registry) => registry.texture(path, target),
            None => ImageDecoder::decode(path, target).and_then(ImageAsset::new),
        }
        .map_err(|error| error.to_string())
    }

    /// The image source for a finished raster, reporting a failed decode.
    pub(crate) fn source_for(path: &Path, result: Result<Arc<ImageAsset>, String>) -> ImageSource {
        match result {
            Ok(asset) => ImageSource::Asset(asset),
            Err(error) => {
                ImageLoadDiagnostics::emit(
                    path.to_string_lossy().to_string(),
                    ImageLoadFailureReason::DecodeFailed,
                    format!("using TextureId::none; image decode failed: {error}"),
                );
                ImageSource::Texture {
                    texture_id: 0,
                    size: None,
                }
            }
        }
    }

    pub fn resolve_resource(
        resource: &BundleImageResource,
        width: f32,
        height: f32,
    ) -> ImageSource {
        match resource.path() {
            Some(path) => Self::resolve_path(path, width, height),
            None => {
                ImageLoadDiagnostics::emit(
                    resource.name.clone(),
                    ImageLoadFailureReason::ResourceNotFound,
                    "bundle resource path is nil",
                );
                ImageSource::Texture {
                    texture_id: 0,
                    size: None,
                }
            }
        }
    }

    pub fn from_file(
        path: impl AsRef<Path>,
        width: f32,
        height: f32,
        tint: cove_core::Color,
        content_mode: ImageContentMode,
        rendering_mode: ImageRenderingMode,
    ) -> Self {
        let path = path.as_ref();
        let source = Self::resolve_path(path, width, height);
        let mut image = Self::new(source, width, height, tint, content_mode, rendering_mode);
        if matches!(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "svg" | "pdf"
        ) {
            image.vector_source_url = Some(path.to_path_buf());
        }
        image
    }

    pub fn from_resource(
        resource: &BundleImageResource,
        width: f32,
        height: f32,
        tint: cove_core::Color,
        content_mode: ImageContentMode,
        rendering_mode: ImageRenderingMode,
    ) -> Self {
        match resource.path() {
            Some(path) => Self::from_file(path, width, height, tint, content_mode, rendering_mode),
            None => {
                ImageLoadDiagnostics::emit(
                    resource.name.clone(),
                    ImageLoadFailureReason::ResourceNotFound,
                    "bundle resource path is nil",
                );
                Self::new(
                    ImageSource::Texture {
                        texture_id: 0,
                        size: None,
                    },
                    width,
                    height,
                    tint,
                    content_mode,
                    rendering_mode,
                )
            }
        }
    }

    pub fn from_url(
        url: impl AsRef<Path>,
        width: f32,
        height: f32,
        tint: cove_core::Color,
        content_mode: ImageContentMode,
        rendering_mode: ImageRenderingMode,
    ) -> Self {
        Self::from_file(url, width, height, tint, content_mode, rendering_mode)
    }

    /// Vector helper retained so `VectorImageRaster` can share the same path.
    pub fn vector_raster(
        path: impl Into<PathBuf>,
        initial: ImageSource,
        width: f32,
        height: f32,
    ) -> VectorImageRaster {
        VectorImageRaster::new(path, initial, width, height)
    }
}

/// Errors are surfaced through diagnostics; callers that want them directly can
/// use this wrapper instead of the lossy `resolve_path`.
pub fn decode_image(
    path: impl AsRef<Path>,
    target: Option<PixelSize>,
) -> Result<std::sync::Arc<ImageAsset>, ImageDecodeError> {
    ImageDecoder::decode(path, target).and_then(ImageAsset::new)
}
