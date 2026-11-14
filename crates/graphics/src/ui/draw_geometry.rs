use crate::rhi::{RhiError, RhiResult};
use cove_core::DrawList;
pub(super) struct NativeUIGeometry {
    snapshot: DrawList,
    pub relies_on_clip: Vec<bool>,
}
impl NativeUIGeometry {
    pub fn new(list: &DrawList) -> RhiResult<Self> {
        // SAFETY: slices are initialized contiguous storage; the C validator bounds-checks indices before any dereference.
        let valid = unsafe {
            cove_ui_validate_geometry(
                list.vertices().as_ptr(),
                list.vertices().len(),
                list.indices().as_ptr(),
                list.indices().len(),
            )
        } != 0;
        if !valid {
            return Err(RhiError::invalid_argument(describe_invalid_geometry(list)));
        }
        let mut relies_on_clip = Vec::with_capacity(list.batches().len());
        for batch in list.batches() {
            let start = batch.index_offset as usize;
            let count = batch.index_count as usize;
            if start > list.indices().len()
                || count > list.indices().len() - start
                || !count.is_multiple_of(3)
            {
                return Err(RhiError::invalid_argument(
                    "UI batch index range exceeds the draw list",
                ));
            }
            let relies = if let Some(rect) = batch.scissor {
                // SAFETY: all indices and this exact range were validated above; C borrows read-only storage.
                // Parenthesised: a block-like expression in statement position cannot take a binary operator.
                (unsafe {
                    cove_ui_relies_on_clip(
                        list.vertices().as_ptr(),
                        list.indices()[start..start + count].as_ptr(),
                        count,
                        rect.x - 0.01,
                        rect.y - 0.01,
                        rect.max_x() + 0.01,
                        rect.max_y() + 0.01,
                    )
                }) != 0
            } else {
                false
            };
            relies_on_clip.push(relies);
        }

        let mut snapshot = list.clone();
        snapshot.resources.reset();
        Ok(Self {
            snapshot,
            relies_on_clip,
        })
    }
    pub fn matches(&self, list: &DrawList) -> bool {
        self.snapshot.geometry_shares_storage(list)
    }
}

/// Names the first offending vertex or index, so a rejected frame reports what
/// went wrong instead of only that it did.
fn describe_invalid_geometry(list: &DrawList) -> String {
    if let Some((index, vertex)) = list.vertices().iter().enumerate().find(|(_, vertex)| {
        !(vertex.x.is_finite()
            && vertex.y.is_finite()
            && vertex.u.is_finite()
            && vertex.v.is_finite())
    }) {
        let batch = list.batches().iter().enumerate().find(|(_, batch)| {
            let start = batch.index_offset as usize;
            let count = batch.index_count as usize;
            list.indices()
                .get(start..start + count)
                .is_some_and(|indices| indices.contains(&(index as u32)))
        });
        let owner = match batch {
            Some((position, batch)) => format!(
                "batch {position} texture={:?} indices={}..{} scissor={:?}",
                batch.texture_id,
                batch.index_offset,
                batch.index_offset as usize + batch.index_count as usize,
                batch.scissor
            ),
            None => "no batch references it".to_owned(),
        };
        let nearby: Vec<String> = list
            .vertices()
            .iter()
            .skip(index.saturating_sub(4))
            .take(4)
            .map(|vertex| format!("({},{})", vertex.x, vertex.y))
            .collect();
        return format!(
            "invalid UI vertices or indices: vertex {index} of {} is not finite (x={} y={} u={} v={}), {owner}, quad corners {}",
            list.vertices().len(),
            vertex.x,
            vertex.y,
            vertex.u,
            vertex.v,
            nearby.join(" ")
        );
    }
    let vertex_count = list.vertices().len() as u32;
    match list
        .indices()
        .iter()
        .enumerate()
        .find(|(_, index)| **index >= vertex_count)
    {
        Some((offset, index)) => format!(
            "invalid UI vertices or indices: index {offset} of {} is {index}, past {vertex_count} vertices",
            list.indices().len()
        ),
        None => "invalid UI vertices or indices".to_owned(),
    }
}

unsafe extern "C" {
    fn cove_ui_validate_geometry(
        vertices: *const cove_core::Vertex,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
    ) -> i32;
    fn cove_ui_relies_on_clip(
        vertices: *const cove_core::Vertex,
        indices: *const u32,
        count: usize,
        x: f32,
        y: f32,
        r: f32,
        b: f32,
    ) -> i32;
}
