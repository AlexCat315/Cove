//! Media primitives.
pub(crate) mod async_image;
pub(crate) mod audio_meter;
pub(crate) mod danmaku;
pub(crate) mod frame_ticker;
pub(crate) mod icon;
pub(crate) mod image;
pub mod image_data_download;
pub(crate) mod image_file;
pub(crate) mod image_geometry;
pub(crate) mod image_load_services;
pub(crate) mod loading_animation_clock;
pub(crate) mod media_player;
pub(crate) mod progress_bar;
pub(crate) mod skeleton;
pub mod ui_icon_catalog;
pub mod ui_icons;
pub(crate) mod vector_image_raster;
pub(crate) mod video_player;
pub(crate) mod video;
pub(crate) mod waveform;

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

/// The media layer these components present; players, decoders and devices live here.
pub use cove_media;
pub use vector_image_raster::VectorImageRaster;
pub use video::VideoSurface;

pub(crate) mod api {
    pub use super::async_image::{AsyncImage, AsyncImagePhase, ImageLoadingPolicy};
    pub use super::audio_meter::{AudioLevelMeter, meter_fraction};
    pub use super::icon::Icon;
    pub use super::image::{Image, ImageSource};
    pub use super::image_geometry::{ImageContentMode, ImageRenderingMode};
    pub use super::media_player::{
        AudioPlayer, MediaTransport, format_media_time, steady_snapshot, toggle_playback,
        use_playback, use_playback_select,
    };
    pub use super::skeleton::{Skeleton, SkeletonAppearance, SkeletonMotion, SkeletonShape};
    pub use super::ui_icons::UIIcons;
    pub use super::danmaku::{
        ActiveDanmaku, DanmakuComment, DanmakuFeed, DanmakuLayer, DanmakuMode, DanmakuScheduler,
        DanmakuSettings,
    };
    pub use super::frame_ticker::FrameTicker;
    pub use super::progress_bar::{MediaProgressBar, PROGRESS_BAR_HEIGHT};
    pub use super::video::VideoView;
    pub use super::video_player::{
        PLAYBACK_RATES, PlayerActions, PlayerPresentation, PlayerSettings, VideoPlayer,
        send_danmaku,
    };
    pub use super::waveform::{Waveform, resample_peaks};
}
