//! Translation of `Text/Tests/CoveUITextTests/ColorGlyphAtlasTests.swift`.
//!
//! Behaviour checklist:
//! - metrics stay lazy: asking for metrics does not dirty the atlas, and
//!   re-rasterizing an already-packed glyph does not dirty it either
//! - a rasterized colour glyph publishes exactly its own pixels, and the alpha
//!   plane stays untouched
//! - the RGBA region preserves rows and the transparent shelf spacing between
//!   two glyphs, then reports only the new shelf for the next insert
//! - `reset` clears both planes and the exhaustion state
//! - invalid RGBA data does not consume shelf space

use cove_core::GlyphMetrics;
use cove_text::{ColorGlyphAtlas, ColorGlyphBitmap};

fn bitmap(index: u32, size: usize) -> ColorGlyphBitmap {
    let metrics = GlyphMetrics {
        glyph_index: index,
        width: size as f32,
        height: size as f32,
        bearing_x: 0.0,
        bearing_y: size as f32,
        advance: (size + 1) as f32,
    };
    let mut pixels = Vec::with_capacity(size * size * 4);
    for _ in 0..(size * size) {
        pixels.extend_from_slice(&[index as u8, 20, 30, 128]);
    }
    ColorGlyphBitmap {
        pixels,
        width: size,
        height: size,
        metrics,
    }
}

#[test]
fn rgba_region_preserves_rows_and_transparent_shelf_spacing() {
    let mut plane = ColorGlyphAtlas::new(8, 8);
    plane.insert(&bitmap(1, 2)).expect("first glyph fits");
    plane.insert(&bitmap(2, 2)).expect("second glyph fits");

    let payload = plane.payload().expect("both inserts dirtied one region");
    assert_eq!(
        payload.region.width, 5,
        "two 2px cells plus the 1px shelf gap"
    );
    assert_eq!(payload.region.height, 2);
    // Row layout: glyph 1, the transparent shelf column, then glyph 2.
    let row: [u8; 20] = [
        1, 20, 30, 128, 1, 20, 30, 128, 0, 0, 0, 0, 2, 20, 30, 128, 2, 20, 30, 128,
    ];
    let mut expected = Vec::with_capacity(40);
    expected.extend_from_slice(&row);
    expected.extend_from_slice(&row);
    assert_eq!(
        payload.pixels, expected,
        "rows are copied verbatim, gap included"
    );

    plane.mark_clean();
    plane
        .insert(&bitmap(3, 2))
        .expect("third glyph opens a new shelf");
    let patch = plane.payload().expect("the new shelf is dirty");
    assert_eq!(patch.region.y, 3, "the second shelf starts below the first");
    assert_eq!(
        patch.pixels,
        bitmap(3, 2).pixels,
        "only the new glyph is republished"
    );
}

#[test]
fn invalid_rgba_data_does_not_consume_shelf_space() {
    let mut plane = ColorGlyphAtlas::new(8, 8);
    let valid = bitmap(1, 2);
    let mismatched = ColorGlyphBitmap {
        pixels: vec![0],
        width: 2,
        height: 2,
        metrics: valid.metrics,
    };
    assert!(
        plane.insert(&mismatched).is_none(),
        "a 1-byte buffer cannot describe a 2x2 RGBA glyph"
    );
    assert!(
        plane.payload().is_none(),
        "a rejected glyph dirties nothing"
    );

    let first = plane.insert(&valid).expect("the valid glyph still fits");
    assert_eq!(
        first.uv_min_x, 0.0,
        "the rejected insert left the shelf at the origin"
    );
    assert!(!plane.is_full);
}

#[test]
fn reset_clears_both_planes_and_exhaustion_state() {
    let mut plane = ColorGlyphAtlas::new(4, 4);
    // The packer leaves a one-pixel gutter after every cell, so a 2x2 glyph at
    // the origin consumes x = 0..3 and the next 2x2 cannot share that shelf:
    // 2 + 2 + 1 > 4 forces a new shelf at y = 3, where 3 + 2 + 1 > 4 also fails.
    // A 4x4 plane therefore holds exactly one 2x2 glyph.
    plane.insert(&bitmap(1, 2)).expect("first glyph fits");
    assert!(
        plane.insert(&bitmap(2, 2)).is_none(),
        "the gutter leaves no room for a second 2x2 glyph"
    );
    assert!(plane.is_full);

    plane.reset();
    assert!(!plane.is_full, "reset clears the exhaustion state");
    let payload = plane.payload().expect("reset publishes a full clear");
    assert_eq!(
        payload.pixels.len(),
        4 * 4 * 4,
        "the whole RGBA plane is republished"
    );
    assert!(
        payload.pixels.iter().all(|byte| *byte == 0),
        "the plane reads back zeroed"
    );
    assert!(
        plane.insert(&bitmap(3, 2)).is_some(),
        "the plane accepts glyphs again"
    );
}
