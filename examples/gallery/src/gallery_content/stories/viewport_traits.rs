//! Viewport-trait stories.
//!
//! Swift source: `GalleryContent/Stories/Foundation/SurfaceTraits.swift`.
//!
//! The Swift story reads `SurfaceReader` plus the `WidthClassReader`,
//! `HeightClassReader` and `OrientationReader` composition readers. All three
//! classifications already exist as pure functions on `cove_core::Viewport`, so
//! this module ports the reporting logic and keeps the view a thin wrapper.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Environment, Insets, Orientation};
use cove_ui_compose::adaptive::SurfaceReader;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::layout::{SafeArea, SafeAreaEdges, SurfaceRoot};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use crate::gallery_content::stories::StorySection;

/// The four viewport lines the story renders, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewportTraits {
    pub width: i32,
    pub height: i32,
    pub width_class: &'static str,
    pub height_class: &'static str,
    pub orientation: &'static str,
    pub platform: &'static str,
    pub design_language: &'static str,
    pub device_class: &'static str,
    pub posture: &'static str,
    pub segments: usize,
    pub safe_area: (i32, i32, i32, i32),
    pub text_scale_percent: i32,
    pub keyboard_visible: bool,
}

impl ViewportTraits {
    /// Reads every trait from one surface environment.
    pub fn new(environment: &Environment) -> Self {
        let viewport = environment.viewport;
        let safe_area = environment.safe_area.normalized();
        Self {
            // `Int(...)` truncates toward zero, as the Swift interpolation does.
            width: viewport.width as i32,
            height: viewport.height as i32,
            width_class: viewport.width_class().name(),
            height_class: viewport.height_class().name(),
            orientation: Self::orientation_name(viewport.orientation()),
            platform: environment.platform.name(),
            design_language: environment.design_language.name(),
            device_class: environment.device_class.name(),
            posture: environment.posture.name(),
            segments: environment.segments().len(),
            safe_area: (
                safe_area.top as i32,
                safe_area.leading as i32,
                safe_area.bottom as i32,
                safe_area.trailing as i32,
            ),
            // `Int((factor * 100).rounded())` — rounded, not truncated.
            text_scale_percent: (environment.text_scale.factor() * 100.0).round() as i32,
            keyboard_visible: environment.keyboard.is_visible(),
        }
    }

    pub fn orientation_name(orientation: Orientation) -> &'static str {
        match orientation {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }

    /// `Viewport: W × H`
    pub fn viewport_line(&self) -> String {
        format!("Viewport: {} × {}", self.width, self.height)
    }

    pub fn width_class_line(&self) -> String {
        format!("Width class: {}", self.width_class)
    }

    pub fn height_class_line(&self) -> String {
        format!("Height class: {}", self.height_class)
    }

    pub fn orientation_line(&self) -> String {
        format!("Orientation: {}", self.orientation)
    }

    pub fn device_line(&self) -> String {
        format!(
            "Device: {} · {} · {}",
            self.platform, self.design_language, self.device_class
        )
    }

    pub fn posture_line(&self) -> String {
        format!("Posture: {} · Segments: {}", self.posture, self.segments)
    }

    pub fn safe_area_line(&self) -> String {
        let (top, leading, bottom, trailing) = self.safe_area;
        format!("Safe area: {top} / {leading} / {bottom} / {trailing}")
    }

    pub fn text_scale_line(&self) -> String {
        format!(
            "Text scale: {}%   ·   Keyboard: {}",
            self.text_scale_percent,
            if self.keyboard_visible {
                "visible"
            } else {
                "hidden"
            }
        )
    }
}

/// The `viewportTraits` story: reports the live surface traits.
pub struct ViewportTraitsStory;

impl ViewportTraitsStory {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ViewportTraitsStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for ViewportTraitsStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SurfaceReader::new(|surface: &Environment| {
            let traits = ViewportTraits::new(surface);
            let line = |text: String, name: &'static str| {
                AnyView::new(
                    Text::new(text)
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .debug_name(name),
                )
            };
            let traits_section = AnyView::new(StorySection::new(
                "Viewport traits",
                "Current host surface.",
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(TupleView::new(vec![
                        line(traits.viewport_line(), ViewportDebugName::SIZE),
                        AnyView::new(
                            Text::new(traits.width_class_line())
                                .debug_name(ViewportDebugName::WIDTH_CLASS),
                        ),
                        AnyView::new(
                            Text::new(traits.height_class_line())
                                .debug_name(ViewportDebugName::HEIGHT_CLASS),
                        ),
                        AnyView::new(
                            Text::new(traits.orientation_line())
                                .debug_name(ViewportDebugName::ORIENTATION),
                        ),
                        AnyView::new(
                            Text::new(traits.device_line()).debug_name(ViewportDebugName::DEVICE),
                        ),
                        AnyView::new(
                            Text::new(traits.posture_line()).debug_name(ViewportDebugName::POSTURE),
                        ),
                        line(traits.safe_area_line(), ViewportDebugName::SAFE_AREA),
                        line(traits.text_scale_line(), ViewportDebugName::TEXT_SCALE),
                        line(THRESHOLD_CAPTION.to_string(), "viewport-thresholds"),
                    ])),
                )),
            ));

            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                24.0,
                AnyView::new(TupleView::new(vec![
                    traits_section,
                    segments_section(),
                    safe_area_section(),
                ])),
            ))
        }))
    }
}

/// The threshold caption the story always shows.
pub const THRESHOLD_CAPTION: &str =
    "Compact < 600 · Medium 600–839 · Expanded 840–1199 · Large 1200–1599 · Extra large ≥ 1600";

/// Debug names used by the gallery and DevTools.
pub struct ViewportDebugName;
impl ViewportDebugName {
    pub const SIZE: &'static str = "viewport-size";
    pub const WIDTH_CLASS: &'static str = "viewport-width-class";
    pub const HEIGHT_CLASS: &'static str = "viewport-height-class";
    pub const ORIENTATION: &'static str = "viewport-orientation";
    pub const DEVICE: &'static str = "viewport-device";
    pub const POSTURE: &'static str = "viewport-posture";
    pub const SAFE_AREA: &'static str = "viewport-safe-area";
    pub const TEXT_SCALE: &'static str = "viewport-text-scale";
}

/// Panels that follow the window segments: one panel until a fold or hinge
/// crosses this section, then one per segment.
fn segments_section() -> AnyView {
    use cove_ui_compose::primitives::layout::{PostureLayout, SegmentedLayout};
    let panel = |text: &'static str, name: &'static str| {
        AnyView::new(
            AnyView::new(Text::new(text))
                .padding(12.0)
                .frame(
                    Some(FrameDimension::Percent(100.0)),
                    Some(FrameDimension::Percent(100.0)),
                    None,
                    None,
                    None,
                    None,
                )
                .semantic_background(SemanticColorRef::surface_variant())
                .debug_name(name),
        )
    };
    AnyView::new(StorySection::new(
        "Window segments",
        "",
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    PostureLayout::new(
                        SegmentedLayout::new(panel("One segment", "segments-single")).split(
                            panel("First segment", "segments-first"),
                            panel("Second segment", "segments-second"),
                        ),
                    )
                    .tabletop(
                        panel("Above the fold", "segments-top"),
                        panel("Below the fold", "segments-bottom"),
                    ),
                ),
            )
            .frame_height(160.0)
            .debug_name("segments-surface"),
        ),
    ))
}

/// A surface that declares insets, so `SafeArea` has something to consume.
///
/// `SurfaceRoot` publishes the host facts for its subtree and never pads by
/// itself; each logical edge is consumed once by the innermost `SafeArea` that
/// asks for it, which is what the nested container below demonstrates.
fn safe_area_section() -> AnyView {
    let mut environment = Environment::default();
    environment.safe_area = Insets {
        top: 24.0,
        bottom: 16.0,
        leading: 12.0,
        trailing: 12.0,
    };
    let inner = AnyView::new(SafeArea::new(
        SafeAreaEdges::ALL,
        AnyView::new(
            AnyView::new(Text::new("Inside the safe area"))
                .padding(8.0)
                .semantic_background(SemanticColorRef::surface_variant()),
        ),
    ));
    AnyView::new(StorySection::new(
        "Safe area",
        "The surface declares 24/16 points and 12/12 insets. SafeArea consumes them along the edges it selects, and a nested container only sees what is left.",
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(SurfaceRoot::new(environment, inner)),
            )
            .frame_size(320.0, 140.0)
            .debug_name("safe-area-surface"),
        ),
    ))
}

#[cfg(test)]
mod checks {
    use super::*;
    use cove_core::{Keyboard, TextScale};

    fn environment(width: f32, height: f32) -> Environment {
        let mut environment = Environment::default();
        environment.viewport.width = width;
        environment.viewport.height = height;
        environment.safe_area.top = 24.0;
        environment.safe_area.leading = 12.0;
        environment
    }

    #[test]
    fn size_classes_use_the_documented_thresholds() {
        // Compact < 600, Medium 600..839, Expanded 840..1199, Large 1200..1599.
        assert_eq!(
            ViewportTraits::new(&environment(599.0, 800.0)).width_class,
            "compact"
        );
        assert_eq!(
            ViewportTraits::new(&environment(600.0, 800.0)).width_class,
            "medium"
        );
        assert_eq!(
            ViewportTraits::new(&environment(839.0, 800.0)).width_class,
            "medium"
        );
        assert_eq!(
            ViewportTraits::new(&environment(840.0, 800.0)).width_class,
            "expanded"
        );
        assert_eq!(
            ViewportTraits::new(&environment(1200.0, 800.0)).width_class,
            "large"
        );
        assert_eq!(
            ViewportTraits::new(&environment(1200.0, 479.0)).height_class,
            "compact"
        );
        assert_eq!(
            ViewportTraits::new(&environment(1200.0, 1000.0)).height_class,
            "expanded"
        );
    }

    #[test]
    fn orientation_follows_the_wider_extent() {
        assert_eq!(
            ViewportTraits::new(&environment(1280.0, 720.0)).orientation,
            "landscape"
        );
        assert_eq!(
            ViewportTraits::new(&environment(390.0, 844.0)).orientation,
            "portrait"
        );
        // A square viewport counts as landscape (width >= height).
        assert_eq!(
            ViewportTraits::new(&environment(600.0, 600.0)).orientation,
            "landscape"
        );
    }

    #[test]
    fn lines_match_the_swift_interpolation() {
        let traits = ViewportTraits::new(&environment(1280.6, 720.9));
        // `Int(...)` truncates, so 1280.6 renders as 1280.
        assert_eq!(traits.viewport_line(), "Viewport: 1280 × 720");
        assert_eq!(traits.width_class_line(), "Width class: large");
        assert_eq!(traits.height_class_line(), "Height class: medium");
        assert_eq!(traits.orientation_line(), "Orientation: landscape");
        assert!(traits.device_line().ends_with(" · desktop"));
        assert_eq!(traits.posture_line(), "Posture: flat · Segments: 1");
        assert_eq!(traits.safe_area_line(), "Safe area: 24 / 12 / 0 / 0");
        assert_eq!(
            traits.text_scale_line(),
            "Text scale: 100%   ·   Keyboard: hidden"
        );
    }

    #[test]
    fn keyboard_visibility_follows_its_height() {
        let mut surface = environment(390.0, 844.0);
        assert!(!ViewportTraits::new(&surface).keyboard_visible);
        surface.keyboard = Keyboard::new(320.0);
        let traits = ViewportTraits::new(&surface);
        assert!(traits.keyboard_visible);
        assert!(traits.text_scale_line().ends_with("Keyboard: visible"));
    }

    #[test]
    fn text_scale_percent_is_rounded_not_truncated() {
        let mut surface = environment(1280.0, 720.0);
        assert_eq!(ViewportTraits::new(&surface).text_scale_percent, 100);
        surface.text_scale = TextScale::linear(1.25);
        assert_eq!(ViewportTraits::new(&surface).text_scale_percent, 125);
        surface.text_scale = TextScale::linear(1.5);
        assert_eq!(ViewportTraits::new(&surface).text_scale_percent, 150);
    }

    #[test]
    fn debug_names_match_the_gallery() {
        assert_eq!(ViewportDebugName::SIZE, "viewport-size");
        assert_eq!(ViewportDebugName::WIDTH_CLASS, "viewport-width-class");
        assert_eq!(ViewportDebugName::TEXT_SCALE, "viewport-text-scale");
        assert_eq!(ViewportDebugName::DEVICE, "viewport-device");
        assert_eq!(ViewportDebugName::POSTURE, "viewport-posture");
        assert_eq!(
            THRESHOLD_CAPTION,
            "Compact < 600 · Medium 600–839 · Expanded 840–1199 · Large 1200–1599 · Extra large ≥ 1600"
        );
    }
}
