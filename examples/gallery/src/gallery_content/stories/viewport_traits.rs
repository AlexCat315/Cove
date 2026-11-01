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
use cove_core::{Environment, Idiom, Orientation, SizeClass};
use cove_ui_compose::adaptive::SurfaceReader;
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
    pub idiom: &'static str,
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
            width_class: Self::size_class_name(viewport.width_class()),
            height_class: Self::size_class_name(viewport.height_class()),
            orientation: Self::orientation_name(viewport.orientation()),
            idiom: Self::idiom_name(environment.idiom),
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

    pub fn size_class_name(class: SizeClass) -> &'static str {
        match class {
            SizeClass::Compact => "compact",
            SizeClass::Medium => "medium",
            SizeClass::Expanded => "expanded",
        }
    }

    pub fn orientation_name(orientation: Orientation) -> &'static str {
        match orientation {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }

    pub fn idiom_name(idiom: Idiom) -> &'static str {
        match idiom {
            Idiom::Phone => "phone",
            Idiom::Tablet => "tablet",
            Idiom::Desktop => "desktop",
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

    pub fn idiom_line(&self) -> String {
        format!("Idiom: {}", self.idiom)
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
            AnyView::new(StorySection::new(
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
                            Text::new(traits.idiom_line()).debug_name(ViewportDebugName::IDIOM),
                        ),
                        line(traits.safe_area_line(), ViewportDebugName::SAFE_AREA),
                        line(traits.text_scale_line(), ViewportDebugName::TEXT_SCALE),
                        line(THRESHOLD_CAPTION.to_string(), "viewport-thresholds"),
                    ])),
                )),
            ))
        }))
    }
}

/// The threshold caption the story always shows.
pub const THRESHOLD_CAPTION: &str = "Compact < 600 · Medium 600–999 · Expanded ≥ 1000";

/// Debug names used by the gallery and DevTools.
pub struct ViewportDebugName;
impl ViewportDebugName {
    pub const SIZE: &'static str = "viewport-size";
    pub const WIDTH_CLASS: &'static str = "viewport-width-class";
    pub const HEIGHT_CLASS: &'static str = "viewport-height-class";
    pub const ORIENTATION: &'static str = "viewport-orientation";
    pub const IDIOM: &'static str = "viewport-idiom";
    pub const SAFE_AREA: &'static str = "viewport-safe-area";
    pub const TEXT_SCALE: &'static str = "viewport-text-scale";
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
        // Compact < 600, Medium 600..999, Expanded >= 1000.
        assert_eq!(
            ViewportTraits::new(&environment(599.0, 800.0)).width_class,
            "compact"
        );
        assert_eq!(
            ViewportTraits::new(&environment(600.0, 800.0)).width_class,
            "medium"
        );
        assert_eq!(
            ViewportTraits::new(&environment(999.0, 800.0)).width_class,
            "medium"
        );
        assert_eq!(
            ViewportTraits::new(&environment(1000.0, 800.0)).width_class,
            "expanded"
        );
        assert_eq!(
            ViewportTraits::new(&environment(1200.0, 599.0)).height_class,
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
        assert_eq!(traits.width_class_line(), "Width class: expanded");
        assert_eq!(traits.height_class_line(), "Height class: medium");
        assert_eq!(traits.orientation_line(), "Orientation: landscape");
        assert_eq!(traits.idiom_line(), "Idiom: desktop");
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
        assert_eq!(
            THRESHOLD_CAPTION,
            "Compact < 600 · Medium 600–999 · Expanded ≥ 1000"
        );
    }
}
