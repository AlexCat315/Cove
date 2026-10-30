//! Translation of `Tests/CoveUIRuntimeTests/DrawListTests.swift`.
//!
//! `DrawList` lives in `cove-core` but text emission is a `cove-text` extension,
//! so the whole file lives in the Runtime crate where both are reachable.
//!
//! Behaviour checklist:
//! - colour packs to little-endian rgba8
//! - `add_rect` emits 4 vertices and 6 indices in one batch
//! - lines emit an opaque core, a transparent fringe and rounded caps
//! - two adjacent rects merge into a single batch
//! - a backdrop request rides on its own batch, consumed by the following draw
//! - a different texture id splits batches
//! - `push_clip` / `pop_clip` nest correctly and intersect with the parent
//! - rounded fill coverage stays within half a physical pixel and preserves alpha
//! - `add_text` produces glyph quads with atlas UVs and snaps to whole pixels
//! - `reset` clears all CPU buffers
//! - a translated append moves vertices but preserves the parent scissor
//! - appending into an empty list keeps the source storage immutable

use std::path::Path;

use cove_core::{
    layout_text, Backdrop, Color, DrawList, Point, Rect, TextLayoutOptions, TextLayoutResult,
};
use cove_text::DrawListText;
use cove_text::{FontAtlas, TextShaper};

/// Cross-platform test font resolved from the host's system fonts.
fn test_font_path() -> Option<&'static str> {
    static PATH: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    *PATH.get_or_init(|| {
        const CANDIDATES: [&str; 8] = [
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ];
        CANDIDATES.into_iter().find(|candidate| Path::new(candidate).is_file())
    })
}

fn alpha(vertex: &cove_core::Vertex) -> u32 {
    vertex.color >> 24
}

#[test]
fn color_packing() {
    let color = Color::from_rgba8(0x12, 0x34, 0x56, 0x78);
    assert_eq!(color.rgba8(), 0x78563412, "R packs into the low byte");
}

#[test]
fn add_rect_geometry() {
    let mut list = DrawList::default();
    list.add_rect(Rect::new(10.0, 20.0, 30.0, 40.0), Color::WHITE)
        .expect("the rect is accepted");

    assert_eq!(list.vertices().len(), 4);
    assert_eq!(list.indices().len(), 6);
    assert_eq!(list.batches().len(), 1);
    assert_eq!(list.batches()[0].texture_id, 0);
    assert_eq!(list.batches()[0].index_count, 6);
    assert_eq!(list.batches()[0].index_offset, 0);

    // Vertices: top-left, top-right, bottom-right, bottom-left.
    assert_eq!((list.vertices()[0].x, list.vertices()[0].y), (10.0, 20.0));
    assert_eq!((list.vertices()[2].x, list.vertices()[2].y), (40.0, 60.0));
    assert_eq!(list.vertices()[0].u, -1.0, "a solid fill uses the sentinel UV");
}

#[test]
fn line_coverage() {
    let mut list = DrawList::default();
    list.pixel_scale = 2.0;
    list.add_line(Point { x: 2.0, y: 3.0 }, Point { x: 42.0, y: 23.0 }, 2.0, Color::WHITE)
        .expect("the line is accepted");

    assert!(list.vertices().len() > 4);
    assert!(list.indices().len() > 6);
    assert!(
        list.indices().iter().all(|index| (*index as usize) < list.vertices().len()),
        "every index stays in range",
    );
    assert!(list.vertices().iter().any(|vertex| alpha(vertex) == 0xFF), "an opaque core");
    assert!(list.vertices().iter().any(|vertex| alpha(vertex) == 0), "a transparent fringe");
    assert_eq!(list.batches().len(), 1);
}

#[test]
fn merge_adjacent_batches() {
    let mut list = DrawList::default();
    list.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE).expect("first rect");
    list.add_rect(Rect::new(20.0, 0.0, 10.0, 10.0), Color::BLACK).expect("second rect");

    assert_eq!(list.vertices().len(), 8);
    assert_eq!(list.indices().len(), 12);
    assert_eq!(list.batches().len(), 1, "matching draws share a batch");
    assert_eq!(list.batches()[0].index_count, 12);
}

#[test]
fn backdrop_starts_own_batch() {
    let mut list = DrawList::default();
    // The shadow behind a panel: solid colour, same texture, same scissor, and
    // exactly the kind of draw a frosted fill would otherwise merge into — which
    // would put the blur behind the shadow instead of the fill.
    let backdrop = Backdrop {
        rect: Rect::new(4.0, 4.0, 32.0, 32.0),
        corner_radius: 6.0,
        blur_radius: 12.0,
        opacity: 1.0,
    };
    list.add_rect(Rect::new(0.0, 0.0, 40.0, 40.0), Color::BLACK).expect("the shadow rect");
    list.begin_backdrop(backdrop);
    list.add_rounded_rect(Rect::new(4.0, 4.0, 32.0, 32.0), 6.0, Color::WHITE)
        .expect("the frosted fill");
    // The request is consumed: the row after the fill merges as usual.
    list.add_rect(Rect::new(4.0, 4.0, 32.0, 32.0), Color::WHITE).expect("the trailing rect");

    assert_eq!(list.batches().len(), 2);
    assert!(list.batches()[0].backdrop.is_none(), "the shadow keeps its own plain batch");
    assert_eq!(list.batches()[0].index_count, 6);
    let frosted = &list.batches()[1];
    assert_eq!(frosted.backdrop, Some(backdrop), "the request rides on the fill's batch");
    assert_eq!(frosted.index_offset, 6);
    assert_eq!(
        frosted.index_count,
        list.indices().len() as u32 - 6,
        "the rounded fill and the rect after it share one batch",
    );
}

#[test]
fn texture_splits_batches() {
    let mut list = DrawList::default();
    list.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE).expect("first rect");
    list.add_glyph_quad(
        Rect::new(20.0, 0.0, 10.0, 10.0),
        1,
        Color::WHITE,
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 1.0 },
    )
    .expect("the glyph quad");
    list.add_rect(Rect::new(40.0, 0.0, 10.0, 10.0), Color::WHITE).expect("last rect");

    assert_eq!(list.batches().len(), 3);
    assert_eq!(list.batches()[0].texture_id, 0);
    assert_eq!(list.batches()[1].texture_id, 1);
    assert_eq!(list.batches()[2].texture_id, 0);
}

#[test]
fn clip_stack() {
    let mut list = DrawList::default();
    let outer = Rect::new(0.0, 0.0, 100.0, 100.0);
    let inner = Rect::new(10.0, 10.0, 50.0, 50.0);

    list.push_clip_rect(outer);
    list.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE).expect("outer rect");
    list.push_clip_rect(inner);
    list.add_rect(Rect::new(20.0, 20.0, 5.0, 5.0), Color::WHITE).expect("inner rect");
    list.pop_clip_rect();
    list.add_rect(Rect::new(30.0, 30.0, 10.0, 10.0), Color::WHITE).expect("outer rect again");
    list.pop_clip_rect();
    list.add_rect(Rect::new(0.0, 0.0, 5.0, 5.0), Color::WHITE).expect("unclipped rect");

    let scissors: Vec<Option<Rect>> = list.batches().iter().map(|batch| batch.scissor).collect();
    assert_eq!(
        scissors,
        [Some(outer), Some(inner), Some(outer), None],
        "the clip stack nests and restores",
    );
}

#[test]
fn clip_intersection() {
    let mut list = DrawList::default();
    list.push_clip_rect(Rect::new(0.0, 0.0, 100.0, 100.0));
    list.push_clip_rect(Rect::new(50.0, 50.0, 200.0, 200.0));

    // The intersection is (50,50)-(100,100), i.e. 50x50.
    assert_eq!(
        list.current_clip(),
        Some(Rect::new(50.0, 50.0, 50.0, 50.0)),
        "a nested clip intersects its parent",
    );
}

#[test]
fn rounded_rect_geometry() {
    for scale in [1.0_f32, 1.5, 2.0, 3.0] {
        let mut list = DrawList::default();
        list.pixel_scale = scale;
        let tint = Color::from_rgba8(40, 90, 180, 128);
        list.add_rounded_rect(Rect::new(0.0, 0.0, 100.0, 60.0), 10.0, tint).expect("the fill");

        assert_eq!(list.batches().len(), 1, "scale {scale}: one batch");
        assert_eq!(list.batches()[0].index_count, list.indices().len() as u32);
        assert!(
            list.indices().iter().all(|index| (*index as usize) < list.vertices().len()),
            "scale {scale}: every index stays in range",
        );
        assert!(
            list.vertices().iter().all(|vertex| alpha(vertex) == 128 || alpha(vertex) == 0),
            "scale {scale}: source alpha is preserved exactly",
        );

        let left = list.vertices().iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
        let right = list.vertices().iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max);
        let half = 0.5 / scale;
        assert!((left + half).abs() < 0.001, "scale {scale}: left coverage is within half a pixel");
        assert!(
            (right - 100.0 - half).abs() < 0.001,
            "scale {scale}: right coverage is within half a pixel",
        );

        // No triangle interiors may overlap: a translucent fill must be painted
        // once, not made more opaque by the antialiasing implementation.
        let mut area = 0.0_f32;
        for triangle in list.indices().chunks_exact(3) {
            let [a, b, c] = [
                &list.vertices()[triangle[0] as usize],
                &list.vertices()[triangle[1] as usize],
                &list.vertices()[triangle[2] as usize],
            ];
            area += ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5;
        }
        let outer_radius = 10.0 + half;
        let expected = (100.0 + 1.0 / scale) * (60.0 + 1.0 / scale)
            - (4.0 - std::f32::consts::PI) * outer_radius * outer_radius;
        assert!(
            (area - expected).abs() < 0.5,
            "scale {scale}: covered area {area} tracks the rounded rect ({expected})",
        );
    }
}

fn shaped_hi() -> Option<(FontAtlas, TextLayoutResult)> {
    let path = test_font_path()?;
    let atlas = FontAtlas::new(512, 512);
    if !atlas.load_font_path(Path::new(path), 16.0, 1.0) {
        return None;
    }
    let mut shaper = TextShaper::default();
    shaper.set_face(atlas.font_face()?, 16.0);
    let shaped = shaper.shape("Hi", None, None, None);
    let result = layout_text(&shaped, "Hi", &atlas, TextLayoutOptions { line_height: 20.0, ..Default::default() });
    Some((atlas, result))
}

#[test]
fn add_text_geometry() {
    let Some((atlas, result)) = shaped_hi() else {
        eprintln!("SKIP add_text_geometry: no system test font available");
        return;
    };
    let mut list = DrawList::default();
    list.add_text(&result, Point { x: 0.0, y: 0.0 }, Color::WHITE, 7, Some(&atlas), None)
        .expect("the text is accepted");

    // Two visible glyphs → 8 vertices, 12 indices, one batch with texture 7.
    assert_eq!(list.vertices().len(), 8);
    assert_eq!(list.indices().len(), 12);
    assert_eq!(list.batches().len(), 1);
    assert_eq!(list.batches()[0].texture_id, 7);

    let info = atlas
        .rasterize_codepoint('H', 0)
        .expect("`H` rasterises in the primary face");
    assert_eq!(list.vertices()[0].u, info.uv_min_x, "the first quad takes the `H` UV origin");
    assert_eq!(list.vertices()[0].v, info.uv_min_y);
}

#[test]
fn add_text_pixel_snap() {
    let Some((atlas, result)) = shaped_hi() else {
        eprintln!("SKIP add_text_pixel_snap: no system test font available");
        return;
    };
    let glyph = result
        .lines
        .first()
        .and_then(|line| line.glyphs.first())
        .expect("the first glyph exists");
    let info = atlas
        .rasterize_glyph(glyph.glyph_id, glyph.font_id)
        .expect("the first glyph rasterises");

    let origin = Point { x: 0.35, y: 0.65 };
    let raw_x = origin.x + glyph.x + info.bearing_x;
    let raw_y = origin.y + glyph.y - info.bearing_y;

    let mut list = DrawList::default();
    list.add_text(&result, origin, Color::WHITE, 7, Some(&atlas), None)
        .expect("the text is accepted");

    assert_eq!(list.vertices()[0].x, raw_x.round(), "glyph quads snap to whole pixels in x");
    assert_eq!(list.vertices()[0].y, raw_y.round(), "glyph quads snap to whole pixels in y");
}

#[test]
fn reset_clears() {
    let mut list = DrawList::default();
    list.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE).expect("a rect");
    list.reset();

    assert!(list.vertices().is_empty());
    assert!(list.indices().is_empty());
    assert!(list.batches().is_empty());
    assert!(list.current_clip().is_none(), "reset clears the clip stack");
}

#[test]
fn translated_append() {
    let mut source = DrawList::default();
    source.push_clip_rect(Rect::new(0.0, 0.0, 100.0, 80.0));
    source.add_rect(Rect::new(10.0, 20.0, 30.0, 40.0), Color::WHITE).expect("the source rect");

    let mut destination = DrawList::default();
    destination.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK).expect("the leading rect");
    destination
        .append(&source, Point { x: 7.0, y: -5.0 })
        .expect("the translated append");

    assert_eq!(destination.vertices()[4].x, 17.0);
    assert_eq!(destination.vertices()[4].y, 15.0);
    assert_eq!(destination.indices()[6], 4);
    assert_eq!(destination.batches()[1].index_offset, 6);
    assert_eq!(
        destination.batches()[1].scissor,
        Some(Rect::new(0.0, 0.0, 100.0, 80.0)),
        "the translated append keeps the source scissor",
    );
}

#[test]
fn empty_append_copy_on_write() {
    let mut source = DrawList::default();
    source.add_rect(Rect::new(10.0, 20.0, 30.0, 40.0), Color::WHITE).expect("the source rect");

    let mut destination = DrawList::default();
    destination.append(&source, Point::default()).expect("the append");

    assert_eq!(destination.vertices().len(), source.vertices().len());
    assert_eq!(destination.vertices()[0].x, source.vertices()[0].x);
    assert_eq!(destination.vertices()[0].y, source.vertices()[0].y);
    assert_eq!(destination.indices(), source.indices());
    assert_eq!(destination.batches().len(), source.batches().len());

    destination.add_rect(Rect::new(50.0, 60.0, 10.0, 10.0), Color::BLACK).expect("a new rect");
    assert_eq!(destination.vertices().len(), 8, "the destination grew");
    assert_eq!(source.vertices().len(), 4, "the source stayed put");
    assert_eq!(source.indices().len(), 6);

    destination.reset();
    assert!(destination.vertices().is_empty());
    assert_eq!(source.vertices().len(), 4, "resetting the destination leaves the source intact");
    assert_eq!(source.indices().len(), 6);
}
