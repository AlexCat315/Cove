//! Translation of `Tests/CoveUIComposeTests/ColorGlyphResampleTests.swift`.
//!
//! Behaviour checklist:
//! - box resampling averages coverage and weights colour by alpha
//! - a face without colour tables reports no colour source

use crate::support;

use cove_text::{FontAtlas, FreeTypeColorGlyphFace};

#[test]
fn resample_averages_coverage_and_weights_color_by_alpha() {
    // 2x2: opaque red, transparent black, opaque red, transparent black -> 1x1.
    let pixels: [u8; 16] = [255, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0];
    let result =
        FreeTypeColorGlyphFace::resample(&pixels, 2, 2, 1, 1).expect("a 2x2 RGBA source is valid");
    assert_eq!(
        result,
        vec![255, 0, 0, 127],
        "half the pixels are covered, so alpha averages to 127 while the hue stays red"
    );
}

#[test]
fn monochrome_face_has_no_color_source() {
    let atlas = FontAtlas::new(64, 64);
    let Some(path) = support::system_test_font_path() else {
        // Without a host font there is no face to probe; the assertion is about
        // colour tables, which only exist on a loaded face.
        return;
    };
    atlas.load_font_path(std::path::Path::new(path), 16.0, 1.0);
    let Some(face) = atlas.font_face() else {
        return;
    };
    assert!(
        !face.has_color_source(),
        "the host test font is monochrome, so it exposes no colour source"
    );
}
