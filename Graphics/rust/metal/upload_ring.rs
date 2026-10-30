//! Translation of NativeRHI/Metal/Device/MetalUploadRing.swift.
//!
//! Each frames-in-flight slot owns a `ChunkUploadAllocator` whose chunks wrap a
//! shared-storage `MTLBuffer`. Suballocations stay alive for exactly the frame
//! that uses them (the slot is only reset after its GPU work completes), so
//! there is no per-upload staging alloc + commit + wait on the hot path.
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::MTLBuffer;

use crate::rhi::{
    Buffer, ChunkUploadAllocator, FrameUploader, RhiResult, UploadLocation, UploadRingChunk,
};

/// A chunk of GPU-visible upload storage: a shared-storage `MTLBuffer` plus the
/// `Buffer` handle it is registered under, so the frontend can bind it.
#[derive(Clone)]
pub struct MetalUploadChunk {
    buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    buffer_handle: Buffer,
    capacity: usize,
}
impl MetalUploadChunk {
    pub fn new(buffer: Retained<ProtocolObject<dyn MTLBuffer>>, handle: Buffer) -> Self {
        let capacity = buffer.length();
        Self { buffer, buffer_handle: handle, capacity }
    }
    pub fn buffer(&self) -> &Retained<ProtocolObject<dyn MTLBuffer>> {
        &self.buffer
    }
    pub const fn buffer_handle(&self) -> Buffer {
        self.buffer_handle
    }
}
// SAFETY: Metal resources are documented as safe to use from multiple threads;
// `MTLBuffer` retains its own atomic reference count and the allocator only
// hands out non-overlapping suballocations for one frame slot at a time.
unsafe impl Send for MetalUploadChunk {}

impl UploadRingChunk for MetalUploadChunk {
    fn capacity(&self) -> usize {
        self.capacity
    }

    unsafe fn cpu_pointer(&self) -> *mut u8 {
        // SAFETY: a shared-storage buffer exposes a CPU mapping valid for
        // `length()` bytes for as long as the chunk (which retains it) is alive.
        self.buffer.contents().as_ptr().cast()
    }
}

/// Per-frame uploader vended to the frontend. Wraps a
/// `ChunkUploadAllocator<MetalUploadChunk>` and translates allocations into
/// `UploadLocation`s the frontend can bind directly.
pub struct MetalFrameUploader {
    allocator: ChunkUploadAllocator<MetalUploadChunk>,
}
impl MetalFrameUploader {
    pub fn new(
        chunk_size: usize,
        make_chunk: impl FnMut(usize) -> RhiResult<MetalUploadChunk> + Send + 'static,
    ) -> Self {
        Self { allocator: ChunkUploadAllocator::new(chunk_size, make_chunk) }
    }
}
impl FrameUploader for MetalFrameUploader {
    fn reset(&mut self) {
        self.allocator.reset();
    }

    /// Zero-copy fast path: copies the caller's memory straight into ring
    /// storage without building an intermediate buffer.
    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadLocation> {
        let allocation = self.allocator.write(bytes, alignment)?;
        Ok(UploadLocation::new(allocation.chunk().buffer_handle, allocation.offset()))
    }
}
