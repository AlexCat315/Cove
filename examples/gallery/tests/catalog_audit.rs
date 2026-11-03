//! Structural coverage is separate from manual visual/interaction acceptance.
mod support;

use cove_composition::AnyView;
use cove_core::{Environment, InputCapabilities};
use cove_examples::gallery_content::{GalleryView, catalog::GalleryCatalog};
use cove_ui_compose::{
    adaptive::SurfaceEnvironmentExt, theme::styles::appearance_modifier::Appearance,
};
use serde_json::json;
use support::GalleryScene;

#[test]
fn every_catalog_page_mounts_with_valid_geometry_on_pointer_and_touch_surfaces() {
    let mut report = Vec::new();
    let mut invalid = Vec::new();
    for (width, touch, scale, appearance) in [
        (1280.0, false, 1.0, Appearance::Light),
        (390.0, true, 2.0, Appearance::Dark),
    ] {
        for entry in GalleryCatalog::entries() {
            let mut environment = Environment::default();
            environment.viewport.width = width;
            environment.viewport.height = 820.0;
            environment.input = if touch {
                InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
            } else {
                InputCapabilities::POINTER | InputCapabilities::KEYBOARD
            };
            GalleryScene::run(
                width,
                820.0,
                |_| {
                    AnyView::new(
                        GalleryView::new(entry.page, appearance, scale)
                            .surface_environment(environment),
                    )
                },
                |scene| {
                    let diagnostics =
                        cove_devtools::inspection::Diagnostics::new(scene.graph.tree.clone(), None);
                    let overview = diagnostics.overview(100);
                    if !overview.layout_issues.invalid_geometry.is_empty() {
                        invalid.push(format!(
                            "{} {width}: {:?}",
                            entry.title, overview.layout_issues.invalid_geometry
                        ));
                    }
                    let section_count = scene
                        .nodes()
                        .iter()
                        .filter(|node| {
                            support::debug_name(node)
                                .is_some_and(|name| name.starts_with("story-section-"))
                        })
                        .count();
                    report.push(json!({"page": entry.page.raw_value(), "title": entry.title, "width": width, "touch": touch, "text_scale": scale,
                    "nodes": overview.node_count, "sections": section_count, "layout_issues": overview.layout_issues}));
                    assert!(
                        overview.has_root && overview.node_count > 10,
                        "{} has a real scene",
                        entry.title
                    );
                },
            );
        }
    }
    if let Some(path) = std::env::var_os("COVE_GALLERY_AUDIT_OUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    assert!(invalid.is_empty(), "invalid geometry: {invalid:?}");
}
