//! Translation of Graphics/Tests/NativeRHITests/CacheTests.swift.

use cove_graphics::{
    BindingResource, BindingSetCache, BindingSetEntry, Buffer, PipelineInterfaceKey,
    PipelineLayoutCache,
};

#[test]
fn layout_keyed_by_full_id_list() {
    let mut cache = PipelineLayoutCache::new();
    cache.insert(
        10,
        PipelineInterfaceKey { set_layouts: vec![1, 2], push_constants: vec![] },
    );
    cache.insert(
        11,
        PipelineInterfaceKey { set_layouts: vec![2, 1], push_constants: vec![] },
    );

    // Order matters: [1,2] and [2,1] are distinct layouts.
    assert_eq!(
        cache.layout(&PipelineInterfaceKey { set_layouts: vec![1, 2], push_constants: vec![] }),
        Some(10)
    );
    assert_eq!(
        cache.layout(&PipelineInterfaceKey { set_layouts: vec![2, 1], push_constants: vec![] }),
        Some(11)
    );
}

#[test]
fn unknown_layout_returns_none() {
    let cache = PipelineLayoutCache::new();
    assert_eq!(
        cache.layout(&PipelineInterfaceKey { set_layouts: vec![9], push_constants: vec![] }),
        None
    );
}

#[test]
fn reinserting_same_layout_overwrites() {
    let mut cache = PipelineLayoutCache::new();
    cache.insert(
        10,
        PipelineInterfaceKey { set_layouts: vec![1], push_constants: vec![] },
    );
    cache.insert(
        12,
        PipelineInterfaceKey { set_layouts: vec![1], push_constants: vec![] },
    );
    assert_eq!(
        cache.layout(&PipelineInterfaceKey { set_layouts: vec![1], push_constants: vec![] }),
        Some(12)
    );
}

#[test]
fn uniform_ranges_do_not_alias() {
    let mut cache = BindingSetCache::new();
    let small = [BindingSetEntry::new(
        0,
        BindingResource::UniformBuffer { buffer: Buffer::new(1), offset: 0, size: Some(16) },
    )];
    let large = [BindingSetEntry::new(
        0,
        BindingResource::UniformBuffer { buffer: Buffer::new(1), offset: 0, size: Some(192) },
    )];
    cache.insert(100, 5, &small);
    assert_eq!(cache.existing(5, &small), Some(100));
    assert_eq!(cache.existing(5, &large), None);
}

fn make_entry(slot: u32) -> BindingSetEntry {
    BindingSetEntry::new(slot, BindingResource::uniform_buffer(Buffer::new(slot + 1)))
}

#[test]
fn miss_then_hit() {
    let mut cache = BindingSetCache::new();
    let entries = [make_entry(0)];

    assert_eq!(cache.existing(5, &entries), None);
    assert_eq!(cache.stats().misses, 1);

    cache.insert(100, 5, &entries);
    assert_eq!(cache.existing(5, &entries), Some(100));
    assert_eq!(cache.stats().hits, 1);
}

#[test]
fn different_layouts_do_not_alias() {
    let mut cache = BindingSetCache::new();
    let entries = [make_entry(0)];
    cache.insert(100, 5, &entries);
    cache.insert(101, 6, &entries);

    assert_eq!(cache.existing(5, &entries), Some(100));
    assert_eq!(cache.existing(6, &entries), Some(101));
}

#[test]
fn eviction_does_not_trigger_before_capacity() {
    let mut cache = BindingSetCache::new();
    assert_eq!(cache.evict_if_full(), None);
    assert_eq!(cache.stats().evictions, 0);
}

#[test]
fn eviction_when_full_returns_lru_handle() {
    let mut cache = BindingSetCache::new();
    // Fill to capacity.
    for index in 0..BindingSetCache::CAPACITY {
        let entries = [make_entry(index as u32)];
        cache.insert(index as u32 + 1, index as u32 + 1, &entries);
    }
    // Entry 0 is the least recently used (never re-read).
    let evicted = cache.evict_if_full();
    assert_eq!(evicted, Some(1));
    assert_eq!(cache.stats().evictions, 1);
}
