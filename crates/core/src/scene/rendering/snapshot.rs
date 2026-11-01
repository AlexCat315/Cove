//! Translation of CoveUIScene/Rendering/Snapshot.swift.
use crate::{DrawBatch, DrawListResources, GlyphAtlasFormat, TextureId, Vertex};

/// Atlas upload payload captured on the main thread and consumed by the render
/// thread.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawListAtlasDirty {
    pub pixels: Vec<u8>,
    pub region_x: u32,
    pub region_y: u32,
    pub region_width: u32,
    pub region_height: u32,
    pub texture_width: u32,
    pub texture_height: u32,
    pub texture_id: TextureId,
    pub format: GlyphAtlasFormat,
}
impl DrawListAtlasDirty {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pixels: Vec<u8>,
        region_x: u32,
        region_y: u32,
        region_width: u32,
        region_height: u32,
        texture_width: u32,
        texture_height: u32,
        texture_id: TextureId,
        format: GlyphAtlasFormat,
    ) -> Self {
        Self {
            pixels,
            region_x,
            region_y,
            region_width,
            region_height,
            texture_width,
            texture_height,
            texture_id,
            format,
        }
    }
}

/// Immutable snapshot of one frame's in-game UI draw commands, safe to pass
/// across the main→render thread boundary.
#[derive(Clone, Debug, Default)]
pub struct DrawListSnapshot {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<DrawBatch>,
    pub logical_size: [f32; 2],
    pub atlas_updates: Vec<DrawListAtlasDirty>,
    pub resources: DrawListResources,
}
impl DrawListSnapshot {
    pub fn new(
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
        batches: Vec<DrawBatch>,
        logical_size: [f32; 2],
    ) -> Self {
        Self {
            vertices,
            indices,
            batches,
            logical_size,
            atlas_updates: Vec::new(),
            resources: DrawListResources::default(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_reports_no_batches() {
        let snapshot = DrawListSnapshot::new(Vec::new(), Vec::new(), Vec::new(), [800.0, 600.0]);
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.logical_size, [800.0, 600.0]);
    }
}
