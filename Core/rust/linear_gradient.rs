//! Translation of CoveUICore/LinearGradient.swift.
use crate::{Color, Point, Rect};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearGradient { pub start: Color, pub end: Color, pub angle: f32 }
impl LinearGradient {
    pub fn color_at(self, point: Point, rect: Rect) -> Color {
        let radians = (self.angle * std::f32::consts::PI / 180.0) as f64;
        let dx = radians.sin() as f32; let dy = -radians.cos() as f32;
        let half_span = ((rect.width * dx).abs() + (rect.height * dy).abs()) / 2.0;
        if half_span <= 0.0 { return self.start; }
        let cx = rect.x + rect.width / 2.0; let cy = rect.y + rect.height / 2.0;
        let t = (((point.x - cx) * dx + (point.y - cy) * dy) / (2.0 * half_span) + 0.5).clamp(0.0, 1.0);
        let mix = |a, b| a + (b - a) * t;
        Color { r: mix(self.start.r, self.end.r), g: mix(self.start.g, self.end.g), b: mix(self.start.b, self.end.b), a: mix(self.start.a, self.end.a) }
    }
    pub fn multiplied_alpha(self, factor: f32) -> Self {
        Self { start: self.start.multiplied_alpha(factor), end: self.end.multiplied_alpha(factor), ..self }
    }
}
