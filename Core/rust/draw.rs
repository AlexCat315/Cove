use crate::{Color, Point, Rect};
use serde::{Deserialize, Serialize};
use std::{
    ffi::c_void,
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

unsafe extern "C" {
    fn cove_render_translate(vertices: *mut c_void, count: usize, x: f32, y: f32);
    fn cove_render_indices(indices: *mut u32, count: usize, offset: u32);
    fn cove_render_opacity(vertices: *mut c_void, count: usize, opacity: f32);
    fn cove_render_bounds(vertices: *const c_void, count: usize, bounds: *mut f32);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawBatch {
    pub index_offset: u32,
    pub index_count: u32,
    pub texture_id: u64,
    pub scissor: Option<Rect>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DrawError {
    #[error("draw buffer exceeds the 32-bit index range")]
    Overflow,
    #[error("an index or batch references data outside the draw buffer")]
    InvalidRange,
}

/// Clones share immutable buffers; editing detaches only the modified buffer.
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    vertices: Arc<Vec<Vertex>>,
    indices: Arc<Vec<u32>>,
    batches: Arc<Vec<DrawBatch>>,
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
            vertices: Arc::new(vertices),
            indices: Arc::new(indices),
            batches: Arc::new(batches),
        })
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
        *self = Self::default();
    }
    pub fn paint_bounds(&self) -> Option<Rect> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut bounds = [0.0_f32; 4];
        // SAFETY: Vertex is repr(C), matches the 20-byte C ABI; both slices are live.
        unsafe {
            cove_render_bounds(
                self.vertices.as_ptr().cast(),
                self.vertices.len(),
                bounds.as_mut_ptr(),
            );
        }
        Some(Rect::new(bounds[0], bounds[1], bounds[2], bounds[3]))
    }
    pub fn multiply_opacity(&mut self, start: usize, opacity: f32) {
        let start = start.min(self.vertices.len());
        if start == self.vertices.len() {
            return;
        }
        let opacity = if opacity.is_nan() {
            0.0
        } else {
            opacity.clamp(0.0, 1.0)
        };
        let vertices = &mut Arc::make_mut(&mut self.vertices)[start..];
        // SAFETY: exclusive slice and repr(C) layout as checked above.
        unsafe {
            cove_render_opacity(vertices.as_mut_ptr().cast(), vertices.len(), opacity);
        }
    }
    pub fn append(&mut self, source: &Self, translation: Point) -> Result<(), DrawError> {
        let vertex_base = u32::try_from(self.vertices.len()).map_err(|_| DrawError::Overflow)?;
        let index_base = u32::try_from(self.indices.len()).map_err(|_| DrawError::Overflow)?;
        if self
            .vertices
            .len()
            .checked_add(source.vertices.len())
            .is_none_or(|n| n > u32::MAX as usize)
            || self
                .indices
                .len()
                .checked_add(source.indices.len())
                .is_none_or(|n| n > u32::MAX as usize)
        {
            return Err(DrawError::Overflow);
        }
        if self.vertices.is_empty()
            && self.indices.is_empty()
            && self.batches.is_empty()
            && translation == Point::default()
        {
            *self = source.clone();
            return Ok(());
        }
        let vertices = Arc::make_mut(&mut self.vertices);
        vertices.extend_from_slice(source.vertices());
        let tail = &mut vertices[vertex_base as usize..];
        // SAFETY: only newly appended, exclusively owned vertices are modified.
        unsafe {
            cove_render_translate(
                tail.as_mut_ptr().cast(),
                tail.len(),
                translation.x,
                translation.y,
            );
        }
        let indices = Arc::make_mut(&mut self.indices);
        indices.extend_from_slice(source.indices());
        let tail = &mut indices[index_base as usize..];
        // SAFETY: indices were validated on construction; total vertex count fits u32.
        unsafe {
            cove_render_indices(tail.as_mut_ptr(), tail.len(), vertex_base);
        }
        Arc::make_mut(&mut self.batches).extend(source.batches.iter().map(|b| DrawBatch {
            index_offset: b.index_offset + index_base,
            ..b.clone()
        }));
        Ok(())
    }
    pub fn add_rect(&mut self, rect: Rect, color: Color) -> Result<(), DrawError> {
        let c = color.rgba8();
        let vertices = [
            (rect.x, rect.y),
            (rect.x + rect.width, rect.y),
            (rect.x + rect.width, rect.y + rect.height),
            (rect.x, rect.y + rect.height),
        ]
        .map(|(x, y)| Vertex {
            x,
            y,
            color: c,
            ..Vertex::default()
        })
        .to_vec();
        self.append(
            &Self::from_parts(
                vertices,
                vec![0, 1, 2, 0, 2, 3],
                vec![DrawBatch {
                    index_offset: 0,
                    index_count: 6,
                    texture_id: 0,
                    scissor: None,
                }],
            )?,
            Point::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
