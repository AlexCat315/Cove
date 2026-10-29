use crate::Size;
use std::sync::Arc;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BitmapError {
    #[error("RGBA bitmap dimensions must be in 1..=16384 and match its byte count")]
    InvalidDimensions,
}

/// Owned, straight-alpha RGBA8 pixels. Cloning shares immutable storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pixels: Arc<[u8]>,
    width: u32,
    height: u32,
}

unsafe extern "C" {
    fn cove_image_reduced_mip(pixels: *const u8, width: usize, height: usize, output: *mut u8);
}
impl Bitmap {
    pub fn new(width: u32, height: u32, pixels: impl Into<Arc<[u8]>>) -> Result<Self, BitmapError> {
        let pixels = pixels.into();
        if width == 0
            || height == 0
            || width > 16384
            || height > 16384
            || (width as usize)
                .checked_mul(height as usize)
                .and_then(|n| n.checked_mul(4))
                != Some(pixels.len())
        {
            return Err(BitmapError::InvalidDimensions);
        }
        Ok(Self {
            pixels,
            width,
            height,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
    pub fn reduced_mip(&self) -> Option<Self> {
        if self.width == 1 && self.height == 1 {
            return None;
        }
        let width = (self.width / 2).max(1);
        let height = (self.height / 2).max(1);
        let mut pixels = vec![0; width as usize * height as usize * 4];
        // SAFETY: constructor validates dimensions and input length; output has exactly
        // max(1, width/2) * max(1, height/2) * 4 exclusively writable bytes.
        unsafe {
            cove_image_reduced_mip(
                self.pixels.as_ptr(),
                self.width as usize,
                self.height as usize,
                pixels.as_mut_ptr(),
            );
        }
        Some(Self {
            pixels: pixels.into(),
            width,
            height,
        })
    }
    pub fn mip_chain(&self) -> Vec<Self> {
        let mut chain = vec![self.clone()];
        while let Some(next) = chain.last().unwrap().reduced_mip() {
            chain.push(next);
        }
        chain
    }
}

/// `count` is the number of reduced levels beyond the base image.
pub fn image_lod(source: Size, target: Size, count: usize) -> usize {
    if !source.width.is_finite()
        || !source.height.is_finite()
        || !target.width.is_finite()
        || !target.height.is_finite()
        || target.width <= 0.0
        || target.height <= 0.0
    {
        return 0;
    }
    let mut ratio = (source.width / target.width).min(source.height / target.height);
    let mut level = 0;
    while ratio > std::f32::consts::SQRT_2 && level < count {
        ratio *= 0.5;
        level += 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn area_filter_includes_odd_edges_and_ignores_transparent_rgb() {
        let bitmap = Bitmap::new(3, 1, vec![255, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 255]).unwrap();
        let mip = bitmap.reduced_mip().unwrap();
        assert_eq!((mip.width(), mip.height()), (1, 1));
        assert_eq!(mip.pixels(), [128, 0, 128, 170]);
        assert!(mip.reduced_mip().is_none());
        assert_eq!(bitmap.pixels().len(), 12);
    }
    #[test]
    fn invalid_bitmap_never_reaches_native_code() {
        for (w, h, pixels) in [(0, 1, vec![]), (1, 1, vec![0; 3]), (16385, 1, vec![])] {
            assert_eq!(
                Bitmap::new(w, h, pixels).unwrap_err(),
                BitmapError::InvalidDimensions
            );
        }
    }
}
