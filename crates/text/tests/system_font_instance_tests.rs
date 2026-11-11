//! Translation of `Tests/CoveUIRuntimeTests/SystemFontInstanceTests.swift`.
//!
//! A managed system font must keep working after the temporary objects that
//! created it drain, its requested weight must reach the shared FreeType face as
//! a static face or as design coordinates, and changing the display density must change raster
//! resolution without changing the selected optical size or the advance widths.
//!
//! Behaviour checklist:
//! - the primary face still shapes and rasterizes after the provider is reused
//! - every shaped glyph rasterizes into a non-empty atlas
//! - regular and bold resolve to distinct system instances and rasterize differently
//! - changing the raster scale keeps the design coordinates identical
//! - changing the raster scale keeps the advance widths within a third of a point

use std::rc::Rc;

use cove_core::FontWeight;
use cove_text::{FontAtlas, FontProvider, FontRun, SystemFontDefaults};

const ATLAS_EDGE: usize = 1024;

fn provider(size: f32, raster_scale: f32) -> FontProvider {
    FontProvider::new(size, raster_scale)
}

fn load(provider: &mut FontProvider, weight: FontWeight) -> Rc<cove_text::ManagedFont> {
    provider
        .load_primary_font(&SystemFontDefaults::primary_font_name(), weight, false)
        .expect("the system primary font loads")
}

fn coordinates(font: &cove_text::ManagedFont) -> Vec<(u32, f64)> {
    font.face
        .applied_design_coordinates()
        .iter()
        .map(|(axis, value)| (axis.0, *value))
        .collect()
}

#[test]
fn backing_lifetime() {
    let mut font_provider = provider(14.0, 2.0);
    {
        load(&mut font_provider, FontWeight::Regular);
        assert!(
            !font_provider.resolve_runs("Muing 中文").is_empty(),
            "the primary face resolves runs after being loaded in a nested scope",
        );
    }

    let atlas = FontAtlas::new(ATLAS_EDGE, ATLAS_EDGE);
    let runs = font_provider.resolve_runs("Muing 中文");
    font_provider.register_all_fonts(&atlas);
    let glyphs: Vec<_> = runs
        .iter()
        .flat_map(|run: &FontRun| font_provider.shape_run(run))
        .collect();
    assert!(!glyphs.is_empty(), "the text shapes to at least one glyph");
    for glyph in glyphs {
        assert!(
            atlas
                .rasterize_glyph(glyph.glyph_id, glyph.font_id)
                .is_some(),
            "glyph {} of font {} rasterizes after the provider is reused",
            glyph.glyph_id,
            glyph.font_id,
        );
    }
    assert!(
        atlas.alpha_pixels().iter().any(|&pixel| pixel != 0),
        "the atlas holds rasterized coverage",
    );
}

#[test]
fn weights() {
    let mut coverage = Vec::new();
    let mut instances = Vec::new();
    for weight in [FontWeight::Regular, FontWeight::Bold] {
        let mut font_provider = provider(16.0, 2.0);
        let font = load(&mut font_provider, weight);
        instances.push((font.post_script_name.clone(), coordinates(&font)));

        let atlas = FontAtlas::new(ATLAS_EDGE, ATLAS_EDGE);
        font_provider.register_all_fonts(&atlas);
        let run = font_provider
            .resolve_runs("Hamburgefonts 0123456789")
            .into_iter()
            .next()
            .expect("the sample text resolves one run");
        let glyphs = font_provider.shape_run(&run);
        for glyph in &glyphs {
            atlas
                .rasterize_glyph(glyph.glyph_id, glyph.font_id)
                .expect("the requested system weight rasterizes");
        }
        coverage.push(atlas.alpha_pixels().iter().map(|&p| p as u64).sum::<u64>());
    }

    assert_ne!(
        instances[0], instances[1],
        "bold resolves to a distinct instance"
    );
    assert!(
        coverage[0] > 0 && coverage[1] > coverage[0],
        "bold produces more ink than regular: {coverage:?}",
    );
}

#[test]
fn optical_size() {
    for size in [13.0_f32, 24.0, 34.0] {
        let mut widths = Vec::new();
        let mut instances = Vec::new();
        for raster_scale in [1.0_f32, 2.0] {
            let mut font_provider = provider(size, raster_scale);
            let font = load(&mut font_provider, FontWeight::Semibold);
            instances.push(coordinates(&font));
            let mut run_width = 0.0;
            for run in font_provider.resolve_runs("Dashboard · English 54.8 MB/s") {
                run_width += font_provider
                    .shape_run(&run)
                    .iter()
                    .map(|glyph| glyph.x_advance)
                    .sum::<f32>();
            }
            widths.push(run_width);
        }
        assert_eq!(
            instances[0], instances[1],
            "size {size}: raster scale does not select a different optical size",
        );
        assert!(
            (widths[0] - widths[1]).abs() < 0.3,
            "size {size}: advances are density-independent, got {widths:?}",
        );
    }
}
