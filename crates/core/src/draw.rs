//! Translation of CoveUICore/DrawList.swift. Primitive emission is in draw_primitives.rs.
use crate::{
    Clip, DrawListResources, GeometryAntialiasing, Point, Rect, Reveal, RoundedRect, TextureId,
};
use crate::{analytic_clipper::analytic_clip, geometry_kernels, rounded_clip::clip_polygon};
use serde::{Deserialize, Serialize};
use std::{
    mem::{align_of, size_of},
    sync::Arc,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub color: u32,
}
const _: () = assert!(size_of::<Vertex>() == 20);
const _: () = assert!(align_of::<Vertex>() == 4);
impl Vertex {
    pub const STRIDE: usize = 20;
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Backdrop {
    pub rect: Rect,
    pub corner_radius: f32,
    pub blur_radius: f32,
    pub opacity: f32,
}
impl Backdrop {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            corner_radius: 0.0,
            blur_radius: 12.0,
            opacity: 1.0,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DrawBatch {
    pub index_offset: u32,
    pub index_count: u32,
    pub texture_id: TextureId,
    pub scissor: Option<Rect>,
    #[serde(default)]
    pub backdrop: Option<Backdrop>,
}
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DrawError {
    #[error("draw buffer exceeds the 32-bit index range")]
    Overflow,
    #[error("an index or batch references data outside the draw buffer")]
    InvalidRange,
    #[error("native clipping allocation failed")]
    Allocation,
}
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub(crate) vertices: Arc<Vec<Vertex>>,
    pub(crate) indices: Arc<Vec<u32>>,
    pub(crate) batches: Arc<Vec<DrawBatch>>,
    pub resources: DrawListResources,
    pub pixel_scale: f32,
    pub geometry_antialiasing: GeometryAntialiasing,
    clip_stack: Vec<Rect>,
    pub clip_revision: u64,
    masks: Vec<RoundedRect>,
    mask_outlines: Vec<Vec<Point>>,
    pub viewport_bounds: Option<Rect>,
    pending_backdrop: Option<Backdrop>,
    pub reveal: Option<Reveal>,
}
impl DrawList {
    pub fn from_parts(
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
        batches: Vec<DrawBatch>,
    ) -> Result<Self, DrawError> {
        if vertices.len() > u32::MAX as usize || indices.len() > u32::MAX as usize {
            return Err(DrawError::Overflow);
        }
        if indices.iter().any(|&i| i as usize >= vertices.len())
            || batches.iter().any(|b| {
                b.index_offset
                    .checked_add(b.index_count)
                    .is_none_or(|end| end as usize > indices.len())
            })
        {
            return Err(DrawError::InvalidRange);
        }
        Ok(Self {
            clip_revision: u64::from(batches.iter().any(|b| b.scissor.is_some())),
            vertices: Arc::new(vertices),
            indices: Arc::new(indices),
            batches: Arc::new(batches),
            ..Self::default()
        })
    }
    pub fn geometry_shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.vertices, &other.vertices)
            && Arc::ptr_eq(&self.indices, &other.indices)
            && Arc::ptr_eq(&self.batches, &other.batches)
    }
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
    pub fn batches(&self) -> &[DrawBatch] {
        &self.batches
    }
    pub fn clear(&mut self) {
        self.reset();
    }
    pub fn reset(&mut self) {
        Arc::make_mut(&mut self.vertices).clear();
        Arc::make_mut(&mut self.indices).clear();
        Arc::make_mut(&mut self.batches).clear();
        self.resources.reset();
        self.clip_stack.clear();
        self.masks.clear();
        self.mask_outlines.clear();
        self.pending_backdrop = None;
        self.viewport_bounds = None;
        self.reveal = None;
    }
    pub fn load(
        &mut self,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
        batches: Vec<DrawBatch>,
        resources: DrawListResources,
    ) -> Result<(), DrawError> {
        let replacement = Self::from_parts(vertices, indices, batches)?;
        if replacement.clip_revision != 0 {
            self.clip_revision = self.clip_revision.wrapping_add(1);
        }
        self.vertices = replacement.vertices;
        self.indices = replacement.indices;
        self.batches = replacement.batches;
        self.resources = resources;
        self.clip_stack.clear();
        self.masks.clear();
        self.mask_outlines.clear();
        self.viewport_bounds = None;
        self.reveal = None;
        Ok(())
    }
    pub fn begin_reveal(&mut self, reveal: Reveal) {
        self.reveal = Some(reveal);
    }
    pub fn set_viewport_bounds(&mut self, bounds: Option<Rect>) {
        self.viewport_bounds = bounds;
    }
    pub fn begin_backdrop(&mut self, backdrop: Backdrop) {
        self.pending_backdrop = Some(backdrop);
    }
    pub fn paint_bounds(&self) -> Option<Rect> {
        geometry_kernels::bounds(&self.vertices)
    }
    pub fn push_clip_rect(&mut self, rect: Rect) {
        self.clip_revision = self.clip_revision.wrapping_add(1);
        self.clip_stack.push(
            self.current_clip()
                .map_or(rect, |parent| intersect(parent, rect)),
        );
    }
    pub fn pop_clip_rect(&mut self) {
        self.clip_stack.pop();
    }
    pub fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }
    pub fn push_clip(&mut self, clip: Clip) {
        self.push_clip_rect(clip.rect);
        if let Some(mask) = clip.mask {
            self.push_mask(mask);
        }
    }
    pub fn pop_clip(&mut self, clip: Clip) {
        if clip.mask.is_some() {
            self.pop_mask();
        }
        self.pop_clip_rect();
    }
    pub fn push_mask(&mut self, mask: RoundedRect) {
        self.masks.push(mask);
        self.mask_outlines.push(if mask.radius > 0.0 {
            mask.outline(self.corner_segment_count(mask.radius))
        } else {
            vec![]
        });
    }
    pub fn pop_mask(&mut self) {
        self.masks.pop();
        self.mask_outlines.pop();
    }
    pub fn current_masks(&self) -> &[RoundedRect] {
        &self.masks
    }
    pub fn multiply_opacity(&mut self, start: usize, opacity: f32) {
        let start = start.min(self.vertices.len());
        let alpha = if opacity.is_nan() {
            0.0
        } else {
            opacity.clamp(0.0, 1.0)
        };
        if alpha >= 1.0 {
            return;
        }
        geometry_kernels::opacity(&mut Arc::make_mut(&mut self.vertices)[start..], alpha);
        for batch in Arc::make_mut(&mut self.batches) {
            if let Some(backdrop) = &mut batch.backdrop
                && self
                    .indices
                    .get(batch.index_offset as usize)
                    .is_some_and(|&index| index as usize >= start)
                {
                    backdrop.opacity *= alpha;
                }
        }
    }
    pub(crate) fn recolor(&mut self, start: usize, color: impl Fn(f32, f32, f32) -> crate::Color) {
        for vertex in &mut Arc::make_mut(&mut self.vertices)[start..] {
            vertex.color = color(vertex.x, vertex.y, (vertex.color >> 24) as f32 / 255.0).rgba8();
        }
    }
    pub fn append(&mut self, source: &Self, translation: Point) -> Result<(), DrawError> {
        let first = self.batches.len();
        self.append_unmasked(source, translation)?;
        if self.masks.iter().any(|m| m.radius > 0.0) {
            self.mask_batches(first)?;
        }
        Ok(())
    }
    pub fn append_preclipped(
        &mut self,
        source: &Self,
        clips: &[Clip],
        translation: Point,
    ) -> Result<(), DrawError> {
        if self
            .masks
            .iter()
            .all(|mask| clips.iter().any(|clip| clip.mask.as_ref() == Some(mask)))
        {
            self.append_unmasked(source, translation)
        } else {
            self.append(source, translation)
        }
    }
    fn append_unmasked(&mut self, source: &Self, translation: Point) -> Result<(), DrawError> {
        self.check_capacity(source.vertices.len(), source.indices.len())?;
        if source.clip_revision != 0 {
            self.clip_revision = self.clip_revision.wrapping_add(1);
        }
        self.resources.append(&source.resources);
        if let Some(reveal) = source.reveal {
            self.reveal = Some(reveal);
        }
        if self.vertices.is_empty()
            && self.indices.is_empty()
            && self.batches.is_empty()
            && translation == Point::default()
        {
            self.vertices = source.vertices.clone();
            self.indices = source.indices.clone();
            self.batches = source.batches.clone();
            return Ok(());
        }
        let vertex_base = self.vertices.len() as u32;
        let index_base = self.indices.len() as u32;
        let vertices = Arc::make_mut(&mut self.vertices);
        vertices.extend_from_slice(&source.vertices);
        if translation != Point::default() {
            geometry_kernels::translate(
                &mut vertices[vertex_base as usize..],
                translation.x,
                translation.y,
            );
        }
        let indices = Arc::make_mut(&mut self.indices);
        indices.extend_from_slice(&source.indices);
        if vertex_base != 0 {
            geometry_kernels::rebase(&mut indices[index_base as usize..], vertex_base);
        }
        Arc::make_mut(&mut self.batches).extend(source.batches.iter().cloned().map(|batch| {
            DrawBatch {
                index_offset: batch.index_offset + index_base,
                ..batch
            }
        }));
        Ok(())
    }
    fn mask_batches(&mut self, first: usize) -> Result<(), DrawError> {
        if first >= self.batches.len() {
            return Ok(());
        }
        let start = self.batches[first].index_offset as usize;
        let original = self.indices[start..].to_vec();
        Arc::make_mut(&mut self.indices).truncate(start);
        let mut offset = start as u32;
        for i in first..self.batches.len() {
            let batch = self.batches[i].clone();
            let local = batch.index_offset as usize - start;
            let before = self.indices.len() as u32;
            Arc::make_mut(&mut self.indices)
                .extend_from_slice(&original[local..local + batch.index_count as usize]);
            let count = self.apply_masks(before, batch.index_count)?;
            let batch = &mut Arc::make_mut(&mut self.batches)[i];
            batch.index_offset = offset;
            batch.index_count = count;
            offset += count;
        }
        Ok(())
    }
    pub(crate) fn check_capacity(&self, vertices: usize, indices: usize) -> Result<(), DrawError> {
        if self
            .vertices
            .len()
            .checked_add(vertices)
            .is_none_or(|n| n > u32::MAX as usize)
            || self
                .indices
                .len()
                .checked_add(indices)
                .is_none_or(|n| n > u32::MAX as usize)
        {
            return Err(DrawError::Overflow);
        }
        Ok(())
    }
    pub(crate) fn append_coverage_geometry(
        &mut self,
        points: &[Vertex],
        triangles: &[u32],
    ) -> Result<(), DrawError> {
        self.append_geometry(points, triangles, 0)
    }
    pub(crate) fn append_geometry(
        &mut self,
        points: &[Vertex],
        triangles: &[u32],
        texture: TextureId,
    ) -> Result<(), DrawError> {
        self.check_capacity(points.len(), triangles.len())?;
        if triangles.iter().any(|&i| i as usize >= points.len()) {
            return Err(DrawError::InvalidRange);
        }
        let vertex_base = self.vertices.len() as u32;
        let index_base = self.indices.len() as u32;
        Arc::make_mut(&mut self.vertices).extend_from_slice(points);
        let indices = Arc::make_mut(&mut self.indices);
        indices.extend_from_slice(triangles);
        geometry_kernels::rebase(&mut indices[index_base as usize..], vertex_base);
        self.record_indices(index_base, triangles.len() as u32, texture)
    }
    fn record_indices(
        &mut self,
        start: u32,
        count: u32,
        texture_id: TextureId,
    ) -> Result<(), DrawError> {
        let count = if self.masks.iter().any(|m| m.radius > 0.0) {
            self.apply_masks(start, count)?
        } else {
            count
        };
        if count == 0 {
            self.pending_backdrop = None;
            return Ok(());
        }
        let scissor = self.current_clip();
        let backdrop = self.pending_backdrop.take();
        let batches = Arc::make_mut(&mut self.batches);
        if backdrop.is_none()
            && let Some(last) = batches.last_mut()
            && last.texture_id == texture_id
                && last.scissor == scissor
                && last.index_offset + last.index_count == start
            {
                last.index_count += count;
                return Ok(());
            }
        batches.push(DrawBatch {
            index_offset: start,
            index_count: count,
            texture_id,
            scissor,
            backdrop,
        });
        Ok(())
    }
    fn apply_masks(&mut self, base: u32, count: u32) -> Result<u32, DrawError> {
        let start = base as usize;
        let end = start + count as usize;
        let active: Vec<_> = self
            .masks
            .iter()
            .enumerate()
            .filter(|(_, m)| m.radius > 0.0)
            .map(|(i, _)| i)
            .collect();
        let analytic = self.geometry_antialiasing == GeometryAntialiasing::Analytic;
        let half = if analytic {
            0.5 / self.pixel_scale.max(1.0)
        } else {
            0.0
        };
        let contains = |i: usize, bounds: Rect| self.masks[i].inset(half).contains_bounds(bounds);
        let outside = |bounds: Rect| {
            active.iter().any(|&i| {
                let r = self.masks[i].rect;
                bounds.x + bounds.width < r.x - half
                    || bounds.x > r.x + r.width + half
                    || bounds.y + bounds.height < r.y - half
                    || bounds.y > r.y + r.height + half
            })
        };
        let emitted = self.indices[start..end].to_vec();
        let bounds_of = |indices: &[u32]| {
            let mut x = f32::INFINITY;
            let mut y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for &i in indices {
                let v = self.vertices[i as usize];
                x = x.min(v.x);
                y = y.min(v.y);
                max_x = max_x.max(v.x);
                max_y = max_y.max(v.y);
            }
            Rect::new(x, y, max_x - x, max_y - y)
        };
        let bounds = bounds_of(&emitted);
        if active.iter().all(|&i| contains(i, bounds)) {
            return Ok(count);
        }
        if outside(bounds) {
            Arc::make_mut(&mut self.indices).drain(start..end);
            return Ok(0);
        }
        let mut kept = Vec::with_capacity(emitted.len());
        let mut extra = Vec::new();
        let mut changed = false;
        for triangle in emitted.as_chunks::<3>().0 {
            let bounds = bounds_of(triangle);
            if outside(bounds) {
                changed = true;
                continue;
            }
            let intersecting: Vec<_> = active
                .iter()
                .copied()
                .filter(|&i| !contains(i, bounds))
                .collect();
            if intersecting.is_empty() {
                kept.extend_from_slice(triangle);
                continue;
            }
            changed = true;
            let points = triangle
                .iter()
                .map(|&i| self.vertices[i as usize])
                .collect::<Vec<_>>();
            let polygons = if analytic {
                analytic_clip(
                    &points,
                    &intersecting
                        .iter()
                        .map(|&i| self.masks[i])
                        .collect::<Vec<_>>(),
                    self.pixel_scale,
                )?
            } else {
                let outlines = intersecting
                    .iter()
                    .map(|&i| {
                        let mask = self.masks[i];
                        let r = mask.rect;
                        if (bounds.y >= r.y + mask.radius
                            && bounds.y + bounds.height <= r.y + r.height - mask.radius)
                            || (bounds.x >= r.x + mask.radius
                                && bounds.x + bounds.width <= r.x + r.width - mask.radius)
                        {
                            vec![
                                Point { x: r.x, y: r.y },
                                Point {
                                    x: r.x + r.width,
                                    y: r.y,
                                },
                                Point {
                                    x: r.x + r.width,
                                    y: r.y + r.height,
                                },
                                Point {
                                    x: r.x,
                                    y: r.y + r.height,
                                },
                            ]
                        } else {
                            self.mask_outlines[i].clone()
                        }
                    })
                    .collect::<Vec<_>>();
                vec![clip_polygon(&points, &outlines)?]
            };
            for polygon in polygons {
                if polygon.len() < 3 {
                    continue;
                }
                self.check_capacity(
                    extra
                        .len()
                        .checked_add(polygon.len())
                        .ok_or(DrawError::Overflow)?,
                    kept.len()
                        .checked_add((polygon.len() - 2) * 3)
                        .ok_or(DrawError::Overflow)?,
                )?;
                let first = (self.vertices.len() + extra.len()) as u32;
                extra.extend_from_slice(&polygon);
                for fan in 1..polygon.len() - 1 {
                    kept.extend_from_slice(&[first, first + fan as u32, first + fan as u32 + 1]);
                }
            }
        }
        if !changed {
            return Ok(count);
        }
        Arc::make_mut(&mut self.vertices).extend(extra);
        let indices = Arc::make_mut(&mut self.indices);
        indices.splice(start..end, kept.iter().copied());
        u32::try_from(kept.len()).map_err(|_| DrawError::Overflow)
    }
    pub(crate) fn corner_segment_count(&self, radius: f32) -> usize {
        if radius <= 0.0 {
            return 10;
        }
        ((std::f32::consts::PI / 2.0 * radius.max(0.0)) / (0.8 / self.pixel_scale.max(1.0)))
            .ceil()
            .clamp(10.0, 96.0) as usize
    }
    pub(crate) fn snapped(&self, rect: Rect) -> Rect {
        if self.pixel_scale <= 0.0 {
            return rect;
        }
        let snap = |v: f32| (v * self.pixel_scale).round() / self.pixel_scale;
        let x = snap(rect.x);
        let y = snap(rect.y);
        Rect::new(
            x,
            y,
            (snap(rect.x + rect.width) - x).max(0.0),
            (snap(rect.y + rect.height) - y).max(0.0),
        )
    }
}
fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect::new(
        x,
        y,
        (a.x + a.width).min(b.x + b.width).max(x) - x,
        (a.y + a.height).min(b.y + b.height).max(y) - y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    #[test]
    fn bulk_copy_and_opacity_preserve_shared_snapshot() {
        for count in [0, 1, 15, 16_383, 16_384, 32_771, 131_079] {
            let vertices = (0..count)
                .map(|i| Vertex {
                    x: i as f32 * 0.25,
                    y: -(i as f32),
                    u: (i % 7) as f32,
                    v: (i % 11) as f32,
                    color: ((i % 256) << 24) | 0x123456,
                })
                .collect();
            let source = DrawList::from_parts(
                vertices,
                (0..count).collect(),
                if count == 0 {
                    vec![]
                } else {
                    vec![DrawBatch {
                        index_offset: 0,
                        index_count: count,
                        texture_id: 42,
                        scissor: Some(Rect::new(1.0, 2.0, 3.0, 4.0)),
                        backdrop: None,
                    }]
                },
            )
            .unwrap();
            let snapshot = source.clone();
            let mut target = DrawList::default();
            target.append(&source, Point::default()).unwrap();
            target.append(&source, Point { x: 3.5, y: -2.25 }).unwrap();
            for (i, v) in target.vertices().iter().skip(count as usize).enumerate() {
                let original = snapshot.vertices()[i];
                assert_eq!(
                    (v.x, v.y, v.u, v.v),
                    (original.x + 3.5, original.y - 2.25, original.u, original.v)
                );
                assert_eq!(target.indices()[i + count as usize], i as u32 + count);
            }
            target.multiply_opacity(17, 0.5);
            assert_eq!(source.vertices(), snapshot.vertices());
            for (i, v) in target.vertices().iter().enumerate() {
                let original = snapshot.vertices()[i % (count.max(1) as usize)];
                let a = if i < 17 {
                    original.color >> 24
                } else {
                    ((original.color >> 24) as f32 * 0.5).round() as u32
                };
                assert_eq!(v.color, a << 24 | 0x123456);
            }
        }
    }
    #[test]
    fn invalid_buffers_are_rejected_before_ffi() {
        assert_eq!(
            DrawList::from_parts(vec![], vec![1], vec![]).unwrap_err(),
            DrawError::InvalidRange
        );
        assert!(
            DrawList::from_parts(
                vec![Vertex::default()],
                vec![0],
                vec![DrawBatch {
                    index_offset: u32::MAX,
                    index_count: 1,
                    texture_id: 0,
                    scissor: None,
                    backdrop: None,
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn bounds_include_overflow_geometry() {
        let mut list = DrawList::default();
        assert_eq!(list.paint_bounds(), None);
        list.add_rect(Rect::new(-20.0, -8.0, 30.0, 60.0), Color::WHITE)
            .unwrap();
        assert_eq!(
            list.paint_bounds(),
            Some(Rect::new(-20.0, -8.0, 30.0, 60.0))
        );
    }
}
