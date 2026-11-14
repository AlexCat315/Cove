//! Probing the binding-set cache must not allocate.
//!
//! The UI re-probes the same (layout, texture, sampler) triple for every batch of
//! every frame. That path used to copy the layout entries and build an owned
//! cache key before it consulted the cache, so a full cache hit still cost three
//! heap allocations per batch — steady churn in a 60 Hz render loop. This test
//! counts allocations directly instead of trusting a sampling profiler.

#![cfg(target_os = "macos")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use cove_graphics::{
    BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSetEntry, BindingType,
    BufferDescriptor, BufferUsage, Device, DeviceConfig, GraphicsApi, ShaderVisibility,
};

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every method forwards to the system allocator unchanged and only adds a
// counter, so the returned pointers and their contracts are exactly System's.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, new_size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn device() -> std::sync::Arc<Device> {
    std::sync::Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a Metal device is available"),
    )
}

#[test]
fn a_repeated_binding_set_probe_reuses_its_set_without_allocating() {
    let device = device();
    let layout = device
        .make_binding_layout(BindingLayoutDescriptor::new(vec![BindingLayoutEntry::new(
            0,
            BindingType::UniformBuffer,
            ShaderVisibility::GRAPHICS,
        )]))
        .expect("layout");
    let buffer = device
        .make_buffer(BufferDescriptor::new(256, BufferUsage::UNIFORM))
        .expect("buffer");
    let entries = [BindingSetEntry::new(
        0,
        BindingResource::UniformBuffer {
            buffer,
            offset: 0,
            size: Some(16),
        },
    )];

    let registered = device
        .make_binding_set_entries(layout, &entries)
        .expect("the first probe builds the set");
    // The second probe warms the reusable key buffer; everything after it is a
    // pure cache hit.
    let warm = device
        .make_binding_set_entries(layout, &entries)
        .expect("the second probe hits the cache");
    assert_eq!(
        registered.id(),
        warm.id(),
        "the same entries must resolve to the same set"
    );

    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for _ in 0..256 {
        let hit = device
            .make_binding_set_entries(layout, &entries)
            .expect("a warmed probe keeps hitting the cache");
        assert_eq!(hit.id(), registered.id());
    }
    let after = ALLOCATIONS.load(Ordering::Relaxed);
    assert_eq!(
        after,
        before,
        "a binding-set cache hit must not allocate: {} allocations in 256 probes",
        after - before
    );
}
