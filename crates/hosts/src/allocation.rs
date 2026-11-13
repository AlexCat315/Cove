//! Allocation counters for `query_ui_memory`.
//!
//! The application opts in by installing [`cove_alloc::TrackingAllocator`] as
//! its `#[global_allocator]`; this module then adds the counters that allocator
//! has measured. A host whose binary did not install it reports nothing at all —
//! [`cove_alloc::snapshot`] returns `None`, so the counters are omitted instead
//! of published as zeros.
//!
//! The counters describe bytes and blocks of the process heap. They say nothing
//! about Rust object types, retaining paths or owners, and they are not a leak
//! detector: a block counts as live until it is released, reachable or not.

use std::collections::BTreeMap;

/// Adds the allocation counters for every size class this build measures.
///
/// No counter is added when no tracking allocator has served an allocation.
pub fn insert_counters(counters: &mut BTreeMap<String, u64>) {
    let Some(snapshot) = cove_alloc::snapshot() else {
        return;
    };
    counters.insert(
        cove_alloc::CURRENT_BYTES_COUNTER.to_owned(),
        snapshot.current_bytes,
    );
    counters.insert(
        cove_alloc::PEAK_BYTES_COUNTER.to_owned(),
        snapshot.peak_bytes,
    );
    counters.insert(
        cove_alloc::TOTAL_BYTES_COUNTER.to_owned(),
        snapshot.total_bytes,
    );
    counters.insert(
        cove_alloc::DEALLOCATED_BYTES_COUNTER.to_owned(),
        snapshot.deallocated_bytes,
    );
    counters.insert(
        cove_alloc::LIVE_BLOCKS_COUNTER.to_owned(),
        snapshot.live_blocks,
    );
    counters.insert(
        cove_alloc::PEAK_LIVE_BLOCKS_COUNTER.to_owned(),
        snapshot.peak_live_blocks,
    );
    counters.insert(
        cove_alloc::TOTAL_BLOCKS_COUNTER.to_owned(),
        snapshot.total_blocks,
    );
    counters.insert(
        cove_alloc::DEALLOCATED_BLOCKS_COUNTER.to_owned(),
        snapshot.deallocated_blocks,
    );
    for (counter, bytes) in snapshot.size_class_counters() {
        counters.insert(counter.to_owned(), bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The hosts test binary does not install the tracking allocator, so a
    /// `query_ui_memory` answer built here must carry no allocation counter.
    #[test]
    fn omits_the_counters_without_an_installed_allocator() {
        assert!(
            !cove_alloc::installed(),
            "this test binary must not install the tracking allocator"
        );
        let mut counters = BTreeMap::new();
        insert_counters(&mut counters);
        assert!(
            counters.is_empty(),
            "an uninstrumented host must omit the counters: {counters:?}"
        );
    }
}
