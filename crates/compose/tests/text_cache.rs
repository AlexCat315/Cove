//! Translation of `Tests/CoveUIComposeTests/TextCacheTests.swift`.
//!
//! Behaviour checklist:
//! - reading an existing key refreshes its LRU position, so the *least recently
//!   read* entry is the one evicted when the entry bound is reached
//! - a result larger than the byte bound, or a cache with a zero entry bound,
//!   returns the value without retaining it
//! - a build that trims the cache does not have its older result retained
//!   afterwards
//! - a long run of dynamic text and width changes keeps the environment's own
//!   caches within their default bounds, and trimming leaves them usable

mod support;

use cove_text::{TextCache, TextCacheLimits, TextEnvironment};

use support::GlobalTestLock;

#[test]
fn evicts_least_recently_read_entry() {
    let cache: TextCache<String, i32> = TextCache::new(TextCacheLimits {
        maximum_entries: 2,
        maximum_bytes: 10,
    });

    let read = |key: &str, value: i32| -> i32 { cache.value(key.to_string(), |_| 4, || value) };

    assert_eq!(read("a", 1), 1, "the first read builds and stores");
    assert_eq!(read("b", 2), 2, "the second read fills the entry bound");
    assert_eq!(
        read("a", 9),
        1,
        "re-reading `a` returns the cached value, not the new build"
    );
    assert_eq!(
        read("c", 3),
        3,
        "`c` is stored, evicting the least recently read entry"
    );
    assert_eq!(read("a", 9), 1, "`a` survived the eviction");
    assert_eq!(
        read("b", 9),
        9,
        "`b` was the least recently read entry, so it rebuilds"
    );

    let snapshot = cache.snapshot();
    assert_eq!(snapshot.entries, 2, "the entry bound is respected");
    assert_eq!(
        snapshot.estimated_bytes, 8,
        "two entries at 4 bytes each account for exactly 8 bytes"
    );
}

#[test]
fn oversized_results_and_disabled_caches_are_not_retained() {
    // A zero entry bound disables retention entirely.
    let disabled = TextCache::<i32, i32>::new(TextCacheLimits {
        maximum_entries: 0,
        maximum_bytes: 8 * 1024 * 1024,
    });
    assert_eq!(
        disabled.value(1, |_| 4, || 42),
        42,
        "the value is still returned"
    );
    assert_eq!(
        disabled.snapshot().entries,
        0,
        "a zero entry bound retains nothing"
    );

    // A result costing more than the byte bound is returned but not stored.
    let tiny = TextCache::<i32, i32>::new(TextCacheLimits {
        maximum_entries: 512,
        maximum_bytes: 3,
    });
    assert_eq!(
        tiny.value(1, |_| 4, || 42),
        42,
        "the value is still returned"
    );
    assert_eq!(
        tiny.snapshot().entries,
        0,
        "a result over the byte bound is never retained"
    );
}

#[test]
fn trim_does_not_retain_an_older_build() {
    let cache: TextCache<i32, i32> = TextCache::new(TextCacheLimits::default());
    let value = cache.value(
        1,
        |_| 4,
        || {
            // Trimming while the build runs invalidates this generation.
            cache.clear();
            42
        },
    );
    assert_eq!(value, 42, "the built value is still returned");
    assert_eq!(
        cache.snapshot().entries,
        0,
        "a build that saw a trim is not inserted afterwards"
    );
}

#[test]
fn dynamic_text_and_width_changes_remain_bounded() {
    GlobalTestLock::locked(|| {
        let environment = support::test_text_environment(16.0, 20.0);
        for index in 0..2_000 {
            let text = format!("Traffic {index}");
            let max_width = 100.0 + (index % 400) as f32;
            let _ = environment.cached_layout(
                &text,
                None,
                None,
                max_width,
                cove_core::TextAlignment::Leading,
                0.0,
            );
        }

        let shaping = environment.shaping_cache_snapshot();
        let layout = environment.layout_cache_snapshot();
        assert!(
            shaping.entries <= 512,
            "the shaping cache stays within its default entry bound ({} entries)",
            shaping.entries
        );
        assert!(
            layout.entries <= 512,
            "the layout cache stays within its default entry bound ({} entries)",
            layout.entries
        );
        assert!(
            layout.estimated_bytes <= 8 * 1024 * 1024,
            "the layout cache stays within its default byte bound ({} bytes)",
            layout.estimated_bytes
        );

        environment.trim_text_caches();
        assert_eq!(
            environment.shaping_cache_snapshot().entries,
            0,
            "trimming empties the shaping cache"
        );
        assert_eq!(
            environment.layout_cache_snapshot().estimated_bytes,
            0,
            "trimming empties the layout cache"
        );

        // The environment still measures after a trim.
        let restored = environment.cached_layout(
            "Restored",
            None,
            None,
            f32::INFINITY,
            cove_core::TextAlignment::Leading,
            0.0,
        );
        assert!(
            !restored.lines.is_empty(),
            "a trimmed environment still lays out text"
        );
    });
}
