//! Translation of NativeRHI/Validation/Caches.swift: value-keyed caches and an
//! O(1) LRU.
//!
//! The Zig prototype keyed its pipeline-layout and binding-set caches by a bare
//! Wyhash value with no collision handling, and its LRU eviction was O(n). This
//! port keys caches by the *full* value (the hash map resolves collisions
//! through equality) and keeps the LRU as an intrusive list over a slot arena,
//! giving O(1) touch/insert/evict without `unsafe`.
use std::collections::HashMap;
use std::hash::Hash;

use super::{BindingLayoutEntry, BindingSetEntry, PipelineInterfaceKey};

// MARK: - O(1) LRU map

struct LruSlot<K, V> {
    key: K,
    value: V,
    prev: Option<usize>,
    next: Option<usize>,
}

/// A least-recently-used map with O(1) `get`, `insert`, `remove` and eviction.
///
/// Slots live in a `Vec` arena and are recycled through a free list, so a node
/// is never heap-allocated individually and no reference counting is needed.
pub struct LruMap<K, V> {
    index: HashMap<K, usize>,
    slots: Vec<Option<LruSlot<K, V>>>,
    free: Vec<usize>,
    /// Most recently used.
    head: Option<usize>,
    /// Least recently used.
    tail: Option<usize>,
    count: usize,
}
impl<K: Eq + Hash + Clone, V> LruMap<K, V> {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            slots: Vec::new(),
            free: Vec::new(),
            head: None,
            tail: None,
            count: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let slot = *self.index.get(key)?;
        self.move_to_head(slot);
        self.slots[slot].as_ref().map(|slot| &slot.value)
    }

    pub fn insert(&mut self, key: K, value: V) {
        if let Some(&slot) = self.index.get(&key) {
            if let Some(entry) = self.slots[slot].as_mut() {
                entry.value = value;
            }
            self.move_to_head(slot);
            return;
        }
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                self.slots.push(None);
                self.slots.len() - 1
            }
        };
        self.slots[slot] = Some(LruSlot {
            key: key.clone(),
            value,
            prev: None,
            next: None,
        });
        self.index.insert(key, slot);
        self.add_to_head(slot);
        self.count += 1;
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let slot = self.index.remove(key)?;
        self.detach(slot);
        let entry = self.slots[slot].take()?;
        self.free.push(slot);
        self.count -= 1;
        Some(entry.value)
    }

    pub fn evict_least_recently_used(&mut self) -> Option<(K, V)> {
        let slot = self.tail?;
        self.detach(slot);
        let entry = self.slots[slot].take()?;
        self.index.remove(&entry.key);
        self.free.push(slot);
        self.count -= 1;
        Some((entry.key, entry.value))
    }

    pub fn remove_all(&mut self) {
        self.index.clear();
        self.slots.clear();
        self.free.clear();
        self.head = None;
        self.tail = None;
        self.count = 0;
    }

    fn add_to_head(&mut self, slot: usize) {
        if let Some(entry) = self.slots[slot].as_mut() {
            entry.prev = None;
            entry.next = self.head;
        }
        if let Some(head) = self.head {
            if let Some(entry) = self.slots[head].as_mut() {
                entry.prev = Some(slot);
            }
        }
        self.head = Some(slot);
        if self.tail.is_none() {
            self.tail = Some(slot);
        }
    }

    fn detach(&mut self, slot: usize) {
        let Some(entry) = self.slots[slot].as_ref() else {
            return;
        };
        let (prev, next) = (entry.prev, entry.next);
        match prev {
            Some(prev) => {
                if let Some(entry) = self.slots[prev].as_mut() {
                    entry.next = next;
                }
            }
            None => self.head = next,
        }
        match next {
            Some(next) => {
                if let Some(entry) = self.slots[next].as_mut() {
                    entry.prev = prev;
                }
            }
            None => self.tail = prev,
        }
        if let Some(entry) = self.slots[slot].as_mut() {
            entry.prev = None;
            entry.next = None;
        }
    }

    fn move_to_head(&mut self, slot: usize) {
        self.detach(slot);
        self.add_to_head(slot);
    }
}
impl<K: Eq + Hash + Clone, V> Default for LruMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

// MARK: - Cache statistics

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}
impl CacheStats {
    pub fn total_lookups(&self) -> u64 {
        self.hits.saturating_add(self.misses)
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.total_lookups();
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    /// Counts since `previous`, saturating at zero for each field.
    pub fn delta_since(&self, previous: &Self) -> Self {
        Self {
            hits: self.hits.saturating_sub(previous.hits),
            misses: self.misses.saturating_sub(previous.misses),
            evictions: self.evictions.saturating_sub(previous.evictions),
        }
    }
}

// MARK: - Pipeline layout cache

/// Maps the ordered set of binding-layout IDs to a pipeline-layout handle.
/// Keyed by the full ID list, so distinct layouts never collide.
#[derive(Clone, Debug, Default)]
pub struct PipelineLayoutCache {
    by_interface: HashMap<PipelineInterfaceKey, u32>,
}
impl PipelineLayoutCache {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn layout(&self, key: &PipelineInterfaceKey) -> Option<u32> {
        self.by_interface.get(key).copied()
    }
    pub fn insert(&mut self, handle_id: u32, key: PipelineInterfaceKey) {
        self.by_interface.insert(key, handle_id);
    }
    pub fn len(&self) -> usize {
        self.by_interface.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_interface.is_empty()
    }
}

// MARK: - Binding set cache

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingSetCacheKey {
    pub layout_id: u32,
    pub entries: Vec<BindingSetEntry>,
}

/// Bounded LRU cache of resolved binding sets. Lookups record hit/miss stats;
/// when full, the least recently used set is evicted and its handle returned for
/// teardown.
pub struct BindingSetCache {
    lru: LruMap<BindingSetCacheKey, u32>,
    stats: CacheStats,
}
impl BindingSetCache {
    pub const CAPACITY: usize = 1024;

    pub fn new() -> Self {
        Self {
            lru: LruMap::new(),
            stats: CacheStats::default(),
        }
    }

    pub fn count(&self) -> usize {
        self.lru.count()
    }

    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    pub fn existing(&mut self, layout_id: u32, entries: &[BindingSetEntry]) -> Option<u32> {
        let key = BindingSetCacheKey {
            layout_id,
            entries: entries.to_vec(),
        };
        if let Some(id) = self.lru.get(&key) {
            let id = *id;
            self.stats.hits += 1;
            return Some(id);
        }
        self.stats.misses += 1;
        None
    }

    pub fn insert(&mut self, handle_id: u32, layout_id: u32, entries: &[BindingSetEntry]) {
        self.lru.insert(
            BindingSetCacheKey {
                layout_id,
                entries: entries.to_vec(),
            },
            handle_id,
        );
    }

    pub fn evict_if_full(&mut self) -> Option<u32> {
        if self.lru.count() < Self::CAPACITY {
            return None;
        }
        let evicted = self.lru.evict_least_recently_used();
        self.stats.evictions += 1;
        evicted.map(|(_, handle)| handle)
    }

    /// Removes a known set entry. Used when the caller explicitly destroys a
    /// binding set; it already holds the layout ID and entries.
    pub fn remove(&mut self, layout_id: u32, entries: &[BindingSetEntry]) -> Option<u32> {
        self.lru.remove(&BindingSetCacheKey {
            layout_id,
            entries: entries.to_vec(),
        })
    }

    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }
}
impl Default for BindingSetCache {
    fn default() -> Self {
        Self::new()
    }
}

// MARK: - Grouped device caches

/// All frontend-owned binding/pipeline bookkeeping, grouped so the public
/// `Device` stays small (single responsibility per group).
#[derive(Default)]
pub struct DeviceCaches {
    /// Binding-layout handle ID → its layout entries.
    binding_layouts: HashMap<u32, Vec<BindingLayoutEntry>>,
    /// Binding-set handle ID → the layout it was built against.
    binding_set_owners: HashMap<u32, u32>,
    /// Pipeline-layout handle ID → the ordered set of binding-layout IDs.
    pipeline_layout_definitions: HashMap<u32, Vec<u32>>,
    pub pipeline_layouts: PipelineLayoutCache,
    pub binding_sets: BindingSetCache,
}
impl DeviceCaches {
    pub fn new() -> Self {
        Self::default()
    }

    // Binding layout

    pub fn store_binding_layout(&mut self, id: u32, entries: Vec<BindingLayoutEntry>) {
        self.binding_layouts.insert(id, entries);
    }

    pub fn binding_layout_entries(&self, id: u32) -> Option<&[BindingLayoutEntry]> {
        self.binding_layouts.get(&id).map(Vec::as_slice)
    }

    pub fn remove_binding_layout(&mut self, id: u32) -> Option<Vec<BindingLayoutEntry>> {
        self.binding_layouts.remove(&id)
    }

    // Binding set

    pub fn store_binding_set(&mut self, id: u32, layout_id: u32) {
        self.binding_set_owners.insert(id, layout_id);
    }

    pub fn binding_set_layout_id(&self, id: u32) -> Option<u32> {
        self.binding_set_owners.get(&id).copied()
    }

    pub fn remove_binding_set_owner(&mut self, id: u32) -> Option<u32> {
        self.binding_set_owners.remove(&id)
    }

    pub fn binding_set_count(&self) -> usize {
        self.binding_set_owners.len()
    }

    // Pipeline layout

    pub fn store_pipeline_layout_definition(&mut self, id: u32, set_layouts: Vec<u32>) {
        self.pipeline_layout_definitions.insert(id, set_layouts);
    }

    pub fn pipeline_layout_definition(&self, id: u32) -> Option<&[u32]> {
        self.pipeline_layout_definitions.get(&id).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{BindingResource, Buffer};

    fn entry(slot: u32, buffer: u32) -> BindingSetEntry {
        BindingSetEntry::new(slot, BindingResource::uniform_buffer(Buffer::new(buffer)))
    }

    #[test]
    fn the_lru_evicts_in_access_order() {
        let mut lru = LruMap::new();
        lru.insert(1, "one");
        lru.insert(2, "two");
        lru.insert(3, "three");
        assert_eq!(lru.count(), 3);
        // Touch 1 so 2 becomes the least recently used.
        assert_eq!(lru.get(&1), Some(&"one"));
        assert_eq!(lru.evict_least_recently_used(), Some((2, "two")));
        assert_eq!(lru.count(), 2);
        assert_eq!(lru.evict_least_recently_used(), Some((3, "three")));
        assert_eq!(lru.evict_least_recently_used(), Some((1, "one")));
        assert_eq!(lru.evict_least_recently_used(), None);
    }

    #[test]
    fn replacing_a_key_keeps_the_count_stable() {
        let mut lru = LruMap::new();
        lru.insert(1, 10);
        lru.insert(1, 20);
        assert_eq!(lru.count(), 1);
        assert_eq!(lru.get(&1), Some(&20));
        assert_eq!(lru.remove(&1), Some(20));
        assert_eq!(lru.remove(&1), None);
    }

    #[test]
    fn binding_set_cache_records_hits_and_misses() {
        let mut cache = BindingSetCache::new();
        let entries = [entry(0, 1)];
        assert_eq!(cache.existing(7, &entries), None);
        cache.insert(100, 7, &entries);
        assert_eq!(cache.existing(7, &entries), Some(100));
        let stats = cache.stats();
        assert_eq!((stats.hits, stats.misses), (1, 1));
        assert_eq!(stats.hit_rate(), 0.5);
        assert_eq!(cache.remove(7, &entries), Some(100));
        assert_eq!(cache.count(), 0);
    }

    #[test]
    fn cache_stats_delta_saturates() {
        let previous = CacheStats {
            hits: 10,
            misses: 5,
            evictions: 1,
        };
        let current = CacheStats {
            hits: 12,
            misses: 4,
            evictions: 3,
        };
        let delta = current.delta_since(&previous);
        assert_eq!(
            delta,
            CacheStats {
                hits: 2,
                misses: 0,
                evictions: 2
            }
        );
        assert_eq!(CacheStats::default().hit_rate(), 0.0);
    }

    #[test]
    fn device_caches_track_layout_ownership() {
        use crate::rhi::{BindingLayoutEntry, BindingType, ShaderVisibility};

        let mut caches = DeviceCaches::new();
        caches.store_binding_layout(
            1,
            vec![BindingLayoutEntry::new(
                0,
                BindingType::UniformBuffer,
                ShaderVisibility::VERTEX,
            )],
        );
        assert_eq!(
            caches
                .binding_layout_entries(1)
                .map(|entries| entries.len()),
            Some(1)
        );
        caches.store_binding_set(50, 1);
        assert_eq!(caches.binding_set_layout_id(50), Some(1));
        assert_eq!(caches.remove_binding_set_owner(50), Some(1));
        assert_eq!(caches.binding_set_layout_id(50), None);
        caches.store_pipeline_layout_definition(9, vec![1]);
        assert_eq!(caches.pipeline_layout_definition(9), Some(&[1u32][..]));
        assert_eq!(caches.binding_set_count(), 0);
    }
}
