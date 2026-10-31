//! Translation of Graphics/Tests/NativeRHITests/NativeTextureCopyTests.swift.
//!
//! Color-copy extent validation is CPU-side: a copy must be positive, fit both
//! textures, keep the same format and use a color (not depth) texture.
//!
//! The `copy` helper is device-only (it chains two blits and compares pixels),
//! so only the validation half is ported here.

use cove_graphics::{rhi_color_texture_copy_extent, TextureCopyExtent, TextureFormat};

fn extent(width: usize, height: usize, format: TextureFormat) -> TextureCopyExtent {
    TextureCopyExtent { width, height, format }
}

#[test]
fn color_copy_rejects_out_of_range_and_mismatched_formats() {
    let source = extent(4, 4, TextureFormat::Rgba16Float);
    let destination = extent(6, 6, TextureFormat::Rgba16Float);
    assert!(
        rhi_color_texture_copy_extent(3, 2, source, destination).is_ok(),
        "a 3x2 copy fits the smaller texture"
    );

    for (width, height) in [(0, 1), (1, 0), (5, 1), (1, 5)] {
        assert!(
            rhi_color_texture_copy_extent(width, height, source, destination).is_err(),
            "a {width}x{height} copy is outside the source extent"
        );
    }
    // A negative extent cannot reach this API: the parameters are `usize`, so a
    // negative Swift literal is rejected by the type system instead.
    assert!(
        rhi_color_texture_copy_extent(1, 1, source, extent(6, 6, TextureFormat::Rgba8Unorm)).is_err(),
        "mismatched formats are rejected"
    );
    assert!(
        rhi_color_texture_copy_extent(
            1,
            1,
            extent(4, 4, TextureFormat::Depth32Float),
            extent(4, 4, TextureFormat::Depth32Float)
        )
        .is_err(),
        "depth textures are not color-copy sources"
    );
}

/// A copy also cannot exceed the destination, even when the source is larger.
#[test]
fn copy_is_bounded_by_the_destination() {
    let source = extent(8, 8, TextureFormat::Rgba8Unorm);
    let destination = extent(2, 2, TextureFormat::Rgba8Unorm);
    assert_eq!(rhi_color_texture_copy_extent(2, 2, source, destination), Ok(()));
    assert!(rhi_color_texture_copy_extent(3, 2, source, destination).is_err());
    assert!(rhi_color_texture_copy_extent(2, 3, source, destination).is_err());
}
