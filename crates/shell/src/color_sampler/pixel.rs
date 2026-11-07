//! Pure pixel decoding. Kept free of native types so the Windows and X11
//! backends share it and the conversions can be tested on any host.

use cove_platform::ScreenColor;

/// Win32 `COLORREF` is `0x00BBGGRR`; alpha is always opaque.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn decode_colorref(color: u32) -> ScreenColor {
    ScreenColor::from_rgba8(
        (color & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        ((color >> 16) & 0xFF) as u8,
        255,
    )
}

/// Extract an sRGB pixel from an X11 `XImage` value using the image's real
/// channel masks. Handles 565, 888, 10-bit and other TrueColor layouts without
/// assuming R is always at bit 16.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn decode_masked_pixel(pixel: u64, masks: [u64; 3]) -> ScreenColor {
    let channel = |mask: u64| -> u8 {
        if mask == 0 {
            return 0;
        }
        let shift = mask.trailing_zeros();
        let max = mask >> shift;
        let value = (pixel & mask) >> shift;
        // Scale the channel's native range (e.g. 31 for 565 green) to 0…255.
        ((value * 255 + max / 2) / max).min(255) as u8
    };
    ScreenColor::from_rgba8(channel(masks[0]), channel(masks[1]), channel(masks[2]), 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win32_colorref_is_bgr_and_opaque() {
        assert_eq!(
            decode_colorref(0x00_11_22_33),
            ScreenColor::from_rgba8(0x33, 0x22, 0x11, 255)
        );
        assert_eq!(
            decode_colorref(0x00_FF_00_00),
            ScreenColor::from_rgba8(0, 0, 255, 255)
        );
    }

    #[test]
    fn x11_masks_decode_888_and_565_layouts() {
        let rgb888 = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF];
        assert_eq!(
            decode_masked_pixel(0x12_34_56, rgb888),
            ScreenColor::from_rgba8(0x12, 0x34, 0x56, 255)
        );
        let rgb565 = [0xF800, 0x07E0, 0x001F];
        assert_eq!(
            decode_masked_pixel(0xF800, rgb565),
            ScreenColor::from_rgba8(255, 0, 0, 255)
        );
        assert_eq!(
            decode_masked_pixel(0x07E0, rgb565),
            ScreenColor::from_rgba8(0, 255, 0, 255)
        );
        assert_eq!(
            decode_masked_pixel(0x001F, rgb565),
            ScreenColor::from_rgba8(0, 0, 255, 255)
        );
    }

    #[test]
    fn x11_missing_masks_degrade_to_zero_rather_than_garbage() {
        assert_eq!(
            decode_masked_pixel(u64::MAX, [0, 0, 0]),
            ScreenColor::from_rgba8(0, 0, 0, 255)
        );
    }
}
