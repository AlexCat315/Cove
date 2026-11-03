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

impl ImageDecoder {
    /// Rasterizes a bundle glyph (an alpha-mask SVG) into a black-on-transparent
    /// template PNG so a native menu can tint it alongside the item text.
    pub fn template_png(
        data: &[u8],
        extension: &str,
        points: f32,
    ) -> Result<Vec<u8>, ImageDecodeError> {
        super::image_decoder::validate_data(data)?;
        if !points.is_finite() || points <= 0.0 {
            return Err(ImageDecodeError::DecodeFailure(
                "invalid template icon size".into(),
            ));
        }
        let hint = super::image_decoder::hint_string(extension)?;
        let mut result = crate::native::TemplatePngResult::default();
        // SAFETY: data/hint live through the call and result exclusively receives bridge-owned memory.
        let ok = unsafe {
            crate::native::cove_image_template_png_memory(
                data.as_ptr(),
                data.len(),
                hint.as_ptr(),
                points.round() as i32,
                &mut result,
            )
        };
        if !ok || result.png.is_null() || result.png_size == 0 {
            return Err(ImageDecodeError::DecodeFailure(crate::native::message(
                result.error_message,
            )));
        }
        // SAFETY: a successful result holds png_size initialized bytes that stay alive during the copy.
        Ok(unsafe { std::slice::from_raw_parts(result.png, result.png_size) }.to_vec())
    }
}
