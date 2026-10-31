//! Translation of `Tests/CoveUIRuntimeTests/TextTests.swift`.
//!
//! The atlas is the only place glyph bitmaps exist, so its packing, caching,
//! reset and HiDPI behaviour decide whether text is legible or a mosaic of
//! overlapping rectangles. Shaping then has to agree with the atlas on advance
//! widths, and layout has to read metrics without rasterizing anything.
//!
//! Behaviour checklist:
//! - `FontAtlas` loads a real system font
//! - rasterizing ASCII `A` produces a non-empty bitmap
//! - rasterizing the same glyph twice returns the cached result
//! - the atlas packs multiple glyphs without UV overlap
//! - `reset` clears data and cache
//! - a HiDPI atlas keeps logical metrics while increasing bitmap resolution
//! - `TextShaper` shapes `Hello` into five glyphs
//! - shaping CJK works through the CoreText fallback
//! - shaped advances accumulate correctly
//! - HiDPI shaping preserves logical advances
//! - `FontProvider` keeps the macOS system primary font during fallback
//!   resolution
//! - a bootstrapped system-font environment shapes CJK through fallback
//! - `FontProvider` shapes CJK through fallback for demo-sized HiDPI text
//! - `FontProvider` routes CJK to a fallback face when one is installed
//! - fallback preserves unsupported and joined clusters
//! - single-line layout for short text
//! - multi-line layout wraps at `maxWidth`
//! - multi-line baselines stay stable across different glyph shapes
//! - layout reads metrics without rasterizing glyph bitmaps
//! - an explicit newline forces a line break
//!
//! Difference from Swift: `ShapedGlyph` carries no per-run `colorGlyphFont`;
//! the Rust equivalent is `ManagedFont::color_source()`, which these use
//! instead. `fontFace` is `FontAtlas::font_face()` and `atlasData` is
//! `FontAtlas::alpha_pixels()`.

use std::path::PathBuf;

use cove_core::{FontWeight, ShapedGlyph, TextLayoutOptions, layout_text};
use cove_text::{FontAtlas, FontProvider, SystemFontDefaults, TextShaper};

/// Resolves a real font file from the host's system font directories. CoveUI no
/// longer bundles a font, so tests use whatever sans the platform ships; the
/// assertions here are font-agnostic.
fn system_test_font_path() -> PathBuf {
    [
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.exists())
    .expect("the host ships a usable sans font")
}

fn atlas_with_font(size: f32, scale: f32) -> FontAtlas {
    let atlas = FontAtlas::new(512, 512);
    assert!(
        atlas.load_font_path(&system_test_font_path(), size, scale),
        "the system font loads at {size}pt x{scale}"
    );
    atlas
}

#[test]
fn font_atlas_initializes_and_loads_a_font() {
    let atlas = atlas_with_font(16.0, 1.0);
    assert!(atlas.font_face().is_some(), "the face is live");
}

#[test]
fn rasterize_ascii_a_produces_a_non_empty_bitmap() {
    let atlas = atlas_with_font(24.0, 1.0);
    let info = atlas
        .rasterize_codepoint('A', 0)
        .expect("'A' rasterizes");
    assert!(info.width > 0.0, "width was {}", info.width);
    assert!(info.height > 0.0, "height was {}", info.height);
    assert!(info.advance > 0.0, "advance was {}", info.advance);
}

#[test]
fn rasterizing_the_same_glyph_twice_returns_the_cached_result() {
    let atlas = atlas_with_font(16.0, 1.0);
    let first = atlas.rasterize_codepoint('B', 0).expect("'B' rasterizes");
    let second = atlas.rasterize_codepoint('B', 0).expect("'B' rasterizes again");
    assert_eq!(first.uv_min_x, second.uv_min_x, "the cached quad is reused");
    assert_eq!(first.uv_min_y, second.uv_min_y, "the cached quad is reused");
}

#[test]
fn the_atlas_packs_multiple_glyphs_without_uv_overlap() {
    let atlas = atlas_with_font(20.0, 1.0);
    let mut infos = Vec::new();
    for codepoint in 33u8..=126 {
        if let Some(info) = atlas.rasterize_codepoint(codepoint as char, 0) {
            infos.push(info);
        }
    }
    assert!(
        infos.len() > 80,
        "printable ASCII mostly packs, got {}",
        infos.len()
    );

    for (index, a) in infos.iter().enumerate() {
        for b in &infos[index + 1..] {
            let no_overlap = a.uv_max_x <= b.uv_min_x
                || b.uv_max_x <= a.uv_min_x
                || a.uv_max_y <= b.uv_min_y
                || b.uv_max_y <= a.uv_min_y;
            assert!(
                no_overlap,
                "glyph {} overlaps with glyph {}",
                a.glyph_index, b.glyph_index
            );
        }
    }
}

#[test]
fn reset_clears_data_and_cache() {
    let atlas = atlas_with_font(16.0, 1.0);
    let glyph = atlas.rasterize_codepoint('A', 0).expect("'A' rasterizes");
    assert!(
        atlas
            .alpha_pixels()
            .iter()
            .any(|byte| *byte != 0),
        "the atlas held rasterized coverage"
    );

    atlas.reset();

    assert!(
        atlas.alpha_pixels().iter().all(|byte| *byte == 0),
        "reset zeroes the atlas data"
    );
    assert!(
        atlas.cached_glyph_info(glyph.glyph_index, 0).is_none(),
        "reset clears the glyph cache"
    );
}

#[test]
fn a_hidpi_atlas_keeps_logical_metrics_while_increasing_bitmap_resolution() {
    let base = atlas_with_font(24.0, 1.0);
    let hidpi = FontAtlas::new(1024, 1024);
    assert!(hidpi.load_font_path(&system_test_font_path(), 24.0, 2.0));

    let base_info = base.rasterize_codepoint('A', 0).expect("base 'A'");
    let hidpi_info = hidpi.rasterize_codepoint('A', 0).expect("hidpi 'A'");

    let base_physical = (base_info.uv_max_x - base_info.uv_min_x) * base.width() as f32;
    let hidpi_physical = (hidpi_info.uv_max_x - hidpi_info.uv_min_x) * hidpi.width() as f32;

    assert!(
        (base_info.width - hidpi_info.width).abs() < 2.0,
        "logical width stays put: {} vs {}",
        base_info.width,
        hidpi_info.width
    );
    assert!(
        (base_info.advance - hidpi_info.advance).abs() < 2.0,
        "logical advance stays put: {} vs {}",
        base_info.advance,
        hidpi_info.advance
    );
    assert!(
        hidpi_physical > base_physical,
        "retina covers more atlas pixels: {hidpi_physical} vs {base_physical}"
    );
}

#[test]
fn text_shaper_shapes_hello_into_five_glyphs() {
    let atlas = atlas_with_font(16.0, 1.0);
    let mut shaper = TextShaper::default();
    shaper.set_face(atlas.font_face().expect("a loaded face"), 16.0);

    let glyphs = shaper.shape_latin("Hello");
    assert_eq!(glyphs.len(), 5, "five characters, five glyphs");
    for glyph in &glyphs {
        assert!(glyph.x_advance > 0.0, "{glyph:?} has a positive advance");
    }
}

#[test]
fn text_shaper_shapes_cjk_text() {
    let atlas = FontAtlas::new(512, 512);
    assert!(
        atlas.load_font_path(&PathBuf::from("/System/Library/Fonts/SFNS.ttf"), 16.0, 1.0),
        "the system CJK-capable font loads"
    );
    let mut shaper = TextShaper::default();
    shaper.set_face(atlas.font_face().expect("a loaded face"), 16.0);

    let glyphs = shaper.shape_latin("\u{4F60}\u{597D}");
    assert_eq!(glyphs.len(), 2, "two ideographs, two glyphs");
}

#[test]
fn text_shaper_advances_accumulate_correctly() {
    let atlas = atlas_with_font(16.0, 1.0);
    let mut shaper = TextShaper::default();
    shaper.set_face(atlas.font_face().expect("a loaded face"), 16.0);

    let glyphs = shaper.shape_latin("AB");
    assert_eq!(glyphs.len(), 2);
    let total: f32 = glyphs.iter().map(|glyph| glyph.x_advance).sum();
    assert!(total > 0.0, "the run advances, got {total}");
}

#[test]
fn hidpi_shaping_preserves_logical_advances() {
    let base_atlas = atlas_with_font(16.0, 1.0);
    let hidpi_atlas = FontAtlas::new(1024, 1024);
    assert!(hidpi_atlas.load_font_path(&system_test_font_path(), 16.0, 2.0));

    let mut base_shaper = TextShaper::default();
    base_shaper.set_face(base_atlas.font_face().expect("a loaded face"), 16.0);
    let mut hidpi_shaper = TextShaper::default();
    hidpi_shaper.set_face(hidpi_atlas.font_face().expect("a loaded face"), 16.0);

    let base: f32 = base_shaper
        .shape_latin("Hello")
        .iter()
        .map(|glyph| glyph.x_advance)
        .sum();
    let hidpi: f32 = hidpi_shaper
        .shape_latin("Hello")
        .iter()
        .map(|glyph| glyph.x_advance)
        .sum();
    assert!(
        (base - hidpi).abs() < 2.0,
        "retina does not change the logical advance: {base} vs {hidpi}"
    );
}

fn provider(size: f32, scale: f32) -> FontProvider {
    FontProvider::new(size, scale, 1)
}

fn load_primary(provider: &mut FontProvider) -> String {
    provider
        .load_primary_font(
            &SystemFontDefaults::primary_font_name(),
            FontWeight::Regular,
            false,
        )
        .expect("the primary font loads")
        .post_script_name
        .clone()
}

#[test]
fn font_provider_keeps_the_macos_system_primary_font_during_fallback_resolution() {
    let mut provider = provider(13.0, 1.0);
    let primary = load_primary(&mut provider);

    let runs = provider.resolve_runs("Scene Hierarchy");
    assert!(!runs.is_empty(), "Latin text resolves to runs");
    assert!(
        runs.iter()
            .all(|run| run.font.post_script_name == primary),
        "pure Latin text stays on the primary face",
    );
}

#[test]
fn bootstrapped_system_font_environment_shapes_cjk_through_fallback() {
    let mut provider = provider(13.0, 1.0);
    let primary = load_primary(&mut provider);
    let runs = provider.resolve_runs("\u{4F60}");
    let glyphs: Vec<ShapedGlyph> = runs.iter().flat_map(|run| provider.shape_run(run)).collect();

    assert!(!primary.is_empty(), "the primary font loads");
    assert!(!runs.is_empty(), "CJK text resolves to a run");
    assert!(!glyphs.is_empty(), "CJK text shapes to glyphs");
    assert!(glyphs[0].x_advance > 0.0, "the glyph advances");
}

#[test]
fn font_provider_shapes_cjk_through_fallback_for_demo_sized_hidpi_text() {
    let mut provider = provider(14.0, 2.0);
    let primary = load_primary(&mut provider);
    let runs = provider.resolve_runs("\u{4F60}");
    let glyphs: Vec<ShapedGlyph> = runs.iter().flat_map(|run| provider.shape_run(run)).collect();

    assert!(!runs.is_empty(), "CJK text resolves to a run");
    assert_ne!(
        runs[0].font.post_script_name, primary,
        "CJK does not stay on the Latin primary face",
    );
    assert!(!glyphs.is_empty(), "CJK text shapes to glyphs");
    assert_ne!(glyphs[0].glyph_id, 0, "the glyph is real, not tofu");
    assert!(glyphs[0].x_advance > 0.0, "the glyph advances");
}

#[test]
fn font_provider_routes_cjk_to_a_fallback_face_when_one_is_installed() {
    let mut provider = provider(14.0, 2.0);
    let primary = load_primary(&mut provider);

    let runs = provider.resolve_runs("Hello \u{4F60}\u{597D}");
    assert!(!runs.is_empty(), "text is never dropped");
    assert_eq!(
        runs.iter().map(|run| run.text.as_str()).collect::<String>(),
        "Hello \u{4F60}\u{597D}",
        "the runs reconstruct the source text",
    );

    // When a CJK-capable fallback is available the CJK run resolves to a
    // different face than the Latin primary and shapes to real glyphs.
    let cjk_run = runs
        .iter()
        .find(|run| run.text.contains('\u{4F60}'))
        .expect("the CJK text has its own run");
    if cjk_run.font.post_script_name != primary {
        let glyphs = provider.shape_run(cjk_run);
        assert!(!glyphs.is_empty(), "the CJK run shapes");
        assert!(
            glyphs.iter().any(|glyph| glyph.glyph_id != 0),
            "the CJK run shapes to real glyphs",
        );
        assert!(glyphs[0].x_advance > 0.0, "the glyph advances");
    }
}

#[test]
fn fallback_preserves_unsupported_and_joined_clusters_instead_of_dropping_their_source_offsets() {
    let mut provider = provider(14.0, 2.0);
    assert!(!load_primary(&mut provider).is_empty(), "the primary font loads");
    let text = "A\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}C\u{10FFFF}Z";
    let runs = provider.resolve_runs(text);
    assert_eq!(
        runs.iter().map(|run| run.text.as_str()).collect::<String>(),
        text,
        "no run lost or reordered any character",
    );

    let mut offset = 0u32;
    for run in &runs {
        assert_eq!(
            run.utf8_offset, offset,
            "run {:?} starts at {offset}",
            run.text
        );
        offset += run.text.len() as u32;
    }
    assert_eq!(
        offset,
        text.len() as u32,
        "the runs cover the whole UTF-8 buffer",
    );
}

fn shaped_for(text: &str) -> (FontAtlas, Vec<ShapedGlyph>) {
    let atlas = atlas_with_font(16.0, 1.0);
    let mut shaper = TextShaper::default();
    shaper.set_face(atlas.font_face().expect("a loaded face"), 16.0);
    let glyphs = shaper.shape_latin(text);
    (atlas, glyphs)
}

fn options(max_width: f32, line_height: f32) -> TextLayoutOptions {
    TextLayoutOptions {
        max_width,
        line_height,
        ..TextLayoutOptions::default()
    }
}

#[test]
fn single_line_layout_for_short_text() {
    let (atlas, shaped) = shaped_for("Hello");
    let result = layout_text(&shaped, "Hello", &atlas, options(f32::INFINITY, 20.0));

    assert_eq!(result.lines.len(), 1, "one line");
    assert_eq!(result.lines[0].glyphs.len(), 5, "five glyphs on it");
    assert!(result.total_width > 0.0, "width was {}", result.total_width);
    assert_eq!(result.total_height, 20.0);
}

#[test]
fn multi_line_layout_wraps_at_max_width() {
    let text = "The quick brown fox jumps over the lazy dog";
    let (atlas, shaped) = shaped_for(text);
    let result = layout_text(&shaped, text, &atlas, options(100.0, 20.0));

    assert!(result.lines.len() > 1, "the sentence wraps");
    for line in &result.lines {
        assert!(line.width < 120.0, "line {:?} was {} wide", line, line.width);
    }
}

#[test]
fn multi_line_baselines_stay_stable_across_different_glyph_shapes() {
    let text = "HI\ngj";
    let (atlas, shaped) = shaped_for(text);
    let result = layout_text(&shaped, text, &atlas, options(f32::INFINITY, 20.0));

    assert_eq!(result.lines.len(), 2, "the newline splits the text");
    let step = result.lines[1].baseline_y - result.lines[0].baseline_y;
    assert!(
        (step - 20.0).abs() < 0.5,
        "the baseline step matches the line height, got {step}",
    );
}

#[test]
fn layout_reads_metrics_without_rasterizing_glyph_bitmaps() {
    let (atlas, shaped) = shaped_for("AB");
    atlas.mark_clean();

    let result = layout_text(&shaped, "AB", &atlas, options(f32::INFINITY, 20.0));

    assert!(!atlas.is_dirty(), "layout did not dirty the atlas");
    for glyph in &result.lines[0].glyphs {
        assert!(
            glyph.atlas_info.is_none(),
            "layout does not eagerly attach atlas info",
        );
    }
}

#[test]
fn an_explicit_newline_forces_a_line_break() {
    let text = "hello\nworld";
    let (atlas, shaped) = shaped_for(text);
    let result = layout_text(&shaped, text, &atlas, options(f32::INFINITY, 20.0));
    assert_eq!(result.lines.len(), 2, "the newline splits the text");
}
