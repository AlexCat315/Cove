//! Translation of `Text/Tests/CoveUITextTests/FontPipelineTests.swift`.
//!
//! Behaviour checklist:
//! - rasterizing a glyph does not change its layout metrics
//! - an invalid font fails without disturbing the currently loaded face, and a
//!   shaped run stays valid while HarfBuzz references the previous face
//! - a file-backed face owns its storage after the path is deleted
//! - grapheme fallback keeps UTF-8 clusters intact, picks one face per
//!   grapheme, and preserves ligatures
//! - Arabic shapes right-to-left with joining forms
//! - Devanagari falls back to one face for the whole grapheme
//! - atlas exhaustion is reported and `reset` clears it
//! - a HiDPI raster reuses the right atlas keys and draw widths scale with it
//! - reusing cached glyphs produces an empty dirty region

use std::path::PathBuf;

use std::collections::BTreeMap;

use cove_core::Color;
use cove_core::scene::{DrawList, Point};
use cove_text::{FontAtlas, FontCollection, FontFace};

const FONTS: [&str; 5] = [
    "NotoSans.ttf",
    "NotoSansCJKsc.otf",
    "NotoSansArabic.ttf",
    "NotoSansDevanagari.ttf",
    "NotoEmoji.ttf",
];

fn font_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fonts")
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(font_directory().join(name))
        .unwrap_or_else(|error| panic!("read bundled font {name}: {error}"))
}

fn collection() -> FontCollection {
    let mut fonts = FontCollection::new(1024, 1024);
    for name in FONTS {
        assert!(
            fonts.append(bytes(name)).is_some(),
            "the bundled {name} must load"
        );
    }
    fonts
}

#[test]
fn rasterization_does_not_change_layout_metrics() {
    let mut fonts = collection();
    fonts.configure(13.0, 2.0);
    let glyphs = fonts.shape("English office AV 0123 中文");
    assert!(!glyphs.is_empty(), "the sample text shapes to glyphs");

    for glyph in glyphs {
        let before = fonts
            .atlas
            .glyph_metrics(glyph.glyph_id, glyph.font_id)
            .expect("metrics before rasterizing");
        let _ = fonts
            .atlas
            .rasterize_glyph(glyph.glyph_id, glyph.font_id)
            .expect("glyph rasterizes");
        let after = fonts
            .atlas
            .glyph_metrics(glyph.glyph_id, glyph.font_id)
            .expect("metrics after rasterizing");
        assert_eq!(
            (
                before.advance,
                before.width,
                before.height,
                before.bearing_x,
                before.bearing_y
            ),
            (
                after.advance,
                after.width,
                after.height,
                after.bearing_x,
                after.bearing_y
            ),
            "drawing must not change wrapping or caret metrics on the next layout"
        );
    }
}

#[test]
fn owned_memory_and_invalid_font_preserve_current_face() {
    let atlas = FontAtlas::new(512, 512);
    assert!(atlas.load_font_bytes(bytes("NotoSans.ttf"), 16.0, 1.0));
    let face = atlas.font_face().expect("a face is loaded");

    assert!(
        !atlas.load_font_bytes(vec![1, 2, 3], 16.0, 1.0),
        "invalid font bytes are rejected"
    );
    assert_eq!(
        atlas
            .font_face()
            .map(|current| current.post_script_name().to_owned()),
        Some(face.post_script_name().to_owned()),
        "a rejected load leaves the current face in place"
    );

    let mut shaper = cove_text::TextShaper::default();
    shaper.set_face(face.clone(), 16.0);
    let shaped = shaper.shape(
        "memory survives the temporary input array",
        None,
        None,
        None,
    );
    assert!(
        shaped.iter().all(|glyph| glyph.glyph_id != 0),
        "the face still shapes after its source buffer was dropped"
    );

    assert!(atlas.load_font_bytes(bytes("NotoSansArabic.ttf"), 16.0, 1.0));
    let shaped = shaper.shape(
        "old face remains alive while HarfBuzz references it",
        None,
        None,
        None,
    );
    assert!(
        shaped.iter().all(|glyph| glyph.glyph_id != 0),
        "the previously installed face stays alive while the shaper references it"
    );
}

#[test]
fn file_face_owns_its_storage_after_the_path_is_removed() {
    let path = std::env::temp_dir().join(format!(
        "cove-font-{}-{}.ttf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&path, bytes("NotoSans.ttf")).expect("write temporary font");

    let face = FontFace::from_path(&path, 16.0, 1.0, Default::default(), BTreeMap::new())
        .expect("load file face");
    std::fs::remove_file(&path).expect("remove the backing file");

    let glyphs = face.shape("owned file storage", None, None, None, 0, 0);
    assert!(
        !glyphs.is_empty(),
        "the face still shapes after the file is gone"
    );
    assert!(glyphs.iter().all(|glyph| glyph.glyph_id != 0));
    let first = glyphs.first().expect("at least one glyph").glyph_id;
    assert!(
        face.rasterize(first).is_some(),
        "the face can still rasterize from owned storage"
    );
}

#[test]
fn grapheme_fallback_preserves_utf8_clusters_and_ligatures() {
    let fonts = collection();
    let text = "office 你好 कि 🙂";
    let shaped = fonts.shape(text);

    assert!(!shaped.is_empty());
    assert!(
        shaped.iter().all(|glyph| glyph.glyph_id != 0),
        "every glyph comes from some face"
    );

    let faces: std::collections::BTreeSet<i64> = shaped.iter().map(|glyph| glyph.font_id).collect();
    assert_eq!(
        faces,
        std::collections::BTreeSet::from([1, 2, 5]),
        "Latin, CJK and emoji each resolve to their own face"
    );

    assert!(
        fonts.shape("office").len() < "office".len(),
        "ligatures collapse characters into fewer glyphs"
    );

    let chinese_start = "office ".len() as u32;
    assert!(
        shaped
            .iter()
            .any(|glyph| glyph.cluster == chinese_start && glyph.font_id == 2),
        "the CJK run starts at its own byte offset and uses the CJK face"
    );

    let devanagari_start = "office 你好 ".len() as u32;
    let devanagari: Vec<_> = shaped
        .iter()
        .filter(|glyph| glyph.cluster == devanagari_start)
        .collect();
    assert_eq!(
        devanagari.len(),
        2,
        "the Devanagari cluster shapes to two glyphs"
    );
    assert!(
        devanagari.iter().all(|glyph| glyph.font_id == 1),
        "one grapheme stays in one face even when another face also covers it"
    );
}

#[test]
fn arabic_auto_direction_and_joining() {
    let fonts = collection();
    let shaped = fonts.shape("سلام");

    assert!(
        shaped
            .iter()
            .all(|glyph| glyph.font_id == 3 && glyph.glyph_id != 0),
        "Arabic resolves entirely to the Arabic face"
    );
    assert!(
        shaped.first().expect("glyphs").cluster > shaped.last().expect("glyphs").cluster,
        "clusters run right to left"
    );

    let isolated: Vec<u32> = "سلام"
        .chars()
        .flat_map(|character| {
            fonts
                .shape(&character.to_string())
                .into_iter()
                .map(|glyph| glyph.glyph_id)
                .collect::<Vec<_>>()
        })
        .collect();
    let joined: Vec<u32> = shaped.iter().map(|glyph| glyph.glyph_id).collect();
    let reversed_isolated: Vec<u32> = isolated.into_iter().rev().collect();
    assert_ne!(
        joined, reversed_isolated,
        "joined forms differ from isolated forms in visual order"
    );

    let advance: f32 = shaped.iter().map(|glyph| glyph.x_advance).sum();
    assert!(advance > 0.0, "the run advances horizontally");
}

#[test]
fn devanagari_fallback_keeps_one_grapheme_in_one_face() {
    let mut fonts = FontCollection::new(1024, 1024);
    assert_eq!(fonts.append(bytes("NotoSansCJKsc.otf")), Some(1));
    assert_eq!(fonts.append(bytes("NotoSansDevanagari.ttf")), Some(2));

    let shaped = fonts.shape("कि");
    assert_eq!(shaped.len(), 2, "the cluster shapes to two glyphs");
    assert!(
        shaped
            .iter()
            .all(|glyph| glyph.font_id == 2 && glyph.glyph_id != 0 && glyph.cluster == 0),
        "the whole grapheme comes from the Devanagari face at cluster zero"
    );
}

#[test]
fn atlas_exhaustion_is_reported_and_reset_clears_it() {
    let mut fonts = FontCollection::new(16, 16);
    assert!(fonts.append(bytes("NotoSans.ttf")).is_some());

    let mut list = DrawList::default();
    let _ = fonts.draw(
        "W",
        42.0,
        1.0,
        &mut list,
        Point::new(0.0, 0.0),
        Color::WHITE,
        1,
    );
    assert!(
        fonts.atlas.is_full(),
        "a 42pt glyph cannot fit a 16x16 atlas"
    );

    fonts.atlas.reset();
    assert!(!fonts.atlas.is_full(), "reset clears the exhaustion flag");
    let payload = fonts
        .atlas
        .dirty_upload_payload()
        .expect("reset marks the whole atlas dirty");
    assert_eq!(
        payload.pixels.len(),
        256,
        "reset uploads the full 16x16 atlas"
    );
}

#[test]
fn raster_sizes_and_high_dpi_reuse_correct_atlas_keys() {
    let mut fonts = collection();
    let mut list = DrawList::default();

    let small = fonts
        .draw(
            "Hi 你好",
            16.0,
            1.0,
            &mut list,
            Point::new(0.0, 0.0),
            Color::WHITE,
            1,
        )
        .expect("small draw");
    list.reset();

    let large = fonts
        .draw(
            "Hi 你好",
            32.0,
            2.0,
            &mut list,
            Point::new(0.0, 0.0),
            Color::WHITE,
            1,
        )
        .expect("large draw");
    assert!(
        (large - small * 2.0).abs() < 3.0,
        "a 2x size at 2x raster scale doubles the advance; small={small} large={large}"
    );
    list.reset();

    let again = fonts
        .draw(
            "Hi 你好",
            16.0,
            1.0,
            &mut list,
            Point::new(0.0, 0.0),
            Color::WHITE,
            1,
        )
        .expect("repeat draw");
    assert!(
        (again - small).abs() < 0.01,
        "the 16pt key is reused exactly; expected {small}, got {again}"
    );
    assert!(
        list.batches().iter().all(|batch| batch.texture_id == 1),
        "every batch draws from the requested atlas texture"
    );
    assert!(!list.vertices().is_empty(), "glyphs produced vertices");
    assert!(
        list.vertices()
            .iter()
            .all(|vertex| vertex.x.is_finite() && vertex.y.is_finite()),
        "no glyph emits a non-finite position"
    );
}

#[test]
fn dirty_regions_become_empty_when_existing_glyphs_are_reused() {
    let mut fonts = collection();
    fonts.atlas.mark_clean();
    let mut list = DrawList::default();

    let _ = fonts.draw(
        "A",
        16.0,
        1.0,
        &mut list,
        Point::new(0.0, 0.0),
        Color::WHITE,
        1,
    );
    let first = fonts
        .atlas
        .dirty_upload_payload()
        .expect("the new glyph dirties a region");
    let region = first.region;
    assert_eq!(
        first.pixels.len(),
        region.width * region.height,
        "the payload covers exactly its dirty region"
    );
    assert!(
        first.pixels.iter().any(|value| *value > 0),
        "the dirty region contains rasterized coverage"
    );
    assert!(
        first.pixels.len() < fonts.atlas.width() * fonts.atlas.height(),
        "a single glyph does not dirty the whole atlas"
    );

    fonts.atlas.mark_clean();
    list.reset();
    let _ = fonts.draw(
        "A",
        16.0,
        1.0,
        &mut list,
        Point::new(0.0, 0.0),
        Color::WHITE,
        1,
    );
    assert!(
        fonts.atlas.dirty_upload_payload().is_none(),
        "reusing a cached glyph leaves nothing to upload"
    );

    let _ = fonts.draw(
        "字",
        16.0,
        1.0,
        &mut list,
        Point::new(20.0, 0.0),
        Color::WHITE,
        1,
    );
    assert!(
        fonts.atlas.dirty_upload_payload().is_some(),
        "a newly rasterized glyph dirties a region again"
    );
}
