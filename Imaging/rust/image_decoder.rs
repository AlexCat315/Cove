use crate::{DecodedImage, ImageOrientation, native};
use std::{
    ffi::CString,
    path::{Path, PathBuf},
};
#[derive(Debug, thiserror::Error)]
pub enum ImageDecodeError {
    #[error("image file not found: {0}")]
    FileNotFound(PathBuf),
    #[error("unsupported image format: {0}")]
    UnsupportedFormat(String),
    #[error("image decode failure: {0}")]
    DecodeFailure(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}
impl PixelSize {
    pub fn new(width: u32, height: u32) -> Result<Self, ImageDecodeError> {
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(ImageDecodeError::DecodeFailure(
                "invalid image dimensions".into(),
            ));
        }
        Ok(Self { width, height })
    }
    pub(crate) fn from_i32(width: i32, height: i32) -> Result<Self, ImageDecodeError> {
        if width <= 0 || height <= 0 {
            return Err(ImageDecodeError::DecodeFailure(
                "invalid image dimensions".into(),
            ));
        }
        Self::new(width as u32, height as u32)
    }
    pub fn pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }
    pub fn bytes(self) -> usize {
        self.width as usize * self.height as usize * 4
    }
    pub(crate) fn bounded(self, bounds: Self, upscale: bool) -> Self {
        let fit = (bounds.width as f64 / self.width as f64)
            .min(bounds.height as f64 / self.height as f64);
        let scale = if upscale { fit } else { fit.min(1.0) };
        Self {
            width: (self.width as f64 * scale).round().max(1.0) as u32,
            height: (self.height as f64 * scale).round().max(1.0) as u32,
        }
    }
}
pub struct ImageDecoder;
impl ImageDecoder {
    pub const DEFAULT_SOURCE_PIXELS: u64 = 32_000_000;
    pub fn decode(
        path: impl AsRef<Path>,
        target: Option<PixelSize>,
    ) -> Result<DecodedImage, ImageDecodeError> {
        let path = path.as_ref();
        let data =
            std::fs::read(path).map_err(|_| ImageDecodeError::FileNotFound(path.to_path_buf()))?;
        let hint = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        Self::decode_with_hint(&data, &hint, target)
    }
    pub fn decode_data(
        data: &[u8],
        target: Option<PixelSize>,
    ) -> Result<DecodedImage, ImageDecodeError> {
        Self::decode_with_hint(data, "", target)
    }
    pub fn decode_with_hint(
        data: &[u8],
        hint: &str,
        target: Option<PixelSize>,
    ) -> Result<DecodedImage, ImageDecodeError> {
        validate_data(data)?;
        if let Some(size) = target {
            PixelSize::new(size.width, size.height)?;
        }
        if Self::apple_format(hint) {
            return Self::decode_apple(data, hint, target, 64 * 1024 * 1024, false, false);
        }
        let orientation = ImageOrientation::read(data);
        let target = target.map(|s| {
            if ImageOrientation::swaps_axes(orientation) {
                PixelSize {
                    width: s.height,
                    height: s.width,
                }
            } else {
                s
            }
        });
        let hint = hint_string(hint)?;
        let mut result = native::ImageResult::default();
        // SAFETY: data/hint live through the call and result exclusively receives bridge-owned memory.
        let ok = unsafe {
            native::cove_image_decode_memory(
                data.as_ptr(),
                data.len(),
                hint.as_ptr(),
                target.map_or(0, |s| s.width as i32),
                target.map_or(0, |s| s.height as i32),
                &mut result,
            )
        };
        if !ok {
            return Err(ImageDecodeError::DecodeFailure(native::message(
                result.error_message,
            )));
        }
        Ok(ImageOrientation::apply(orientation, result.image()?))
    }
    pub fn dimensions(data: &[u8], hint: &str) -> Result<PixelSize, ImageDecodeError> {
        validate_data(data)?;
        let hint = hint_string(hint)?;
        let (mut width, mut height) = (0, 0);
        // SAFETY: input storage and output integers remain valid for this synchronous call.
        let ok = unsafe {
            native::cove_image_dimensions_memory(
                data.as_ptr(),
                data.len(),
                hint.as_ptr(),
                &mut width,
                &mut height,
            )
        };
        if !ok {
            return Err(ImageDecodeError::DecodeFailure(
                "invalid image dimensions".into(),
            ));
        }
        PixelSize::from_i32(width, height)
    }
    pub fn decode_thumbnail(
        data: &[u8],
        hint: &str,
        bounds: PixelSize,
        maximum_source_pixels: u64,
    ) -> Result<DecodedImage, ImageDecodeError> {
        validate_bounds(bounds, maximum_source_pixels)?;
        if Self::apple_format(hint) {
            return Self::decode_apple(
                data,
                hint,
                Some(bounds),
                maximum_source_pixels,
                true,
                false,
            );
        }
        let mut source = Self::dimensions(data, hint)?;
        if source.pixels() > maximum_source_pixels {
            return Err(ImageDecodeError::DecodeFailure(
                "oversized source image".into(),
            ));
        }
        if ImageOrientation::swaps_axes(ImageOrientation::read(data)) {
            std::mem::swap(&mut source.width, &mut source.height);
        }
        let target = source.bounded(
            bounds,
            hint.trim_start_matches('.').eq_ignore_ascii_case("svg"),
        );
        if target.pixels() > maximum_source_pixels {
            return Err(ImageDecodeError::DecodeFailure(
                "oversized thumbnail dimensions".into(),
            ));
        }
        Self::decode_with_hint(data, hint, Some(target))
    }
    fn apple_format(hint: &str) -> bool {
        matches!(
            hint.trim_start_matches('.').to_ascii_lowercase().as_str(),
            "pdf" | "heic" | "heif" | "tiff" | "tif"
        )
    }
}
pub(crate) fn hint_string(hint: &str) -> Result<CString, ImageDecodeError> {
    CString::new(hint).map_err(|_| ImageDecodeError::DecodeFailure("invalid format hint".into()))
}
pub(crate) fn validate_data(data: &[u8]) -> Result<(), ImageDecodeError> {
    if data.is_empty() || data.len() > i32::MAX as usize {
        Err(ImageDecodeError::DecodeFailure(
            "empty or oversized image data".into(),
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn validate_bounds(bounds: PixelSize, budget: u64) -> Result<(), ImageDecodeError> {
    if bounds.width == 0
        || bounds.height == 0
        || bounds.width > 8192
        || bounds.height > 8192
        || budget == 0
    {
        Err(ImageDecodeError::DecodeFailure(
            "invalid thumbnail dimensions or pixel budget".into(),
        ))
    } else {
        Ok(())
    }
}
