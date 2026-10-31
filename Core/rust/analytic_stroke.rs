//! Translation of CoveUICore/AnalyticStroke.swift.
use crate::{Color, DrawError, DrawList, Point, Vertex};
unsafe extern "C" {
    fn cove_shape_path(
        path: *const Point,
        count: usize,
        width: f32,
        scale: f32,
        color: u32,
        closed: std::ffi::c_int,
        vertices: *mut Vertex,
        indices: *mut u32,
    );
}
impl DrawList {
    pub fn add_path_stroke(
        &mut self,
        points: &[Point],
        width: f32,
        color: Color,
        closed: bool,
    ) -> Result<(), DrawError> {
        if !width.is_finite() || width <= 0.0 || color.a <= 0.0 {
            return Ok(());
        }
        let mut path = Vec::with_capacity(points.len());
        for &point in points {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Ok(());
            }
            if path.last() != Some(&point) {
                path.push(point);
            }
        }
        if closed && path.len() > 1 && path.first() == path.last() {
            path.pop();
        }
        if path.len() < if closed { 3 } else { 2 } {
            return Ok(());
        }
        let vertex_count = path
            .len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(if closed { 0 } else { 8 }))
            .ok_or(DrawError::Overflow)?;
        let index_count = (path.len() - usize::from(!closed))
            .checked_mul(18)
            .and_then(|n| n.checked_add(if closed { 0 } else { 36 }))
            .ok_or(DrawError::Overflow)?;
        if vertex_count > u32::MAX as usize || index_count > u32::MAX as usize {
            return Err(DrawError::Overflow);
        }
        let feather = 0.5 / self.pixel_scale.max(1.0);
        let peak = (width / (width * 0.5 + feather)).min(1.0);
        let mut vertices = vec![Vertex::default(); vertex_count];
        let mut indices = vec![0; index_count];
        // SAFETY: path has the required minimum length, and C fills the exact precomputed vertex/index counts.
        unsafe {
            cove_shape_path(
                path.as_ptr(),
                path.len(),
                width,
                self.pixel_scale,
                color.multiplied_alpha(peak).rgba8(),
                i32::from(closed),
                vertices.as_mut_ptr(),
                indices.as_mut_ptr(),
            );
        }
        self.append_coverage_geometry(&vertices, &indices)
    }
    pub fn add_arc_stroke(
        &mut self,
        center: Point,
        radius: f32,
        start: f32,
        sweep: f32,
        width: f32,
        color: Color,
    ) -> Result<(), DrawError> {
        if !radius.is_finite()
            || radius <= 0.0
            || !start.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 0.0001
        {
            return Ok(());
        }
        let sweep = sweep.clamp(-2.0 * std::f32::consts::PI, 2.0 * std::f32::consts::PI);
        let count = (sweep.abs() * radius * self.pixel_scale.max(1.0))
            .ceil()
            .clamp(12.0, 512.0) as usize;
        let closed = sweep.abs() >= 2.0 * std::f32::consts::PI - 0.0001;
        let points: Vec<_> = (0..count + usize::from(!closed))
            .map(|i| {
                let angle = (start + i as f32 / count as f32 * sweep) as f64;
                Point {
                    x: center.x + angle.cos() as f32 * radius,
                    y: center.y + angle.sin() as f32 * radius,
                }
            })
            .collect();
        self.add_path_stroke(&points, width, color, closed)
    }
}
