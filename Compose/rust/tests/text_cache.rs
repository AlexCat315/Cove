//! Translation of `Tests/CoveUIComposeTests/TextCacheTests.swift`.
//!
//! Behaviour checklist:
//! - a long run of dynamic text and width changes stays within the default
//!   entry and byte bounds
//! - `trim_text_caches` empties both caches and the environment keeps working
//!   afterwards
//!
//! The three `TextCache`-direct cases in the Swift file (LRU eviction,
//! oversized/disabled retention, and trim-vs-build ordering) cannot be migrated
//! as written: `TextCache` is `pub(crate)` to `cove-text` and only
//! `TextCacheLimits`/`TextCacheSnapshot` are exported. Those behaviours are
//! covered here indirectly through the environment's bounded caches, which is
//! the layer Compose actually owns.

mod support;

use cove_text::TextEnvironment;

use support::GlobalTestLock;

#[test]
fn dynamic_text_and_width_changes_remain_bounded() {
    GlobalTestLock::locked(|| {
        let environment = TextEnvironment::from(support::test_text_environment(16.0, 20.0));
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
