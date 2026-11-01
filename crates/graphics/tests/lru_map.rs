//! Translation of Graphics/Tests/NativeRHITests/LRUMapTests.swift.

use cove_graphics::{CacheStats, LruMap};

#[test]
fn insert_and_read() {
    let mut lru = LruMap::<i32, String>::new();
    lru.insert(1, "a".into());
    assert_eq!(lru.get(&1).map(String::as_str), Some("a"));
    assert_eq!(lru.count(), 1);
}

#[test]
fn eviction_selects_least_recently_used() {
    let mut lru = LruMap::<i32, String>::new();
    lru.insert(1, "a".into());
    lru.insert(2, "b".into());
    lru.insert(3, "c".into());

    // Touch key 1 so key 2 becomes the least recently used.
    let _ = lru.get(&1);

    let evicted = lru.evict_least_recently_used();
    assert_eq!(evicted, Some((2, "b".into())));
    assert_eq!(lru.count(), 2);
    assert_eq!(lru.get(&2), None);
}

#[test]
fn eviction_order_follows_insertion_without_reads() {
    let mut lru = LruMap::<i32, String>::new();
    lru.insert(1, "a".into());
    lru.insert(2, "b".into());
    assert_eq!(
        lru.evict_least_recently_used().map(|entry| entry.0),
        Some(1)
    );
    assert_eq!(
        lru.evict_least_recently_used().map(|entry| entry.0),
        Some(2)
    );
    assert_eq!(lru.evict_least_recently_used(), None);
}

#[test]
fn update_existing_key_does_not_grow_count() {
    let mut lru = LruMap::<i32, String>::new();
    lru.insert(1, "a".into());
    lru.insert(1, "z".into());
    assert_eq!(lru.count(), 1);
    assert_eq!(lru.get(&1).map(String::as_str), Some("z"));
}

#[test]
fn remove_value() {
    let mut lru = LruMap::<i32, String>::new();
    lru.insert(1, "a".into());
    assert_eq!(lru.remove(&1), Some("a".into()));
    assert_eq!(lru.count(), 0);
    assert_eq!(lru.get(&1), None);
}

#[test]
fn remove_all_resets_ordering() {
    let mut lru = LruMap::<i32, String>::new();
    lru.insert(1, "a".into());
    lru.insert(2, "b".into());
    lru.remove_all();
    assert_eq!(lru.count(), 0);
    assert_eq!(lru.evict_least_recently_used(), None);
}

#[test]
fn hit_rate() {
    let stats = CacheStats {
        hits: 3,
        misses: 1,
        evictions: 0,
    };
    assert_eq!(stats.total_lookups(), 4);
    assert!((stats.hit_rate() - 0.75).abs() < 1e-9);
}

#[test]
fn hit_rate_with_no_lookups_is_zero() {
    assert_eq!(CacheStats::default().hit_rate(), 0.0);
}

#[test]
fn delta_since_earlier_snapshot() {
    let earlier = CacheStats {
        hits: 2,
        misses: 1,
        evictions: 0,
    };
    let later = CacheStats {
        hits: 5,
        misses: 4,
        evictions: 2,
    };
    let delta = later.delta_since(&earlier);
    assert_eq!(
        delta,
        CacheStats {
            hits: 3,
            misses: 3,
            evictions: 2
        }
    );
}
