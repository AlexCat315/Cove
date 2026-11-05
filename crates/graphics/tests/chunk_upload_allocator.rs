//! Translation of Graphics/Tests/NativeRHITests/ChunkUploadAllocatorTests.swift.

use std::sync::{Arc, Mutex};

use cove_graphics::{
    Buffer, ChunkUploadAllocator, FrameUploader, RhiResult, UploadLocation, UploadRingChunk,
};

struct TestChunk {
    storage: Arc<Mutex<Vec<u8>>>,
}

impl TestChunk {
    fn new(capacity: usize) -> Self {
        Self {
            storage: Arc::new(Mutex::new(vec![0; capacity])),
        }
    }
}

impl UploadRingChunk for TestChunk {
    fn capacity(&self) -> usize {
        self.storage.lock().unwrap().len()
    }

    unsafe fn cpu_pointer(&self) -> *mut u8 {
        self.storage.lock().unwrap().as_mut_ptr()
    }
}

impl Clone for TestChunk {
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
        }
    }
}

struct BorrowedBytesUploader {
    copied: Vec<u8>,
}

impl FrameUploader for BorrowedBytesUploader {
    fn reset(&mut self) {
        self.copied.clear();
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadLocation> {
        self.copied = bytes.to_vec();
        Ok(UploadLocation::new(Buffer::new(1), alignment))
    }
}

#[test]
fn borrowed_bytes_dispatch_to_backend_through_protocol() {
    let mut backend = BorrowedBytesUploader { copied: Vec::new() };
    let mut source: [u8; 4] = [17, 29, 41, 53];
    let uploader: &mut dyn FrameUploader = &mut backend;
    let location = uploader.write(&source, 64).unwrap();
    source[0] = 99;
    assert_ne!(source[0], backend.copied[0]);
    assert_eq!(backend.copied, vec![17, 29, 41, 53]);
    assert_eq!(location.offset, 64);
}

fn make_allocator(chunk_size: usize) -> ChunkUploadAllocator<TestChunk> {
    ChunkUploadAllocator::new(chunk_size, |capacity| Ok(TestChunk::new(capacity)))
}

#[test]
fn first_allocation_starts_at_zero() {
    let mut allocator = make_allocator(1024);
    let allocation = allocator.allocate(100, 1).unwrap();
    assert_eq!(allocation.offset(), 0);
    // The allocator floors chunk size to 64 KiB.
    assert_eq!(allocation.chunk().capacity(), 64 * 1024);
}

#[test]
fn allocations_respect_alignment() {
    let mut allocator = make_allocator(1024);
    allocator.allocate(100, 1).unwrap();
    let second = allocator.allocate(100, 256).unwrap();
    assert_eq!(second.offset(), 256);
}

#[test]
fn write_copies_data() {
    let mut allocator = make_allocator(1024);
    let data = [1u8, 2, 3, 4];
    let allocation = allocator.write(&data, 1).unwrap();
    // SAFETY: the allocation owns a live 64 KiB chunk buffer and 4 bytes fit.
    let bytes = unsafe { std::slice::from_raw_parts(allocation.pointer(), 4).to_vec() };
    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[test]
fn allocation_larger_than_chunk_grows_new_chunk() {
    let mut allocator = make_allocator(1024);
    let large = allocator.allocate(4096, 1).unwrap();
    assert!(large.chunk().capacity() >= 4096);
    assert_eq!(large.offset(), 0);
}

#[test]
fn zero_size_throws() {
    let mut allocator = make_allocator(1024);
    assert!(allocator.allocate(0, 1).is_err());
}

#[test]
fn reset_returns_allocations_to_zero() {
    let mut allocator = make_allocator(1024);
    allocator.allocate(500, 1).unwrap();
    allocator.reset();
    let after = allocator.allocate(100, 1).unwrap();
    assert_eq!(after.offset(), 0);
}
