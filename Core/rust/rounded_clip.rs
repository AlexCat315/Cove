//! Translation of CoveUICore/RoundedClip.swift; polygon clipping uses the shared C implementation.
use crate::{DrawError, Point, Rect, Vertex};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip { pub rect: Rect, pub mask: Option<RoundedRect> }
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundedRect { pub rect: Rect, pub radius: f32 }
impl RoundedRect {
    pub fn new(rect: Rect, radius: f32) -> Self { Self { rect, radius: radius.min(rect.width.min(rect.height) / 2.0).max(0.0) } }
    pub fn contains_bounds(self, bounds: Rect) -> bool {
        let r = self.rect;
        let inside = bounds.x >= r.x && bounds.x + bounds.width <= r.x + r.width && bounds.y >= r.y && bounds.y + bounds.height <= r.y + r.height;
        inside && ((bounds.x >= r.x + self.radius && bounds.x + bounds.width <= r.x + r.width - self.radius)
            || (bounds.y >= r.y + self.radius && bounds.y + bounds.height <= r.y + r.height - self.radius))
    }
    pub fn touches_corner(self, bounds: Rect) -> bool {
        if self.radius <= 0.0 { return false; }
        let r = self.rect; let radius = self.radius;
        [Rect::new(r.x, r.y, radius, radius), Rect::new(r.x + r.width - radius, r.y, radius, radius),
            Rect::new(r.x, r.y + r.height - radius, radius, radius), Rect::new(r.x + r.width - radius, r.y + r.height - radius, radius, radius)]
            .iter().any(|corner| bounds.x < corner.x + corner.width && bounds.x + bounds.width > corner.x && bounds.y < corner.y + corner.height && bounds.y + bounds.height > corner.y)
    }
    pub fn outline(self, segments: usize) -> Vec<Point> {
        let segments = segments.clamp(1, 4096); let mut points = vec![Point::default(); 4 * (segments + 1)];
        // SAFETY: Point is repr(C) with two f32 values; C fills exactly four (segments + 1) points.
        unsafe { cove_rounded_outline(self.rect.x, self.rect.y, self.rect.width, self.rect.height, self.radius, segments, points.as_mut_ptr()); } points
    }
    pub fn inset(self, amount: f32) -> Self {
        Self::new(Rect::new(self.rect.x + amount, self.rect.y + amount, (self.rect.width - 2.0 * amount).max(0.0), (self.rect.height - 2.0 * amount).max(0.0)), (self.radius - amount).max(0.0))
    }
    pub fn signed_distance(self, vertex: Vertex) -> f32 {
        let r = self.rect; let x = (vertex.x - r.x - r.width / 2.0).abs() - r.width / 2.0 + self.radius;
        let y = (vertex.y - r.y - r.height / 2.0).abs() - r.height / 2.0 + self.radius;
        (x.max(0.0).powi(2) + y.max(0.0).powi(2)).sqrt() + x.max(y).min(0.0) - self.radius
    }
}
#[repr(C)]
struct ClipResult { vertices: *mut Vertex, count: usize, failed: std::ffi::c_int }
impl Drop for ClipResult {
    fn drop(&mut self) {
        // SAFETY: C returns a malloc allocation or null, and this guard is its unique owner.
        unsafe { cove_clip_free(self.vertices); }
    }
}
unsafe extern "C" {
    fn cove_rounded_outline(x: f32, y: f32, width: f32, height: f32, radius: f32, segments: usize, output: *mut Point);
    fn cove_clip_polygon(input: *const Vertex, count: usize, outline: *const Point, edges: usize) -> ClipResult;
    fn cove_clip_free(vertices: *mut Vertex);
}
pub(crate) fn clip_polygon(vertices: &[Vertex], outlines: &[Vec<Point>]) -> Result<Vec<Vertex>, DrawError> {
    let mut polygon = vertices.to_vec();
    for outline in outlines {
        if polygon.len() < 3 { return Ok(vec![]); }
        // SAFETY: repr(C) slices remain live; C borrows input and returns a separate allocation owned by ClipResult.
        let result = unsafe { cove_clip_polygon(polygon.as_ptr(), polygon.len(), outline.as_ptr(), outline.len()) };
        if result.failed != 0 { return Err(DrawError::Allocation); }
        if result.count == 0 { return Ok(vec![]); }
        // SAFETY: successful C result contains count initialized vertices until its guard is dropped.
        polygon = unsafe { std::slice::from_raw_parts(result.vertices, result.count) }.to_vec();
    }
    Ok(polygon)
}
