use crate::{
    DecodedImage, ImageDecodeError, ImageDecoder, PixelSize,
    image_decoder::{hint_string, validate_bounds},
    native,
};
#[derive(Clone, Debug)]
pub struct DecodedAnimation {
    pub frames: Vec<DecodedImage>,
    pub durations: Vec<f64>,
    pub loop_count: u32,
}
impl DecodedAnimation {
    pub fn total_duration(&self) -> f64 {
        self.durations.iter().sum()
    }
}
impl ImageDecoder {
    pub fn is_animatable(data: &[u8], _hint: &str) -> bool {
        data.starts_with(b"GIF8")
            || (data.len() >= 21
                && &data[..4] == b"RIFF"
                && &data[8..12] == b"WEBP"
                && &data[12..16] == b"VP8X"
                && data[20] & 2 != 0)
    }
    pub fn decode_animation(
        data: &[u8],
        hint: &str,
        bounds: PixelSize,
        maximum_source_pixels: u64,
        maximum_frames: usize,
    ) -> Result<Option<DecodedAnimation>, ImageDecodeError> {
        if !Self::is_animatable(data, hint) {
            return Ok(None);
        }
        validate_bounds(bounds, maximum_source_pixels)?;
        let source = Self::dimensions(data, hint)?;
        if source.pixels() > maximum_source_pixels {
            return Err(ImageDecodeError::DecodeFailure(
                "oversized animation dimensions".into(),
            ));
        }
        let target = source.bounded(bounds, false);
        let frame_budget = maximum_frames
            .min((maximum_source_pixels / target.pixels()) as usize)
            .clamp(1, i32::MAX as usize);
        let hint = hint_string(hint)?;
        let mut result = native::AnimationResult::default();
        // SAFETY: bounded input/target are valid, result has exclusive ownership of the native allocations.
        let ok = unsafe {
            native::cove_image_decode_animation_memory(
                data.as_ptr(),
                data.len(),
                hint.as_ptr(),
                target.width as i32,
                target.height as i32,
                frame_budget as i32,
                &mut result,
            )
        };
        if !ok {
            return Err(ImageDecodeError::DecodeFailure(native::message(
                result.error_message,
            )));
        }
        if result.frame_count <= 1 {
            return Ok(None);
        }
        let size = PixelSize::from_i32(result.width, result.height)?;
        let count = result.frame_count as usize;
        if count > frame_budget || result.pixels.is_null() || result.durations_ms.is_null() {
            return Err(ImageDecodeError::DecodeFailure(
                "invalid animation result".into(),
            ));
        }
        let length = size.bytes().checked_mul(count).ok_or_else(|| {
            ImageDecodeError::DecodeFailure("animation byte count overflow".into())
        })?;
        // SAFETY: native decoder returns count contiguous RGBA frames and matching durations; result remains alive.
        let pixels = unsafe { std::slice::from_raw_parts(result.pixels, length) };
        // SAFETY: the duration allocation contains frame_count elements.
        let durations = unsafe { std::slice::from_raw_parts(result.durations_ms, count) };
        let frames = pixels
            .chunks_exact(size.bytes())
            .map(|frame| {
                DecodedImage::new(size.width, size.height, frame.to_vec())
                    .map_err(|e| ImageDecodeError::DecodeFailure(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(DecodedAnimation {
            frames,
            durations: durations
                .iter()
                .map(|&ms| if ms <= 10 { 0.1 } else { ms as f64 / 1000.0 })
                .collect(),
            loop_count: result.loop_count.max(0) as u32,
        }))
    }
}
