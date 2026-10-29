use std::ffi::{CStr, c_char};
#[repr(C)]
#[derive(Default)]
pub(crate) struct ImageResult { pub pixels: *mut u8, pub width: i32, pub height: i32, pub error_message: *mut c_char }
#[repr(C)]
#[derive(Default)]
pub(crate) struct AnimationResult { pub pixels: *mut u8, pub durations_ms: *mut i32, pub width: i32, pub height: i32, pub frame_count: i32, pub loop_count: i32, pub error_message: *mut c_char }
unsafe extern "C" {
    pub fn cove_image_decode_memory(data: *const u8, count: usize, extension: *const c_char, width: i32, height: i32, result: *mut ImageResult) -> bool;
    pub fn cove_image_dimensions_memory(data: *const u8, count: usize, extension: *const c_char, width: *mut i32, height: *mut i32) -> bool;
    fn cove_image_decode_free(result: *mut ImageResult);
    pub fn cove_image_decode_animation_memory(data: *const u8, count: usize, extension: *const c_char, width: i32, height: i32, frames: i32, result: *mut AnimationResult) -> bool;
    fn cove_image_animation_free(result: *mut AnimationResult);
    pub fn cove_image_orient_rgba(input: *const u8, width: usize, height: usize, orientation: i32, output: *mut u8);
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn cove_image_apple_decode(data: *const u8, count: usize, pdf: bool, width: i32, height: i32, budget: u64, thumbnail: bool, partial: bool, result: *mut ImageResult) -> bool;
}
impl Drop for ImageResult { fn drop(&mut self) { // SAFETY: the bridge initializes and owns these allocations.
    unsafe { cove_image_decode_free(self) }; } }
impl Drop for AnimationResult { fn drop(&mut self) { // SAFETY: the bridge initializes and owns these allocations.
    unsafe { cove_image_animation_free(self) }; } }
pub(crate) fn message(value: *const c_char) -> String { if value.is_null() { "native image decoder failed".into() } else { // SAFETY: error messages are NUL terminated until result destruction.
    unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned() } }
impl ImageResult {
    pub fn image(&self) -> Result<crate::DecodedImage, crate::ImageDecodeError> {
        let size = crate::PixelSize::from_i32(self.width, self.height)?;
        if self.pixels.is_null() { return Err(crate::ImageDecodeError::DecodeFailure(message(self.error_message))); }
        // SAFETY: successful native results contain width * height RGBA bytes and remain alive during copying.
        crate::DecodedImage::new(size.width, size.height, unsafe { std::slice::from_raw_parts(self.pixels, size.bytes()) }.to_vec()).map_err(|e| crate::ImageDecodeError::DecodeFailure(e.to_string()))
    }
}
