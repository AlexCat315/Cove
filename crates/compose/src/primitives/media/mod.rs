//! Media primitives.
pub(crate) mod async_image;
pub(crate) mod icon;
pub(crate) mod image;
pub mod image_data_download;
pub(crate) mod image_file;
pub(crate) mod image_geometry;
pub(crate) mod image_load_services;
pub(crate) mod loading_animation_clock;
pub(crate) mod skeleton;
pub mod ui_icon_catalog;
pub mod ui_icons;
pub(crate) mod vector_image_raster;

pub use api::*;
pub use async_image::{AsyncImageAnimation, AsyncImageAnimationCache, AsyncImageSession};

pub use image::{ContentScaleHolder, image_is_same_source, texture_source};
pub use image_data_download::{
    AsyncImageError, DownloadProgressReporter, DownloadedImage, ImageDataDownload,
    ImageDownloadTransport, ImageDownloadTransportHolder,
};
pub use image_file::{
    ImageLoadDiagnostic, ImageLoadDiagnostics, ImageLoadFailureReason, decode_image,
};
pub use image_geometry::ImageGeometry;
pub use image_load_services::{
    ImageDecode, ImageDecodeExecution, ImageInflightLoads, ImageLoadRequest, ImageLoadServices,
    ImagePayload, ImageProgress, ImagePublicationQueue,
};
pub use loading_animation_clock::LoadingAnimationClock;
pub use ui_icon_catalog::UIIconEntry;

pub use vector_image_raster::VectorImageRaster;

pub(crate) mod api {
    pub use super::async_image::{AsyncImage, AsyncImagePhase, ImageLoadingPolicy};
    pub use super::icon::Icon;
    pub use super::image::{Image, ImageSource};
    pub use super::image_geometry::{ImageContentMode, ImageRenderingMode};
    pub use super::skeleton::{Skeleton, SkeletonAppearance, SkeletonMotion, SkeletonShape};
    pub use super::ui_icons::UIIcons;
}
