//! Translation of CoveUIText/TextCache.swift. Builds happen outside the cache lock.
use std::{collections::HashMap, hash::Hash, sync::Mutex};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCacheLimits { pub maximum_entries: usize, pub maximum_bytes: usize }
impl Default for TextCacheLimits { fn default() -> Self { Self { maximum_entries: 512, maximum_bytes: 8 * 1024 * 1024 } } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCacheSnapshot { pub entries: usize, pub estimated_bytes: usize }
struct Entry<V> { value: V, bytes: usize, used: u64 }
struct Storage<K, V> { entries: HashMap<K, Entry<V>>, bytes: usize, clock: u64, generation: u64 }
/// Bounded LRU cache. Public so callers and tests can assert eviction order,
/// oversize handling and trim ordering directly, which is where drift hides.
pub struct TextCache<K, V> { limits: TextCacheLimits, storage: Mutex<Storage<K, V>> }
impl<K: Clone + Eq + Hash, V: Clone> TextCache<K, V> {
    pub fn new(limits: TextCacheLimits) -> Self { Self { limits, storage: Mutex::new(Storage { entries: HashMap::new(), bytes: 0, clock: 0, generation: 0 }) } }
    pub fn snapshot(&self) -> TextCacheSnapshot { let s = self.storage.lock().unwrap_or_else(|e| e.into_inner()); TextCacheSnapshot { entries: s.entries.len(), estimated_bytes: s.bytes } }
    pub fn clear(&self) { let mut s = self.storage.lock().unwrap_or_else(|e| e.into_inner()); s.generation = s.generation.wrapping_add(1); s.entries.clear(); s.bytes = 0; }
    pub fn value(&self, key: K, cost: impl FnOnce(&V) -> usize, build: impl FnOnce() -> V) -> V {
        let generation = { let mut s = self.storage.lock().unwrap_or_else(|e| e.into_inner());
            s.clock = s.clock.wrapping_add(1); let clock = s.clock;
            if let Some(entry) = s.entries.get_mut(&key) { entry.used = clock; return entry.value.clone(); } s.generation };
        let result = build(); let bytes = cost(&result); let mut s = self.storage.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = s.entries.get(&key) { return cached.value.clone(); }
        if s.generation != generation || self.limits.maximum_entries == 0 || bytes > self.limits.maximum_bytes { return result; }
        while s.entries.len() >= self.limits.maximum_entries || s.bytes > self.limits.maximum_bytes - bytes {
            let Some(oldest) = s.entries.iter().min_by_key(|(_, entry)| entry.used).map(|(key, _)| key.clone()) else { break; };
            if let Some(entry) = s.entries.remove(&oldest) { s.bytes -= entry.bytes; }
        }
        s.clock = s.clock.wrapping_add(1); let clock = s.clock; s.entries.insert(key, Entry { value: result.clone(), bytes, used: clock }); s.bytes += bytes; result
    }
}
