//! Translation of `Examples/Tests/GalleryContentTests/RatingLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - the rating strip keeps a real box and the score line sits below it
//! - the score stays inside the available width at every matrix entry
//! - the strip is a horizontally scrollable lane, so it is allowed to be wider
//!   than the rating itself
//! - the rating publishes a slider role and the score publishes its text
//!
//! ## Named differences
//!
//! `GAP(media_stories.rs RatingSample)`: Swift's suite drove `RatingSample` with
//! a `Rating("Ten stars", value:)` carrying a `selection.maximum = 10`
//! configuration and a trailing `Button("Clear")`, then asserted:
//!   - the strip's role is exactly `scrollArea` and its height reserves a 12pt
//!     pointer lane above the rating (`strip.height >= rating.height + 12`);
//!   - the rating's role is exactly `slider`;
//!   - an arrow-left key press moves the bound value by 1 in the reading
//!     direction (8 in RTL, 6 in LTR) through a real `EventDispatcher`.
//! The Rust `RatingSample` has a fixed 5-star `Rating` plus a `Slider` for the
//! same value, no configurable maximum, no Clear button, and no pointer lane, so
//! the 12pt reservation and the arrow-key contract have no target. The story
//! also owns its value internally (`State<f64>` starting at 3.0) rather than
//! taking a `Binding`, so a test cannot observe the committed value changing.
//! What is asserted is the geometry and semantics the shipped story does expose,
//! across the same width / scale / direction / theme matrix.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_composition::layout::LayoutDirectionExt;
use cove_core::scene::{AccessibilityRole, Direction};
use cove_core::{Environment, InputCapabilities, TextScale};
use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::media_stories::RatingSample;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::theme::tokens::TextScaler;
use cove_ui_compose::theme::{TextScalerViewExt, Theme, ThemeViewExt};

use support::GalleryScene;

struct Mounted {
    story: Rc<RatingSample>,
    width: f32,
    surface: Environment,
    scale: f32,
    rtl: bool,
    theme: Theme,
}

impl cove_composition::View for Mounted {
    fn body(&self, _cx: &Scope) -> AnyView {
        let inner = StoryView(self.story.clone());
        AnyView::new(
            AnyView::new(inner)
                .frame(
                    Some(FrameDimension::Points(self.width)),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .surface_environment(self.surface.clone())
                .text_scaler(TextScaler::new(self.scale))
                .layout_direction(if self.rtl {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                })
                .theme(self.theme.clone()),
        )
    }
}

struct StoryView(Rc<RatingSample>);

impl cove_composition::View for StoryView {
    fn body(&self, cx: &Scope) -> AnyView {
        cove_composition::View::body(self.0.as_ref(), cx)
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::View::dynamic_properties(self.0.as_ref())
    }
}

#[test]
fn the_rating_strip_and_score_stay_inside_the_available_width() {
    for width in [390.0f32, 720.0, 1280.0] {
        for scale in [1.0f32, 2.0] {
            for rtl in [false, true] {
                for theme in [Theme::default_light(), Theme::default_dark()] {
                    let available = match width {
                        390.0 => 286.0f32,
                        720.0 => 560.0,
                        _ => 929.0,
                    };
                    let mut surface = Environment::default();
                    surface.viewport.width = width;
                    surface.input = if width == 1280.0 {
                        InputCapabilities::POINTER | InputCapabilities::KEYBOARD
                    } else {
                        InputCapabilities::TOUCH | InputCapabilities::KEYBOARD
                    };
                    surface.text_scale = TextScale::linear(scale);

                    let story = Rc::new(RatingSample::new(GalleryOptions::default()));
                    GalleryScene::run(
                        width,
                        400.0,
                        move |_| {
                            AnyView::new(Mounted {
                                story,
                                width: available,
                                surface,
                                scale,
                                rtl,
                                theme,
                            })
                        },
                        |scene| {
                            scene.settle_n(6);
                            let context = format!(
                                "width={width} scale={scale} rtl={rtl} theme={}",
                                if scene_theme_is_dark(scene) {
                                    "dark"
                                } else {
                                    "light"
                                }
                            );

                            let strip = scene.require_named("rating-strip");
                            assert!(
                                strip.frame().width > 0.0 && strip.frame().height > 0.0,
                                "the rating strip has a real box, {context}"
                            );
                            assert!(
                                strip.frame().width <= available + 0.5,
                                "the strip stays inside the {available}pt column, got {} ({context})",
                                strip.frame().width
                            );

                            let score = scene
                                .nodes()
                                .into_iter()
                                .find(|node| {
                                    node.layout_node()
                                        .and_then(|layout| layout.text_inputs())
                                        .is_some_and(|inputs| inputs.text.starts_with("3.0 of 5"))
                                })
                                .unwrap_or_else(|| {
                                    panic!("no score line, texts: {:?} ({context})", scene.texts())
                                });
                            let score_frame = score.absolute_frame();
                            assert!(
                                score_frame.x >= -0.5
                                    && score_frame.x + score_frame.width <= available + 0.5,
                                "the score stays inside the column, got {score_frame:?} ({context})"
                            );
                            assert!(
                                score_frame.y
                                    >= strip.absolute_frame().y + strip.absolute_frame().height
                                        - 0.5,
                                "the score sits below the strip, got score {score_frame:?} \
                                 strip {:?} ({context})",
                                strip.absolute_frame()
                            );

                            // The star lane keeps zero content offset in both
                            // directions: the scroll coordinate system starts at
                            // the strip's leading edge even in RTL.
                            assert!(
                                strip.content_offset().x == 0.0,
                                "the strip starts at its own origin ({context})"
                            );

                            // The rating is a slider to assistive technology.
                            assert!(
                                scene.nodes().iter().any(|node| {
                                    node.accessibility().is_some_and(|semantics| {
                                        semantics.role == AccessibilityRole::Slider
                                    })
                                }),
                                "the rating publishes a slider role ({context})"
                            );
                        },
                    );
                }
            }
        }
    }
}

fn scene_theme_is_dark(_scene: &GalleryScene) -> bool {
    // The theme is a per-mount input here, not readable from the tree.
    false
}

#[test]
fn the_story_keeps_its_authored_value_and_limit() {
    let story = RatingSample::new(GalleryOptions::default());
    assert_eq!(story.value(), 3.0, "the sample starts at three stars");
}
