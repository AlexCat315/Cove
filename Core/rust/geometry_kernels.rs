//! Translation of CoveUICore/GeometryKernels.swift; borrowed slices keep parallel writes disjoint.
use crate::{Rect, Vertex};
unsafe extern "C" {
    fn cove_render_translate(vertices: *mut std::ffi::c_void, count: usize, x: f32, y: f32);
    fn cove_render_indices(indices: *mut u32, count: usize, offset: u32);
    fn cove_render_opacity(vertices: *mut std::ffi::c_void, count: usize, opacity: f32);
    fn cove_render_bounds(vertices: *const std::ffi::c_void, count: usize, bounds: *mut f32);
}
fn chunks<T: Send>(values: &mut [T], body: impl Fn(&mut [T]) + Sync) {
    #[cfg(not(target_family = "wasm"))]
    {
        let workers = std::thread::available_parallelism().map(usize::from).unwrap_or(1).min(8);
        let jobs = workers.min(values.len() / 16384);
        if jobs > 1 {
            let size = values.len().div_ceil(jobs);
            std::thread::scope(|scope| { for chunk in values.chunks_mut(size) { let body = &body; scope.spawn(move || body(chunk)); } }); return;
        }
    }
    body(values);
}
pub(crate) fn translate(vertices: &mut [Vertex], x: f32, y: f32) {
    chunks(vertices, |chunk| {
        // SAFETY: each worker has an exclusive slice and Vertex matches the native 20-byte ABI.
        unsafe { cove_render_translate(chunk.as_mut_ptr().cast(), chunk.len(), x, y); }
    });
}
pub(crate) fn rebase(indices: &mut [u32], offset: u32) {
    chunks(indices, |chunk| {
        // SAFETY: workers write disjoint initialized u32 slices; callers validate the index range.
        unsafe { cove_render_indices(chunk.as_mut_ptr(), chunk.len(), offset); }
    });
}
pub(crate) fn opacity(vertices: &mut [Vertex], alpha: f32) {
    chunks(vertices, |chunk| {
        // SAFETY: workers write disjoint repr(C) Vertex slices; alpha has been clamped by the caller.
        unsafe { cove_render_opacity(chunk.as_mut_ptr().cast(), chunk.len(), alpha); }
    });
}
pub(crate) fn bounds(vertices: &[Vertex]) -> Option<Rect> {
    if vertices.is_empty() { return None; } let mut bounds = [0.0; 4];
    // SAFETY: the input is borrowed and C writes exactly four f32 values into the output.
    unsafe { cove_render_bounds(vertices.as_ptr().cast(), vertices.len(), bounds.as_mut_ptr()); }
    Some(Rect::new(bounds[0], bounds[1], bounds[2], bounds[3]))
}
