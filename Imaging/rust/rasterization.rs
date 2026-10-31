use crate::{DecodedImage, ImageDecodeError, ImageDecoder, PixelSize};
impl ImageDecoder {
    pub(crate) fn decode_apple(
        data: &[u8],
        hint: &str,
        target: Option<PixelSize>,
        budget: u64,
        thumbnail: bool,
        partial: bool,
    ) -> Result<DecodedImage, ImageDecodeError> {
        super::image_decoder::validate_data(data)?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let mut result = crate::native::ImageResult::default();
            // SAFETY: byte storage lives through the synchronous call; result is initialized and exclusively borrowed.
            let ok = unsafe {
                crate::native::cove_image_apple_decode(
                    data.as_ptr(),
                    data.len(),
                    hint.eq_ignore_ascii_case("pdf"),
                    target.map_or(0, |s| s.width as i32),
                    target.map_or(0, |s| s.height as i32),
                    budget,
                    thumbnail,
                    partial,
                    &mut result,
                )
            };
            if !ok {
                return Err(ImageDecodeError::DecodeFailure(crate::native::message(
                    result.error_message,
                )));
            }
            result.image()
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = (target, budget, thumbnail, partial);
            Err(ImageDecodeError::UnsupportedFormat(hint.into()))
        }
    }
}
