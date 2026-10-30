//! Translation of Theme/Styles/Components/SliderStyle.swift.
use std::any::TypeId;
use std::rc::Rc;

use cove_core::scene::CompositionLocal;
use cove_core::{Color, SurfaceShadow};

use crate::theme::Theme;
use crate::{AnyView, ModifiedContent, View, ViewExt};

use super::default_slider_style::DefaultSliderStyle;

/// Snapshot of state passed to `SliderStyle::resolve` on every recompose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderStyleConfiguration {
    pub value: f64,
    pub range: (f64, f64),
    pub is_pressed: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
    pub is_enabled: bool,
}

impl SliderStyleConfiguration {
    pub fn fraction(&self) -> f64 {
        let (low, high) = self.range;
        let span = high - low;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - low) / span).clamp(0.0, 1.0)
    }

    pub fn interaction_key(&self) -> SliderInteractionKey {
        SliderInteractionKey {
            is_pressed: self.is_pressed,
            is_hovered: self.is_hovered,
            is_focused: self.is_focused,
            is_enabled: self.is_enabled,
        }
    }
}

/// Interaction flags used by built-in styles to key implicit transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SliderInteractionKey {
    pub is_pressed: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
    pub is_enabled: bool,
}

/// Pure-data description of how a slider should be drawn for one frame.
///
/// A style returns this instead of a view because the filled-track and thumb
/// positions depend on the host's measured width, which is only known at draw
/// time. The host primitive consumes the appearance and emits the quads.
#[derive(Clone, Debug, PartialEq)]
pub struct SliderAppearance {
    pub track_height: f32,
    pub track_color: Color,
    pub fill_color: Color,
    /// Cross-axis knob size (16 in the reference).
    pub thumb_diameter: f32,
    /// Along-axis knob size; a capsule knob is longer than it is tall (20x16).
    pub thumb_length: f32,
    pub thumb_color: Color,
    pub thumb_border_color: Option<Color>,
    pub thumb_border_width: f32,
    /// Drop shadow under the knob; `None` for a flat knob.
    pub thumb_shadow: Option<SurfaceShadow>,
    pub track_corner_radius: f32,
}

#[allow(clippy::too_many_arguments)]
impl SliderAppearance {
    pub fn new(
        track_height: f32,
        track_color: Color,
        fill_color: Color,
        thumb_diameter: f32,
        thumb_length: Option<f32>,
        thumb_color: Color,
        thumb_border_color: Option<Color>,
        thumb_border_width: f32,
        thumb_shadow: Option<SurfaceShadow>,
        track_corner_radius: f32,
    ) -> Self {
        Self {
            track_height,
            track_color,
            fill_color,
            thumb_diameter,
            thumb_length: thumb_length.unwrap_or(thumb_diameter),
            thumb_color,
            thumb_border_color,
            thumb_border_width,
            thumb_shadow,
            track_corner_radius,
        }
    }
}

pub trait SliderStyle {
    fn resolve(&self, configuration: &SliderStyleConfiguration, theme: &Theme) -> SliderAppearance;
}

/// Type-erased `SliderStyle` ferried through the composition tree.
pub struct AnySliderStyle {
    resolve: Rc<dyn Fn(&SliderStyleConfiguration, &Theme) -> SliderAppearance>,
    style_id: TypeId,
}
impl AnySliderStyle {
    pub fn new<S: SliderStyle + 'static>(style: S) -> Self {
        Self {
            resolve: Rc::new(move |configuration, theme| style.resolve(configuration, theme)),
            style_id: TypeId::of::<S>(),
        }
    }
    pub fn resolve(&self, configuration: &SliderStyleConfiguration, theme: &Theme) -> SliderAppearance {
        (self.resolve)(configuration, theme)
    }
}
impl Clone for AnySliderStyle {
    fn clone(&self) -> Self {
        Self { resolve: self.resolve.clone(), style_id: self.style_id }
    }
}
impl PartialEq for AnySliderStyle {
    fn eq(&self, other: &Self) -> bool {
        self.style_id == other.style_id
    }
}
impl Eq for AnySliderStyle {}

pub struct SliderStyleEnvironment;
impl SliderStyleEnvironment {
    pub fn key() -> Rc<CompositionLocal<AnySliderStyle>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<AnySliderStyle>> =
                Rc::new(CompositionLocal::new(|| AnySliderStyle::new(DefaultSliderStyle)));
        }
        KEY.with(|key| key.clone())
    }
}

pub trait SliderStyleViewExt: View + Sized + 'static {
    /// Override the `SliderStyle` used by every slider in this subtree.
    fn slider_style<S: SliderStyle + 'static>(self, style: S) -> ModifiedContent {
        self.composition_local(SliderStyleEnvironment::key(), AnySliderStyle::new(style))
    }
}
impl<T: View + Sized + 'static> SliderStyleViewExt for T {}

/// Default transition duration for built-in slider interaction changes.
pub const SLIDER_INTERACTION_DURATION: f64 = 0.12;

const _: fn() -> Option<AnyView> = || None;
