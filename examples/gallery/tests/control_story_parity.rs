//! Swift page parity: real controls, responsive geometry and state-driven results.
mod support;
use cove_composition::AnyView;
use cove_core::scene::AccessibilityRole;
use cove_examples::gallery_content::{
    catalog::{GalleryOptions, GalleryPage},
    routing::StoryDispatch,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use support::GalleryScene;

fn control(scene: &GalleryScene, label: &str) -> cove_core::scene::Node {
    scene
        .first(|n| {
            n.accessibility().is_some_and(|s| {
                s.label == label
                    && matches!(
                        s.role,
                        AccessibilityRole::Button
                            | AccessibilityRole::Checkbox
                            | AccessibilityRole::TextField
                            | AccessibilityRole::ComboBox
                    )
            })
        })
        .unwrap_or_else(|| panic!("missing control {label}: {:?}", support::labels(scene)))
}

#[test]
fn all_nine_pages_mount_controls_within_narrow_and_wide_content() {
    for page in [
        GalleryPage::SplitButton,
        GalleryPage::Toolbar,
        GalleryPage::CompletionPopover,
        GalleryPage::TextArea,
        GalleryPage::TextEditor,
        GalleryPage::NumberField,
        GalleryPage::Vec3Field,
        GalleryPage::ColorField,
        GalleryPage::Select,
    ] {
        for width in [390.0, 720.0, 1280.0] {
            GalleryScene::run(
                width,
                2200.0,
                move |_| {
                    AnyView::new(LayerRoot::new(
                        StoryDispatch::new(page)
                            .view(GalleryOptions::default())
                            .unwrap(),
                    ))
                },
                |scene| {
                    let controls = scene.all(|n| {
                        n.accessibility().is_some_and(|s| {
                            matches!(
                                s.role,
                                AccessibilityRole::Button
                                    | AccessibilityRole::TextField
                                    | AccessibilityRole::ComboBox
                            )
                        })
                    });
                    assert!(!controls.is_empty(), "{page:?} has real controls");
                    for node in controls {
                        let frame = node.absolute_frame();
                        assert!(
                            frame.width.is_finite() && frame.height.is_finite(),
                            "{page:?}: {frame:?}"
                        );
                        if frame.width > 0.0 && frame.height > 0.0 {
                            assert!(
                                frame.x >= -0.5 && frame.x + frame.width <= width + 0.5,
                                "{page:?} at {width}: {:?} {frame:?}",
                                node.accessibility().unwrap().label
                            );
                        }
                    }
                },
            );
        }
    }
}

#[test]
fn toolbar_restores_groups_search_zoom_and_vertical_overflow() {
    GalleryScene::run(
        1280.0,
        1600.0,
        |_| {
            AnyView::new(LayerRoot::new(
                StoryDispatch::new(GalleryPage::Toolbar)
                    .view(Default::default())
                    .unwrap(),
            ))
        },
        |scene| {
            for label in [
                "Adaptive project commands",
                "Playback and formatting",
                "Playback",
                "Formatting",
                "Zoom",
                "Tools",
                "Vertical commands",
            ] {
                assert!(
                    support::labels(scene).contains(&label.into()),
                    "missing {label}"
                );
            }
            scene.click(&control(scene, "Run"));
            scene.settle();
            assert!(scene.has_text("Playback running"));
            scene.click(&control(scene, "Zoom in"));
            scene.settle();
            assert!(scene.has_text("Zoom: 110% · Applied filter: None"));
            scene.click(&control(scene, "Find in project"));
            scene.text_input("needle");
            scene.settle();
            scene.click(&control(scene, "Apply"));
            scene.settle();
            assert!(scene.has_text("Zoom: 110% · Applied filter: needle"));
        },
    );
}

#[test]
fn split_button_primary_and_menu_publish_the_chosen_action() {
    GalleryScene::run(
        700.0,
        600.0,
        |_| {
            AnyView::new(LayerRoot::new(
                StoryDispatch::new(GalleryPage::SplitButton)
                    .view(Default::default())
                    .unwrap(),
            ))
        },
        |scene| {
            scene.click(&control(scene, "Save"));
            scene.settle();
            assert!(scene.has_text("Last action: Save"));
            scene.click(&control(scene, "More actions"));
            scene.settle();
            let item = scene
                .first(|n| n.accessibility().is_some_and(|s| s.label == "Export"))
                .expect("Export menu item");
            scene.click(&item);
            scene.settle();
            assert!(scene.has_text("Last action: Export"));
        },
    );
}

#[test]
fn live_completion_inserts_and_reports_acceptance() {
    GalleryScene::run(
        700.0,
        1200.0,
        |_| {
            AnyView::new(LayerRoot::new(
                StoryDispatch::new(GalleryPage::CompletionPopover)
                    .view(Default::default())
                    .unwrap(),
            ))
        },
        |scene| {
            let field = control(scene, "Type a symbol");
            scene.click(&field);
            scene.text_input("ren");
            scene.key_with(
                cove_ui_compose::Scancode::SPACE,
                cove_platform::KeyModifiers::LEFT_CTRL,
            );
            scene.settle();
            scene.key(cove_ui_compose::Scancode::RETURN);
            scene.settle();
            assert!(scene.has_text("Inserted: render"));
        },
    );
}
