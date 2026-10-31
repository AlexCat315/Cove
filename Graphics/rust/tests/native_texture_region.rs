//! Translation of Graphics/Tests/NativeRHITests/NativeTextureRegionTests.swift.
//!
//! Upload-region validation is pure byte math on the CPU: the frontend and the
//! shared validation helpers reject an out-of-bounds origin, a zero extent, a
//! bad row pitch and an under-sized capacity before any backend is reached.
//!
//! The `patches` helper is device-only (it uploads, reads back and compares
//! pixels across formats, mips and array layers), so only the validation half is
//! ported here.

use cove_graphics::{rhi_texture_upload_bytes, TextureFormat, TextureUploadRegion};

#[test]
fn upload_bounds_reject_overflow_and_invalid_rows() {
    // (origin, size, row pitch, capacity) — every combination is rejected.
    let rejected: [([i64; 2], (i64, i64), usize, usize); 9] = [
        ([-1, 0], (1, 1), 4, 4),
        ([0, -1], (1, 1), 4, 4),
        ([i64::MAX, 0], (1, 1), 4, 4),
        ([4, 3], (1, 1), 4, 4),
        ([3, 2], (2, 1), 8, 8),
        ([0, 0], (0, 1), 4, 4),
        ([0, 0], (1, 1), 3, 4),
        ([0, 0], (1, 2), i64::MAX as usize - 3, i64::MAX as usize),
        ([1, 1], (2, 2), 8, 15),
    ];
    for (origin, (width, height), row_bytes, capacity) in rejected {
        let region = TextureUploadRegion::with_origin(width, height, origin[0], origin[1]);
        assert!(
            rhi_texture_upload_bytes(
                region,
                row_bytes,
                TextureFormat::Rgba8Unorm,
                4,
                3,
                capacity,
            )
            .is_err(),
            "origin {origin:?} size ({width}, {height}) pitch {row_bytes} capacity {capacity} must be rejected"
        );
    }

    // The bottom-right single texel of a 4x3 texture is still addressable.
    let edge = TextureUploadRegion::with_origin(1, 1, 3, 2);
    assert_eq!(
        rhi_texture_upload_bytes(edge, 8, TextureFormat::Rgba8Unorm, 4, 3, 8),
        Ok(8),
        "a padded row at the last texel is accepted"
    );
}

/// The capacity check is what turns an under-sized staging buffer into an error
/// instead of a read past its end.
#[test]
fn upload_bytes_are_the_padded_row_count() {
    let region = TextureUploadRegion::new(3, 2);
    assert_eq!(
        rhi_texture_upload_bytes(region, 16, TextureFormat::Rgba8Unorm, 4, 3, 32),
        Ok(32),
        "two padded 16-byte rows"
    );
    assert!(
        rhi_texture_upload_bytes(region, 16, TextureFormat::Rgba8Unorm, 4, 3, 31).is_err(),
        "one byte short is rejected"
    );
}

/// Depth formats have no color transfer, so they are refused outright.
#[test]
fn depth_formats_are_not_uploadable() {
    let region = TextureUploadRegion::new(1, 1);
    assert!(
        rhi_texture_upload_bytes(region, 4, TextureFormat::Depth32Float, 4, 3, 4).is_err(),
        "depth targets do not accept color uploads"
    );
}
