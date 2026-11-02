//! Translation of `Tests/CoveUIRuntimeTests/ColorGlyphTests.swift`.
//!
//! Colour emoji go through a second, RGBA atlas: shaping must keep a whole
//! emoji sequence in one positioned glyph (one cluster, one advance), measuring
//! must never allocate or dirty a colour texture, and only a real raster pass
//! touches it. Getting this wrong drops a ZWJ family or a flag into separate
//! tofu boxes.
//!
//! Behaviour checklist:
//! - ZWJ, skin tone, regional flag and variation sequences remain single
//!   positioned glyphs
//! - mixed text keeps global UTF-8 clusters and separates alpha and colour draw
//!   batches
//! - HiDPI increases physical resolution without doubling the logical advance

use cove_core::scene::DrawList;
use cove_core::{Color, GlyphAtlasFormat, Point, TextureId, color_glyph_atlas_id};

/// The texture the caller already binds; a colour glyph redirects to a sibling.
const TEXTURE_ID: TextureId = 1;
use cove_text::{FontAtlas, FontProvider, ShapedGlyph, SystemFontDefaults};

fn pipeline(size: f32, scale: f32) -> (FontProvider, FontAtlas) {
    let mut provider = FontProvider::new(size, scale);
    provider
        .load_primary_font(
            &SystemFontDefaults::primary_font_name(),
            cove_core::FontWeight::Regular,
            false,
        )
        .expect("the system primary font loads");
    (provider, FontAtlas::new(256, 256))
}

fn first_glyph(glyphs: &[ShapedGlyph]) -> &ShapedGlyph {
    glyphs.first().expect("the sequence shapes to a glyph")
}

#[test]
fn zwj_skin_tone_regional_flag_and_variation_sequences_remain_single_positioned_glyphs() {
    for text in [
        "\u{1F642}",
        "\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        "\u{1F1E8}\u{1F1F3}",
        "\u{2764}\u{FE0F}",
    ] {
        let (mut provider, atlas) = pipeline(18.0, 2.0);
        let run = provider
            .resolve_runs(text)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{text:?} resolves to a run"));
        assert!(
            run.font.color_source().is_some(),
            "{text:?} resolves to a colour glyph font",
        );
        let shaped = provider.shape_run(&run);
        assert_eq!(shaped.len(), 1, "{text:?} stays one positioned glyph");
        let glyph = *first_glyph(&shaped);
        assert!(
            glyph.glyph_id != 0 && glyph.cluster == 0 && glyph.x_advance > 12.0,
            "{text:?} shaped to {glyph:?}",
        );

        provider.register_all_fonts(&atlas);
        let metrics = atlas
            .glyph_metrics(glyph.glyph_id, glyph.font_id)
            .unwrap_or_else(|| panic!("{text:?} has atlas metrics"));
        assert!(
            metrics.width > 12.0 && metrics.height > 12.0,
            "{text:?} metrics were {metrics:?}",
        );
        assert!((metrics.advance - glyph.x_advance).abs() < 0.01);

        // Measuring the font never allocates or dirties a colour texture.
        assert!(
            !atlas.is_dirty() && atlas.color_dirty_upload_payload().is_none(),
            "{text:?} measured without touching the atlas",
        );

        let info = atlas
            .rasterize_glyph(glyph.glyph_id, glyph.font_id)
            .unwrap_or_else(|| panic!("{text:?} rasterizes"));
        assert_eq!(info.format, GlyphAtlasFormat::Color);
        assert_eq!(info.width, metrics.width);
        assert_eq!(info.bearing_y, metrics.bearing_y);

        let payload = atlas
            .color_dirty_upload_payload()
            .unwrap_or_else(|| panic!("{text:?} dirtied a colour upload"));
        assert!(
            atlas.is_dirty() && atlas.dirty_upload_payload().is_none(),
            "a colour raster dirties the colour atlas only",
        );
        assert_eq!(
            payload.pixels.len(),
            payload.region.width * payload.region.height * 4,
        );
        let visible: Vec<usize> = (0..payload.pixels.len())
            .step_by(4)
            .filter(|offset| payload.pixels[offset + 3] > 0)
            .collect();
        assert!(!visible.is_empty(), "{text:?} rasterized no visible pixels");
        assert!(
            visible.iter().any(|offset| {
                (payload.pixels[*offset] as i32 - payload.pixels[*offset + 2] as i32).abs() > 30
            }),
            "{text:?} rasterized without any colour variation",
        );
        assert_eq!(payload.pixels[3], 0);

        atlas.mark_clean();
        let _ = atlas.rasterize_glyph(glyph.glyph_id, glyph.font_id);
        assert!(!atlas.is_dirty(), "{text:?} reuses its cached raster");

        atlas.reset();
        assert!(
            atlas
                .cached_glyph_info(glyph.glyph_id, glyph.font_id)
                .is_none(),
            "reset clears the glyph cache for {text:?}",
        );
        assert!(
            atlas
                .color_dirty_upload_payload()
                .is_some_and(|payload| payload.pixels.iter().all(|byte| *byte == 0)),
            "reset zeroes the colour upload for {text:?}",
        );
        let _ = atlas.rasterize_glyph(glyph.glyph_id, glyph.font_id);
        assert!(
            atlas
                .color_dirty_upload_payload()
                .is_some_and(|payload| payload.pixels.iter().any(|byte| *byte != 0)),
            "a raster after reset republishes colour pixels for {text:?}",
        );
    }
}

#[test]
fn mixed_text_keeps_global_utf8_clusters_and_separates_alpha_and_color_draw_batches() {
    let (mut provider, atlas) = pipeline(18.0, 2.0);
    let text = "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}\u{4E2D}\u{1F642}B";
    let glyphs: Vec<ShapedGlyph> = provider
        .resolve_runs(text)
        .iter()
        .flat_map(|run| provider.shape_run(run))
        .collect();

    assert_eq!(
        glyphs.iter().map(|glyph| glyph.cluster).collect::<Vec<_>>(),
        vec![0, 1, 16, 19, 23],
        "clusters stay global UTF-8 offsets across script runs",
    );
    assert!(
        glyphs
            .iter()
            .all(|glyph| glyph.x_advance > 0.0 && glyph.glyph_id != 0),
        "every glyph advances and has a real index",
    );

    provider.register_all_fonts(&atlas);
    let mut list = DrawList::default();
    let tint = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 0.5,
    };
    for glyph in &glyphs {
        let info = atlas
            .rasterize_glyph(glyph.glyph_id, glyph.font_id)
            .unwrap_or_else(|| panic!("{glyph:?} rasterizes"));
        list.add_atlas_glyph(info, Point { x: 0.0, y: 0.0 }, tint, TEXTURE_ID)
            .expect("the glyph quad fits the list");
    }

    let expected_color = color_glyph_atlas_id(TEXTURE_ID);
    assert_eq!(
        list.batches()
            .iter()
            .map(|batch| batch.texture_id)
            .collect::<Vec<_>>(),
        vec![
            TEXTURE_ID,
            expected_color,
            TEXTURE_ID,
            expected_color,
            TEXTURE_ID,
        ],
        "alpha and colour glyphs land in separate, alternating batches",
    );
    assert!(
        atlas.dirty_upload_payload().is_some() && atlas.color_dirty_upload_payload().is_some(),
        "both atlases were touched",
    );
    assert_eq!(list.vertices()[0].color, tint.rgba8());
    assert_eq!(
        list.vertices()[4].color,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.5
        }
        .rgba8(),
        "a colour glyph keeps its alpha but takes white as the tint",
    );
}

#[test]
fn hidpi_increases_physical_resolution_without_doubling_the_logical_advance() {
    fn measure(size: f32, scale: f32) -> (f32, f32, f32) {
        let (mut provider, atlas) = pipeline(size, scale);
        let run = provider
            .resolve_runs("\u{1F642}")
            .into_iter()
            .next()
            .expect("the emoji resolves to a run");
        let glyph = *first_glyph(&provider.shape_run(&run));
        provider.register_all_fonts(&atlas);
        let info = atlas
            .rasterize_glyph(glyph.glyph_id, glyph.font_id)
            .expect("the emoji rasterizes");
        (
            glyph.x_advance,
            info.width,
            (info.uv_max_x - info.uv_min_x) * atlas.width() as f32,
        )
    }

    let base = measure(18.0, 1.0);
    let retina = measure(18.0, 2.0);
    assert!(
        (base.0 - retina.0).abs() < 0.1 && (base.1 - retina.1).abs() <= 2.0,
        "the logical advance and bitmap width stay put: {base:?} vs {retina:?}",
    );
    assert!(
        retina.2 > base.2 * 1.6,
        "retina covers more atlas pixels: {} vs {}",
        retina.2,
        base.2,
    );

    let large = measure(36.0, 2.0);
    // The system bitmap font uses optical sizes; doubling the point size need
    // not double its advance exactly.
    assert!(
        large.0 > retina.0 * 1.7 && large.0 < retina.0 * 2.3,
        "a 36pt emoji advances about twice a 18pt one: {} vs {}",
        large.0,
        retina.0,
    );
}
