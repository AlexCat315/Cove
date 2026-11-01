//! Translation of CoveUICoreTests/MipmapTests.swift.

use cove_core::{Bitmap, Size, image_lod};

#[test]
fn odd_dimensions_include_the_last_pixel() {
    let image = Bitmap::new(3, 1, vec![255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
    assert_eq!(image.reduced_mip().unwrap().pixels(), [170, 0, 85, 255]);
}

#[test]
fn transparent_colors_do_not_bleed() {
    let image = Bitmap::new(2, 1, vec![255, 0, 0, 0, 0, 0, 255, 255]).unwrap();
    assert_eq!(image.reduced_mip().unwrap().pixels(), [0, 0, 255, 128]);
    assert_eq!(
        image_lod(
            Size {
                width: 1920.0,
                height: 1080.0
            },
            Size {
                width: 480.0,
                height: 270.0
            },
            10
        ),
        2
    );
    assert_eq!(
        image_lod(
            Size {
                width: 1920.0,
                height: 1080.0
            },
            Size {
                width: f32::NAN,
                height: 0.0
            },
            10
        ),
        0
    );
}
