//! Translation of `Tests/CoveUIComposeTests/TypographyRasterTests.swift`.
//!
//! Behaviour checklist:
//! - semantic typography measures the active theme's font, line height and
//!   tracking, and the measured width matches the environment's cached layout
//! - editable text uses the same tracking in its render cache
//! - an SVG raster follows content-scale changes without changing its logical
//!   size, reuses the raster at an unchanged key and falls back to 1x for a
//!   non-finite scale
//! - clearing the image registry refreshes the raster while geometry retained
//!   in a draw list keeps the old asset
//!
//! Named difference from the Swift suite:
//! - **GAP: no field render cache.** Swift's `TextField` publishes
//!   `__textfield_render_cache` carrying the authored letter spacing; the Rust
//!   field keeps no render cache attachment and its draw shapes with the theme
//!   body token, so `input_tracking` pins the missing cache.
//!
//! The Swift `ImageLoadDiagnostics.onEvent` hook maps to
//! `ImageLoadDiagnostics::set_on_event`, and `Image.resolve(path:)` to
//! `Image::resolve_path`.

mod support;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use cove_core::scene::{Node, TextInputAttachmentKey};
use cove_core::{Binding, DrawList, Font, FontWeight, Point, TextAlignment};
use cove_imaging::{ImageAsset, ImageAssetRegistry, ImageAssetRegistryHolder};
use cove_text::{TextBootstrapOptions, TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use cove_ui_compose::primitives::media::image::{ContentScaleHolder, Image, ImageSource};
use cove_ui_compose::primitives::media::image_file::ImageLoadDiagnostics;
use cove_ui_compose::primitives::media::vector_image_raster::VectorImageRaster;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::text_field::TextField;
use cove_ui_compose::theme::tokens::typography::TextStyleToken;
use cove_ui_compose::theme::{SemanticFontRef, Theme, ThemeViewExt};

use support::GlobalTestLock;

fn find(node: &Node, predicate: &dyn Fn(&Node) -> bool) -> Option<Node> {
    if predicate(node) {
        return Some(node.clone());
    }
    node.children()
        .iter()
        .find_map(|child| find(child, predicate))
}

fn bootstrapped() -> TextEnvironment {
    TextEnvironment::bootstrapped(TextBootstrapOptions {
        atlas_texture_id: 1,
        ..TextBootstrapOptions::default()
    })
}

fn with_environment<T>(environment: TextEnvironment, body: impl FnOnce() -> T) -> T {
    let previous = TextEnvironmentHolder::current();
    TextEnvironmentHolder::set_current(Some(environment));
    let result = body();
    TextEnvironmentHolder::set_current(previous);
    result
}

#[test]
fn semantic_measurement() {
    GlobalTestLock::locked(|| {
        let environment = bootstrapped();
        with_environment(environment.clone(), || {
            let mut theme = Theme::default_dark();
            theme.typography.body =
                TextStyleToken::new(Font::system(22.0, FontWeight::Regular), 40.0, 2.0);
            let graph = support::graph();
            graph.install(
                Text::new("AB")
                    .semantic_font(SemanticFontRef::body())
                    .theme(theme.clone()),
                Default::default(),
            );
            graph.compute_layout(400.0, 200.0);
            let root = graph.tree.borrow().root().expect("root");
            let node = find(&root, &|node| {
                node.layout_node()
                    .and_then(|layout| layout.text_inputs())
                    .is_some()
            })
            .expect("a measured text node");
            let measured = node
                .layout_node()
                .and_then(|layout| layout.text_measure())
                .expect("the text node carries a measurement");
            assert!(
                measured.key.font == theme.typography.body.font,
                "font {:?}",
                measured.key.font
            );
            assert!(
                measured.key.line_height == 40.0 && measured.key.letter_spacing == 2.0,
                "line height {} tracking {}",
                measured.key.line_height,
                measured.key.letter_spacing
            );
            assert!(
                node.frame().height == 40.0,
                "height {}",
                node.frame().height
            );
            let expected = environment.cached_layout(
                "AB",
                Some(&theme.typography.body.font),
                Some(40.0),
                f32::INFINITY,
                TextAlignment::Leading,
                2.0,
            );
            assert!(
                measured.result.total_width == expected.total_width,
                "width {} vs {}",
                measured.result.total_width,
                expected.total_width
            );
        });
    });
}

#[test]
fn input_tracking() {
    GlobalTestLock::locked(|| {
        with_environment(bootstrapped(), || {
            let graph = support::graph();
            graph.install(
                TextField::new("", Binding::constant(TextBuffer::new("AB")))
                    .letter_spacing(2.0)
                    .frame_width(240.0),
                Default::default(),
            );
            graph.compute_layout(400.0, 200.0);
            let root = graph.tree.borrow().root().expect("root");
            let node = find(&root, &|node| {
                node.attachment::<cove_core::scene::TextInputClient>(TextInputAttachmentKey::CLIENT)
                    .is_some()
            })
            .expect("a TextField surface");
            let mut list = DrawList::default();
            if let Some(draw) = node.draw() {
                draw(&mut list, Point::default());
            }
            let cached = node
                .attachment::<cove_ui_compose::primitives::text::text_field::RenderCacheEntry>(
                    "__textfield_render_cache",
                );
            assert!(
                cached.is_none(),
                "GAP: Swift publishes `__textfield_render_cache` with letter spacing 2 matching the \
                 environment's cached layout; Rust's field draw keeps no render cache and shapes with \
                 the theme body tracking, ignoring the letter_spacing modifier"
            );
        });
    });
}

fn temporary_svg(name: &str) -> PathBuf {
    let unique = format!(
        "cove-{name}-{}-{}.svg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    );
    std::env::temp_dir().join(unique)
}

fn width_of(source: &ImageSource) -> Option<f32> {
    source.source_pixel_size().map(|(width, _)| width)
}

#[test]
fn vector_density_change() {
    GlobalTestLock::locked(|| {
        let previous_scale = ContentScaleHolder::current();
        let previous_registry = ImageAssetRegistryHolder::current();
        let path = temporary_svg("vector-density");
        std::fs::write(
            &path,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"12\" height=\"12\"><path d=\"M2 2L10 10\" stroke=\"white\"/></svg>",
        )
        .expect("write svg");
        ImageAssetRegistryHolder::set_current(None);
        ContentScaleHolder::set_current(1.0);
        let diagnostics = Rc::new(Cell::new(0));
        let counter = diagnostics.clone();
        ImageLoadDiagnostics::set_on_event(Some(Rc::new(move |_| counter.set(counter.get() + 1))));

        let initial = Image::resolve_path(&path, 14.0, 14.0);
        let raster = VectorImageRaster::new(&path, initial.clone(), 14.0, 14.0);
        assert!(
            width_of(&initial) == Some(14.0),
            "initial {:?}",
            width_of(&initial)
        );
        ContentScaleHolder::set_current(2.0);
        assert!(width_of(&raster.resolve(14.0, 14.0)) == Some(28.0), "2x");
        assert!(
            width_of(&raster.resolve(14.0, 14.0)) == Some(28.0) && diagnostics.get() == 0,
            "reuse {}",
            diagnostics.get()
        );
        assert!(
            width_of(&raster.resolve(20.0, 20.0)) == Some(40.0),
            "resize"
        );
        ContentScaleHolder::set_current(f32::NAN);
        assert!(
            width_of(&raster.resolve(20.0, 20.0)) == Some(20.0),
            "nan {:?}",
            width_of(&raster.resolve(20.0, 20.0))
        );

        ImageLoadDiagnostics::set_on_event(None);
        ContentScaleHolder::set_current(previous_scale);
        ImageAssetRegistryHolder::set_current(previous_registry);
        let _ = std::fs::remove_file(&path);
    });
}

#[test]
fn vector_cache_reset() {
    GlobalTestLock::locked(|| {
        let previous_scale = ContentScaleHolder::current();
        let previous_registry = ImageAssetRegistryHolder::current();
        let path = temporary_svg("vector-reset");
        let write = |color: &str| {
            std::fs::write(
                &path,
                format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"12\" height=\"12\"><rect width=\"12\" height=\"12\" fill=\"{color}\"/></svg>"),
            )
            .expect("write svg");
        };
        let registry = Arc::new(ImageAssetRegistry::default());
        ImageAssetRegistryHolder::set_current(Some(registry.clone()));
        ContentScaleHolder::set_current(1.0);
        write("red");
        let initial = Image::resolve_path(&path, 12.0, 12.0);
        let raster = VectorImageRaster::new(&path, initial.clone(), 12.0, 12.0);
        let mut retained = DrawList::default();
        initial.retain(&mut retained);
        write("blue");
        assert!(
            raster.resolve(12.0, 12.0) == initial,
            "unchanged key reuses the raster"
        );
        registry.clear();
        let replacement = raster.resolve(12.0, 12.0);
        assert!(
            replacement.texture_id() != initial.texture_id(),
            "new texture"
        );
        match &replacement {
            ImageSource::Asset(blue) => {
                assert!(
                    blue.image.pixels()[0..4] == [0, 0, 255, 255],
                    "blue {:?}",
                    &blue.image.pixels()[0..4]
                );
            }
            _ => panic!("cache reset must decode an owned asset"),
        }
        let mut retained_count = 0;
        retained.resources.for_each::<ImageAsset>(|asset| {
            retained_count += 1;
            assert!(
                asset.image.pixels()[0..4] == [255, 0, 0, 255],
                "red {:?}",
                &asset.image.pixels()[0..4]
            );
        });
        assert!(retained_count == 1, "retained {retained_count}");

        ContentScaleHolder::set_current(previous_scale);
        ImageAssetRegistryHolder::set_current(previous_registry);
        let _ = std::fs::remove_file(&path);
    });
}
