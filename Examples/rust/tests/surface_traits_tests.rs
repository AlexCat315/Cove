//! Translation of `Examples/Tests/GalleryContentTests/SurfaceTraitsTests.swift`.
//!
//! Behaviour checklist:
//! - the story reports the host's idiom, safe area, text scale and keyboard
//!   visibility from the live surface environment
//! - it reports idle without installing a simulated device preview
//!
//! The Swift suite's last assertion was that no `phone-preview*` node exists —
//! i.e. the gallery reports facts rather than pretending to be a phone. That
//! holds here and is asserted positively.
//!
//! `GAP(SurfaceRoot)`: Swift mounted the story inside a `SurfaceRoot(surface)`
//! helper that injected a synthetic `UIEnvironment` (idiom `.phone`, 393x852,
//! safe area 59/34, keyboard 336, text scale 1.3). Rust has no `SurfaceRoot`;
//! the equivalent is `SurfaceEnvironmentExt::surface_environment`, which is what
//! is used here, so the assertions are unchanged.

mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::{Environment, Idiom, Insets, Keyboard, TextScale};
use cove_core::InputCapabilities;
use cove_examples::gallery_content::stories::viewport_traits::ViewportTraitsStory;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;

use support::GalleryScene;

fn phone_environment() -> Environment {
    let mut surface = Environment::default();
    surface.idiom = Idiom::Phone;
    surface.viewport.width = 393.0;
    surface.viewport.height = 852.0;
    surface.safe_area = Insets {
        top: 59.0,
        leading: 0.0,
        bottom: 34.0,
        trailing: 0.0,
    };
    surface.keyboard = Keyboard { height: 336.0 };
    surface.text_scale = TextScale::linear(1.3);
    surface.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
    surface
}

#[test]
fn the_gallery_reports_host_facts_without_installing_a_simulated_device() {
    let surface = phone_environment();
    let installed = Rc::new(surface.clone());
    GalleryScene::run(
        393.0,
        852.0,
        move |_| {
            AnyView::new(
                AnyView::new(ViewportTraitsStory::new())
                    .surface_environment(installed.as_ref().clone()),
            )
        },
        |scene| {
            scene.settle_n(8);
            let texts = scene.texts();

            assert!(
                texts.iter().any(|text| text == "Idiom: phone"),
                "the story reports the live idiom, texts: {texts:?}"
            );
            assert!(
                texts
                    .iter()
                    .any(|text| text == "Safe area: 59 / 0 / 34 / 0"),
                "the story reports the live safe area, texts: {texts:?}"
            );
            assert!(
                texts
                    .iter()
                    .any(|text| text == "Text scale: 130%   ·   Keyboard: visible"),
                "the story reports the live text scale and keyboard, texts: {texts:?}"
            );

            // No simulated device preview is installed: the story reports the
            // host's facts instead of drawing a fake phone.
            assert!(
                !scene.nodes().iter().any(|node| {
                    support::debug_name(node)
                        .is_some_and(|name| name.starts_with("phone-preview"))
                }),
                "the gallery must not install a simulated device preview"
            );

            // Every named trait line is present and non-empty.
            for name in [
                "viewport-size",
                "viewport-width-class",
                "viewport-height-class",
                "viewport-orientation",
                "viewport-idiom",
                "viewport-safe-area",
                "viewport-text-scale",
                "viewport-thresholds",
            ] {
                let node = scene
                    .named(name)
                    .unwrap_or_else(|| panic!("{name} is missing"));
                let layout = node.layout_node().expect("the trait line has text");
                let inputs = layout.text_inputs().expect("text inputs");
                assert!(!inputs.text.is_empty(), "{name} renders an empty line");
            }
        },
    );
}

#[test]
fn the_trait_lines_follow_the_environment_they_are_given() {
    // A desktop surface must report differently from the phone surface above.
    let mut desktop = Environment::default();
    desktop.viewport.width = 1280.0;
    desktop.viewport.height = 820.0;
    desktop.idiom = Idiom::Desktop;
    desktop.safe_area = Insets::default();
    let installed = Rc::new(desktop);
    GalleryScene::run(
        1280.0,
        820.0,
        move |_| {
            AnyView::new(
                AnyView::new(ViewportTraitsStory::new())
                    .surface_environment(installed.as_ref().clone()),
            )
        },
        |scene| {
            scene.settle_n(8);
            let texts = scene.texts();
            assert!(
                texts.iter().any(|text| text == "Idiom: desktop"),
                "the idiom follows the supplied surface, texts: {texts:?}"
            );
            assert!(
                texts
                    .iter()
                    .any(|text| text == "Safe area: 0 / 0 / 0 / 0"),
                "a desktop surface has no safe area, texts: {texts:?}"
            );
            assert!(
                texts
                    .iter()
                    .any(|text| text == "Text scale: 100%   ·   Keyboard: hidden"),
                "a desktop surface has no keyboard and no scaling, texts: {texts:?}"
            );
        },
    );
}
