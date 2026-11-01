use crate::{DecodedImage, ImageDecoder, PixelSize};
impl ImageDecoder {
    pub fn decode_partial(
        data: &[u8],
        bounds: PixelSize,
        maximum_source_pixels: u64,
    ) -> Option<DecodedImage> {
        super::image_decoder::validate_bounds(bounds, maximum_source_pixels).ok()?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            Self::decode_apple(data, "", Some(bounds), maximum_source_pixels, true, true).ok()
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            // stb accepts completed progressive scans when a truncated JPEG is closed with EOI.
            // A prefix without any decodable scan returns None; previews never enter the asset cache.
            if data.starts_with(&[0xff, 0xd8]) && !data.ends_with(&[0xff, 0xd9]) {
                let mut closed = data.to_vec();
                closed.extend_from_slice(&[0xff, 0xd9]);
                Self::decode_thumbnail(&closed, "jpg", bounds, maximum_source_pixels).ok()
            } else {
                Self::decode_thumbnail(data, "", bounds, maximum_source_pixels).ok()
            }
        }
    }
}
