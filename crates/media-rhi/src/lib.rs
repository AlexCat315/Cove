//! Explicit CPU upload baseline for retained media frames on every Cove RHI backend.
use cove_graphics::{rhi::*, ui::texture_image};
use cove_imaging::TextureImage;
use cove_media::{
    MediaError, MediaErrorKind, MediaResult, PixelFormat, VideoDynamicRange, VideoOutput,
};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct TransferStatistics {
    pub uploaded_frames: u64,
    /// Source plane extents supplied to upload, including row padding, excluding
    /// opaque driver staging/alignment costs. This is not GPU bus measurement.
    pub uploaded_bytes: u64,
}
pub struct VideoPresenter {
    device: Arc<Device>,
    output: VideoOutput,
    last_revision: u64,
    image: Option<Arc<TextureImage>>,
    statistics: TransferStatistics,
}
impl VideoPresenter {
    pub fn new(device: Arc<Device>, output: VideoOutput) -> Self {
        Self {
            device,
            output,
            last_revision: u64::MAX,
            image: None,
            statistics: TransferStatistics::default(),
        }
    }
    pub fn image(&self) -> Option<Arc<TextureImage>> {
        self.image.clone()
    }
    pub fn statistics(&self) -> &TransferStatistics {
        &self.statistics
    }
    /// Invoke on the RHI owner thread. Existing frame snapshots keep the old texture.
    pub fn update(&mut self) -> MediaResult<Option<Arc<TextureImage>>> {
        let snapshot = self.output.snapshot();
        let revision = snapshot.revision;
        if revision == self.last_revision {
            return Ok(self.image());
        }
        let Some(frame) = snapshot.frame else {
            self.image = None;
            self.last_revision = revision;
            return Ok(None);
        };
        if matches!(
            frame.format.dynamic_range,
            VideoDynamicRange::Pq | VideoDynamicRange::Hlg
        ) {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "HDR video requires explicit tone mapping before the SDR UI presenter",
            ));
        }
        let format = match &frame.format.pixel_format {
            PixelFormat::Rgba8 => TextureFormat::Rgba8Unorm,
            PixelFormat::Bgra8 => TextureFormat::Bgra8Unorm,
            PixelFormat::Other(name) => {
                return Err(MediaError::new(
                    MediaErrorKind::Unsupported,
                    format!("CPU video presenter requires negotiated RGBA8/BGRA8, got {name}"),
                ));
            }
        };
        let plane = frame.format.planes.first().ok_or_else(|| {
            MediaError::new(MediaErrorKind::Negotiation, "video frame has no plane")
        })?;
        let data: std::sync::Arc<[u8]> = match frame.storage.shared_plane(0) {
            Some(plane) => plane,
            None => frame
                .storage
                .copy_planes()?
                .into_iter()
                .next()
                .map(Into::into)
                .ok_or_else(|| MediaError::new(MediaErrorKind::Decode, "missing video pixels"))?,
        };
        let data = &data[..];
        let width = frame.format.width as usize;
        let height = frame.format.height as usize;
        let required = plane.stride.checked_mul(height).ok_or_else(|| {
            MediaError::new(MediaErrorKind::InvalidInput, "video extent overflow")
        })?;
        if width == 0
            || height == 0
            || plane.stride
                < width.checked_mul(4).ok_or_else(|| {
                    MediaError::new(MediaErrorKind::InvalidInput, "video row overflow")
                })?
            || data.len() < required
        {
            return Err(MediaError::new(
                MediaErrorKind::Decode,
                "video plane is smaller than its declared extent",
            ));
        }
        let image = self
            .device
            .with_frame_session(|| {
                let resource = Arc::new(TextureResource::new(
                    self.device.clone(),
                    TextureDescriptor::new(
                        width,
                        height,
                        format,
                        TextureUsage::SAMPLED
                            | TextureUsage::TRANSFER_DESTINATION
                            | TextureUsage::TRANSFER_SOURCE,
                    )
                    .with_label("media-frame"),
                )?);
                self.device.upload_texture_data(
                    resource.texture(),
                    data,
                    TextureUploadRegion::new(width as i64, height as i64),
                    plane.stride,
                    TextureSubresource::default(),
                )?;
                texture_image(resource, Vec::new())
            })
            .map_err(|e| MediaError::new(MediaErrorKind::Device, e.to_string()))?;
        self.statistics.uploaded_frames += 1;
        self.statistics.uploaded_bytes += required as u64;
        self.image = Some(image.clone());
        self.last_revision = revision;
        Ok(Some(image))
    }
}
