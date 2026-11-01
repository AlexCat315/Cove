//! Translation of Primitives/Controls/Fields/Slider/Slider.swift.
//!
//! Continuous, single-thumb slider bound to a value within a range. Mirrors the
//! `Button` pattern: the public struct delegates to a stateful composite while
//! hit testing and drawing live in a leaf primitive host.
use crate::Scope;
use std::rc::Rc;

use crate::theme::ControlSizeEnvironment;
use crate::{AnyView, View};
use cove_core::Binding;

use super::host::{SliderAxis, SliderHost};

/// Continuous, single-thumb slider.
pub struct Slider {
    pub value: Binding<f64>,
    pub range: (f64, f64),
    pub step: Option<f64>,
    pub is_enabled: bool,
    pub on_editing_changed: Option<Rc<dyn Fn(bool)>>,
    /// Draws a small tick at every discrete `step` position.
    pub shows_ticks: bool,
    pub axis: SliderAxis,
}

impl Slider {
    pub fn new(value: Binding<f64>) -> Self {
        Self {
            value,
            range: (0.0, 1.0),
            step: None,
            is_enabled: true,
            on_editing_changed: None,
            shows_ticks: false,
            axis: SliderAxis::Horizontal,
        }
    }

    pub fn range(mut self, range: (f64, f64)) -> Self {
        self.range = range;
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            Some(step)
        } else {
            None
        };
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn on_editing_changed(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_editing_changed = Some(Rc::new(handler));
        self
    }

    pub fn shows_ticks(mut self, shows_ticks: bool) -> Self {
        self.shows_ticks = shows_ticks;
        self
    }

    pub fn axis(mut self, axis: SliderAxis) -> Self {
        self.axis = axis;
        self
    }

    fn snapshot(&self) -> Slider {
        Slider {
            value: self.value.clone(),
            range: self.range,
            step: self.step,
            is_enabled: self.is_enabled,
            on_editing_changed: self.on_editing_changed.clone(),
            shows_ticks: self.shows_ticks,
            axis: self.axis,
        }
    }
}

impl View for Slider {
    fn body(&self, _cx: &Scope) -> AnyView {
        let snapshot = self.snapshot();
        let _ = ControlSizeEnvironment::key();
        AnyView::new(SliderHost {
            value: snapshot.value.clone(),
            range: snapshot.range,
            step: snapshot.step,
            is_enabled: snapshot.is_enabled,
            on_editing_changed: snapshot.on_editing_changed.clone(),
            shows_ticks: snapshot.shows_ticks,
            axis: snapshot.axis,
        })
    }
}
