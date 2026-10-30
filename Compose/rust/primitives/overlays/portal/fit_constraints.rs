//! Popup width and height limits translated from Portal/FitConstraints.swift.
use cove_core::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortalFitConstraints {
    pub minimum_width: f32,
    pub viewport_margin: f32,
    pub maximum_width: Option<f32>,
    pub maximum_height: Option<f32>,
}

impl Default for PortalFitConstraints {
    fn default() -> Self {
        Self { minimum_width: 0.0, viewport_margin: 6.0, maximum_width: None, maximum_height: None }
    }
}

impl PortalFitConstraints {
    pub fn width_range(self, window: Rect) -> (f32, f32) {
        let available = (window.width - self.viewport_margin * 2.0).max(0.0);
        let maximum = self.maximum_width.map(|width| available.min(width.max(0.0))).unwrap_or(available);
        (maximum.min(self.minimum_width.max(0.0)), maximum)
    }

    pub fn height_limit(self, window: Rect) -> f32 {
        let available = (window.height - self.viewport_margin * 2.0).max(0.0);
        self.maximum_height.map(|height| available.min(height.max(0.0))).unwrap_or(available)
    }
}
