//! Runs the tracking allocator as the process allocator and checks the counters
//! move with real allocations.
//!
//! `harness = false` keeps the test harness from allocating on other threads,
//! so every byte counted here belongs to this binary's own operations and the
//! totals stay additive. The binary exits non-zero on a failed assertion.

use cove_alloc::{AllocationSnapshot, TrackingAllocator};
use std::alloc::{Layout, alloc, alloc_zeroed, dealloc, realloc};

#[global_allocator]
static ALLOC: TrackingAllocator = TrackingAllocator::new();

fn main() {
    let startup = cove_alloc::snapshot()
        .expect("the process allocator is installed, so startup allocations are counted");
    assert!(cove_alloc::installed());
    assert!(
        startup.total_blocks > 0,
        "the process allocated for startup, so blocks must be counted"
    );
    check_invariants(&startup, "startup");

    allocation_raises_counters(&startup);
    release_lowers_current_bytes();
    reallocation_moves_bytes_between_classes();
    zeroed_allocation_is_counted();

    let end = cove_alloc::snapshot().expect("counters stay installed");
    println!(
        "alloc counters: current={} peak={} total={} deallocated={} live_blocks={} \
         total_blocks={} deallocated_blocks={}",
        end.current_bytes,
        end.peak_bytes,
        end.total_bytes,
        end.deallocated_bytes,
        end.live_blocks,
        end.total_blocks,
        end.deallocated_blocks
    );
}

fn allocation_raises_counters(startup: &AllocationSnapshot) {
    const SIZE: usize = 1 << 20;
    let layout = Layout::from_size_align(SIZE, 8).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { alloc(layout) };
    assert!(!pointer.is_null(), "the system allocator must serve 1 MiB");

    let during = cove_alloc::snapshot().expect("counters stay installed");
    assert!(
        during.current_bytes >= startup.current_bytes + SIZE as u64,
        "current bytes must include the new block: {} then {}",
        startup.current_bytes,
        during.current_bytes
    );
    assert!(
        during.total_bytes >= startup.total_bytes + SIZE as u64,
        "total bytes must include the new block"
    );
    assert!(
        during.live_blocks > startup.live_blocks,
        "live blocks must include the new block"
    );
    assert!(
        during.peak_bytes >= during.current_bytes,
        "peak must cover the current high-water mark"
    );
    check_invariants(&during, "after allocating 1 MiB");

    // SAFETY: `pointer` came from `alloc` with `layout` and is freed once.
    unsafe { dealloc(pointer, layout) };
}

fn release_lowers_current_bytes() {
    let layout = Layout::from_size_align(4096, 16).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { alloc(layout) };
    assert!(!pointer.is_null());
    let held = cove_alloc::snapshot().expect("counters stay installed");

    // SAFETY: `pointer` came from `alloc` with `layout` and is freed once.
    unsafe { dealloc(pointer, layout) };
    let released = cove_alloc::snapshot().expect("counters stay installed");

    assert_eq!(
        released.current_bytes,
        held.current_bytes - 4096,
        "a release must subtract exactly the block size"
    );
    assert_eq!(
        released.live_blocks,
        held.live_blocks - 1,
        "a release must subtract exactly one block"
    );
    assert_eq!(
        released.deallocated_bytes,
        held.deallocated_bytes + 4096,
        "a release must add the block size to the released total"
    );
    assert_eq!(
        released.deallocated_blocks,
        held.deallocated_blocks + 1,
        "a release must add one to the released block total"
    );
    assert_eq!(
        released.peak_bytes, held.peak_bytes,
        "a release never lowers the peak"
    );
    check_invariants(&released, "after releasing 4 KiB");
}

fn reallocation_moves_bytes_between_classes() {
    let baseline = cove_alloc::snapshot().expect("counters stay installed");
    let small = Layout::from_size_align(1024, 16).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { alloc(small) };
    assert!(!pointer.is_null());
    let before = cove_alloc::snapshot().expect("counters stay installed");
    assert_eq!(
        before.size_classes[2].live_blocks,
        baseline.size_classes[2].live_blocks + 1,
        "1024 bytes are the upper bound of the 1 KiB class"
    );
    assert_eq!(
        before.size_classes[3].live_blocks, baseline.size_classes[3].live_blocks,
        "the 16 KiB class is untouched"
    );

    // SAFETY: `pointer` came from `alloc` with `small`, and the new size keeps
    // the same alignment.
    let grown = unsafe { realloc(pointer, small, 8192) };
    assert!(!grown.is_null());
    let after_grow = cove_alloc::snapshot().expect("counters stay installed");
    assert_eq!(
        after_grow.current_bytes,
        before.current_bytes + 8192 - 1024,
        "growing keeps only the difference live"
    );
    assert_eq!(
        after_grow.live_blocks, before.live_blocks,
        "a reallocation replaces one block with one block"
    );
    assert_eq!(
        after_grow.total_blocks,
        before.total_blocks + 1,
        "the new block is served once"
    );
    assert_eq!(
        after_grow.deallocated_blocks,
        before.deallocated_blocks + 1,
        "the old block is released once"
    );
    assert_eq!(
        after_grow.total_bytes,
        before.total_bytes + 8192,
        "the new size is the allocated size"
    );
    assert_eq!(
        after_grow.deallocated_bytes,
        before.deallocated_bytes + 1024,
        "the old size is the released size"
    );
    assert_eq!(
        after_grow.size_classes[2].live_blocks, baseline.size_classes[2].live_blocks,
        "the old block left its size class"
    );
    assert_eq!(
        after_grow.size_classes[4].live_blocks,
        baseline.size_classes[4].live_blocks + 1,
        "8192 bytes live in the 16 KiB class"
    );
    check_invariants(&after_grow, "after growing 1 KiB to 8 KiB");

    let large = Layout::from_size_align(8192, 16).expect("valid layout");
    // SAFETY: `grown` came from `realloc` with `small` and the new size 8192.
    let shrunk = unsafe { realloc(grown, large, 512) };
    assert!(!shrunk.is_null());
    let after_shrink = cove_alloc::snapshot().expect("counters stay installed");
    assert_eq!(
        after_shrink.current_bytes,
        after_grow.current_bytes - (8192 - 512),
        "shrinking keeps only the difference live"
    );
    assert_eq!(after_shrink.live_blocks, after_grow.live_blocks);
    assert_eq!(
        after_shrink.peak_bytes, after_grow.peak_bytes,
        "shrinking never raises the peak"
    );
    check_invariants(&after_shrink, "after shrinking 8 KiB to 512 B");

    let small_again = Layout::from_size_align(512, 16).expect("valid layout");
    // SAFETY: `shrunk` came from `realloc` with the 512-byte layout.
    unsafe { dealloc(shrunk, small_again) };
}

fn zeroed_allocation_is_counted() {
    let baseline = cove_alloc::snapshot().expect("counters stay installed");
    let layout = Layout::from_size_align(65_536, 32).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { alloc_zeroed(layout) };
    assert!(!pointer.is_null());
    let during = cove_alloc::snapshot().expect("counters stay installed");
    assert_eq!(
        during.size_classes[5].live_bytes,
        baseline.size_classes[5].live_bytes + 65_536,
        "64 KiB of zeroed memory lives in the 64 KiB class"
    );
    assert_eq!(
        during.current_bytes,
        baseline.current_bytes + 65_536,
        "zeroed memory is counted like any other block"
    );
    // SAFETY: `pointer` came from `alloc_zeroed` with `layout`.
    unsafe { dealloc(pointer, layout) };
    let after = cove_alloc::snapshot().expect("counters stay installed");
    assert_eq!(
        after.size_classes[5].live_bytes,
        baseline.size_classes[5].live_bytes
    );
    check_invariants(&after, "after releasing the zeroed block");
}

fn check_invariants(snapshot: &AllocationSnapshot, moment: &str) {
    assert!(
        snapshot.peak_bytes >= snapshot.current_bytes,
        "{moment}: peak {} is below current {}",
        snapshot.peak_bytes,
        snapshot.current_bytes
    );
    assert!(
        snapshot.total_bytes >= snapshot.current_bytes,
        "{moment}: total {} is below current {}",
        snapshot.total_bytes,
        snapshot.current_bytes
    );
    assert!(
        snapshot.peak_bytes <= snapshot.total_bytes,
        "{moment}: peak {} exceeds total {}",
        snapshot.peak_bytes,
        snapshot.total_bytes
    );
    assert_eq!(
        snapshot.live_blocks + snapshot.deallocated_blocks,
        snapshot.total_blocks,
        "{moment}: every block is either live or released"
    );
    assert!(
        snapshot.peak_live_blocks >= snapshot.live_blocks,
        "{moment}: peak live blocks is below live blocks"
    );
    assert!(
        snapshot.peak_live_blocks <= snapshot.total_blocks,
        "{moment}: peak live blocks exceeds the blocks ever served"
    );

    let class_bytes: u64 = snapshot
        .size_classes
        .iter()
        .map(|class| class.live_bytes)
        .sum();
    assert_eq!(
        class_bytes, snapshot.current_bytes,
        "{moment}: the size classes must account for every current byte"
    );
    let class_blocks: u64 = snapshot
        .size_classes
        .iter()
        .map(|class| class.live_blocks)
        .sum();
    assert_eq!(
        class_blocks, snapshot.live_blocks,
        "{moment}: the size classes must account for every live block"
    );
    assert_eq!(
        snapshot.size_class_counters().count(),
        snapshot.size_classes.len()
    );
    assert!(
        snapshot.size_classes[7].upper_bound_bytes.is_none(),
        "the last class is the overflow class"
    );
    for (index, class) in snapshot.size_classes.iter().enumerate().take(7) {
        assert_eq!(
            class.upper_bound_bytes,
            Some(cove_alloc::SIZE_CLASS_UPPER_BOUNDS[index])
        );
    }
}
