//! Translation of Orientation.swift; RGBA rearrangement is implemented in C.
use crate::DecodedImage;
pub struct ImageOrientation;
impl ImageOrientation {
    pub fn read(data: &[u8]) -> u8 {
        let bytes = &data[..data.len().min(256 * 1024)];
        if bytes.len() < 5 || bytes[..2] != [0xff, 0xd8] { return 1; }
        let mut offset = 2;
        while offset + 4 <= bytes.len() && bytes[offset] == 0xff {
            let marker = bytes[offset + 1]; if marker == 0xd9 || marker == 0xda { break; }
            let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
            if length < 2 || offset + 2 + length > bytes.len() { break; }
            if marker == 0xe1 && length >= 16 && &bytes[offset + 4..offset + 10] == b"Exif\0\0" {
                return tiff_orientation(&bytes[offset + 10..offset + 2 + length]).unwrap_or(1);
            }
            offset += 2 + length;
        }
        1
    }
    pub fn swaps_axes(orientation: u8) -> bool { (5..=8).contains(&orientation) }
    pub fn apply(orientation: u8, image: DecodedImage) -> DecodedImage {
        if !(2..=8).contains(&orientation) { return image; }
        let (w, h) = (image.width(), image.height()); let (ow, oh) = if Self::swaps_axes(orientation) { (h, w) } else { (w, h) };
        let mut pixels = vec![0; image.pixels().len()];
        // SAFETY: Bitmap validates dimensions, both buffers hold exactly w*h*4 bytes, and do not overlap.
        unsafe { crate::native::cove_image_orient_rgba(image.pixels().as_ptr(), w as usize, h as usize, orientation as i32, pixels.as_mut_ptr()); }
        DecodedImage::new(ow, oh, pixels).expect("orientation preserves bitmap length")
    }
}
fn tiff_orientation(bytes: &[u8]) -> Option<u8> {
    let little = match bytes.get(..2)? { b"II" => true, b"MM" => false, _ => return None };
    let u16_at = |at: usize| -> Option<u16> { let s: [u8; 2] = bytes.get(at..at.checked_add(2)?)?.try_into().ok()?; Some(if little { u16::from_le_bytes(s) } else { u16::from_be_bytes(s) }) };
    if u16_at(2)? != 42 { return None; }
    let offset: [u8; 4] = bytes.get(4..8)?.try_into().ok()?;
    let ifd = if little { u32::from_le_bytes(offset) } else { u32::from_be_bytes(offset) } as usize;
    let count = u16_at(ifd)?.min(512) as usize;
    for index in 0..count {
        let entry = ifd.checked_add(2 + index * 12)?;
        if u16_at(entry)? == 0x112 && u16_at(entry + 2)? == 3 {
            let value = u16_at(entry + 8)?; if (1..=8).contains(&value) { return Some(value as u8); }
        }
    }
    None
}
