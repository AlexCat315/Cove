//! Allocation counters for `query_ui_memory`.
//!
//! [`TrackingAllocator`] wraps another [`GlobalAlloc`] and counts every block
//! that passes through it. The counters are process-wide atomics, so installing
//! the wrapper as the process allocator instruments the whole program:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: cove_alloc::TrackingAllocator = cove_alloc::TrackingAllocator::new();
//! ```
//!
//! What is measured is bytes and blocks: current, peak, total, live and
//! released, plus a bounded histogram by allocation size. The sizes are the
//! `Layout` sizes the caller passes to the allocator, so no per-allocation
//! header is added. The counter layer itself captures no call stack: a tracked
//! allocation costs a few atomic operations, plus one relaxed load for the
//! opt-in site sampler described below.
//!
//! What is **not** measured, and must not be inferred from this crate: Rust
//! object types, retaining paths or owner graphs, garbage collection, and leak
//! detection. A block that is never released stays in `live_blocks` whether or
//! not it is still reachable, so growth is not by itself a leak.
//!
//! A program that never installs the allocator reports nothing: [`installed`]
//! stays `false` until the allocator has served its first allocation and
//! [`snapshot`] returns `None` until then. Hosts omit the counters rather than
//! publishing fake zeros.
//!
//! # Allocation sites
//!
//! [`start_sampling`] is an opt-in, separate layer on top of the counters: every
//! `rate`-th successful fresh allocation captures a bounded raw stack into a
//! fixed lock-free ring, and [`stop_sampling`] aggregates identical stacks into
//! [`AllocationSite`]s with bytes and blocks. [`allocation_sites`] reads the
//! sites collected so far without ending the session, and
//! [`reset_allocation_sites`] discards them while sampling continues.
//!
//! The sampling hook never allocates and holds no lock; a thread-local recursion
//! guard keeps an unwinder that allocates internally from recursively sampling.
//! Reallocations are counted as one release plus one allocation by
//! [`snapshot`], but they are not sampled: sites describe fresh `alloc` and
//! `alloc_zeroed` blocks. Capture exists on Apple platforms and GNU Linux through
//! `libc::backtrace`; elsewhere [`sampling_support`] reports
//! [`SamplingSupport::Unsupported`] and [`start_sampling`] fails with
//! [`SamplingError::Unsupported`] instead of inventing sites.

mod sampling;

pub use sampling::{
    AllocationSite, AllocationSiteReport, MAX_SITE_FRAMES, SITE_RING_CAPACITY, SamplingError,
    SamplingSupport, allocation_sites, reset_allocation_sites, sampling_enabled, sampling_support,
    start_sampling, stop_sampling,
};

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Counter carrying the bytes currently allocated through the tracker.
pub const CURRENT_BYTES_COUNTER: &str = "allocCurrentBytes";
/// Counter carrying the highest `current_bytes` seen.
pub const PEAK_BYTES_COUNTER: &str = "allocPeakBytes";
/// Counter carrying every byte ever allocated through the tracker.
pub const TOTAL_BYTES_COUNTER: &str = "allocTotalBytes";
/// Counter carrying every byte ever released through the tracker.
pub const DEALLOCATED_BYTES_COUNTER: &str = "allocDeallocatedBytes";
/// Counter carrying the blocks currently allocated through the tracker.
pub const LIVE_BLOCKS_COUNTER: &str = "allocLiveBlocks";
/// Counter carrying the highest `live_blocks` seen.
pub const PEAK_LIVE_BLOCKS_COUNTER: &str = "allocPeakLiveBlocks";
/// Counter carrying every block ever allocated through the tracker.
pub const TOTAL_BLOCKS_COUNTER: &str = "allocTotalBlocks";
/// Counter carrying every block ever released through the tracker.
pub const DEALLOCATED_BLOCKS_COUNTER: &str = "allocDeallocatedBlocks";

/// Largest allocation size in each size class, except the overflow class.
pub const SIZE_CLASS_UPPER_BOUNDS: [u64; 7] = [64, 256, 1024, 4096, 16_384, 65_536, 1_048_576];

/// Number of size classes, including the overflow class.
pub const SIZE_CLASS_COUNT: usize = SIZE_CLASS_UPPER_BOUNDS.len() + 1;

/// Counter names carrying live bytes per size class.
pub const SIZE_CLASS_COUNTERS: [&str; SIZE_CLASS_COUNT] = [
    "allocSizeClass64LiveBytes",
    "allocSizeClass256LiveBytes",
    "allocSizeClass1KiBLiveBytes",
    "allocSizeClass4KiBLiveBytes",
    "allocSizeClass16KiBLiveBytes",
    "allocSizeClass64KiBLiveBytes",
    "allocSizeClass1MiBLiveBytes",
    "allocSizeClassOver1MiBLiveBytes",
];

/// One size class of the bounded histogram.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SizeClass {
    /// Largest allocation size in this class; `None` for the overflow class.
    pub upper_bound_bytes: Option<u64>,
    /// Bytes currently allocated in this class.
    pub live_bytes: u64,
    /// Blocks currently allocated in this class.
    pub live_blocks: u64,
}

/// A consistent-enough reading of the process allocation counters.
///
/// The fields are read one at a time, so a snapshot taken while other threads
/// allocate can mix state from slightly different instants. Every field is a
/// real reading; none is a placeholder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllocationSnapshot {
    /// Bytes currently allocated through the tracker.
    pub current_bytes: u64,
    /// Highest [`current_bytes`](Self::current_bytes) seen.
    pub peak_bytes: u64,
    /// Every byte ever allocated through the tracker.
    pub total_bytes: u64,
    /// Every byte ever released through the tracker.
    pub deallocated_bytes: u64,
    /// Blocks currently allocated through the tracker.
    pub live_blocks: u64,
    /// Highest [`live_blocks`](Self::live_blocks) seen.
    pub peak_live_blocks: u64,
    /// Every block ever allocated through the tracker.
    pub total_blocks: u64,
    /// Every block ever released through the tracker.
    pub deallocated_blocks: u64,
    /// Live bytes and blocks by allocation size.
    pub size_classes: [SizeClass; SIZE_CLASS_COUNT],
}

impl AllocationSnapshot {
    /// `(counter, live bytes)` for every size class.
    pub fn size_class_counters(&self) -> impl Iterator<Item = (&'static str, u64)> {
        SIZE_CLASS_COUNTERS
            .iter()
            .zip(self.size_classes.iter())
            .map(|(counter, class)| (*counter, class.live_bytes))
    }
}

/// A [`GlobalAlloc`] wrapper that records bytes and blocks per allocation.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrackingAllocator<A = System> {
    inner: A,
}

impl TrackingAllocator<System> {
    /// Wraps the system allocator, ready for `#[global_allocator]`.
    pub const fn new() -> Self {
        Self { inner: System }
    }
}

impl<A> TrackingAllocator<A> {
    /// Wraps `inner`, which does the actual allocation.
    pub const fn with_allocator(inner: A) -> Self {
        Self { inner }
    }

    /// The wrapped allocator.
    pub fn inner(&self) -> &A {
        &self.inner
    }
}

// SAFETY: every method forwards to `inner` unchanged. A block is only counted
// after `inner` returned it, and released before `inner` frees it, so the
// tracker never hands out or reports memory the wrapped allocator did not
// provide. Accounting uses the caller's `Layout` size, which is the same size
// passed back on deallocation.
unsafe impl<A: GlobalAlloc> GlobalAlloc for TrackingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract for `inner`.
        let pointer = unsafe { self.inner.alloc(layout) };
        if !pointer.is_null() {
            record_alloc(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller upholds `GlobalAlloc::alloc_zeroed`'s contract.
        let pointer = unsafe { self.inner.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_alloc(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_dealloc(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { self.inner.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size == 0 {
            // A zero-sized reallocation is a release; the wrapped allocator is
            // never asked for a zero-sized block.
            // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract, and
            // a zero `new_size` means the block is released.
            unsafe { self.dealloc(pointer, layout) };
            return std::ptr::null_mut();
        }
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract for
        // `inner`; `new_size` is non-zero.
        let new_pointer = unsafe { self.inner.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            record_realloc(layout.size(), new_size);
        }
        new_pointer
    }
}

/// Whether the allocator has served at least one allocation in this process.
///
/// Stays `false` in a program that never allocates through a
/// [`TrackingAllocator`], so a host can tell "not instrumented" from "zero
/// bytes live".
pub fn installed() -> bool {
    COUNTERS.installed.load(Ordering::Acquire)
}

/// The current counters, or `None` when no tracking allocator is installed.
///
/// The reading is not transactional: each field is a separate atomic load, so
/// concurrent activity can mix instants. A target that never installed the
/// allocator must omit the counters instead of reporting zeros.
pub fn snapshot() -> Option<AllocationSnapshot> {
    if !installed() {
        return None;
    }
    Some(AllocationSnapshot {
        current_bytes: COUNTERS.current_bytes.load(Ordering::Relaxed),
        peak_bytes: COUNTERS.peak_bytes.load(Ordering::Relaxed),
        total_bytes: COUNTERS.total_bytes.load(Ordering::Relaxed),
        deallocated_bytes: COUNTERS.deallocated_bytes.load(Ordering::Relaxed),
        live_blocks: COUNTERS.live_blocks.load(Ordering::Relaxed),
        peak_live_blocks: COUNTERS.peak_live_blocks.load(Ordering::Relaxed),
        total_blocks: COUNTERS.total_blocks.load(Ordering::Relaxed),
        deallocated_blocks: COUNTERS.deallocated_blocks.load(Ordering::Relaxed),
        size_classes: std::array::from_fn(|index| SizeClass {
            upper_bound_bytes: SIZE_CLASS_UPPER_BOUNDS.get(index).copied(),
            live_bytes: COUNTERS.size_class_bytes[index].load(Ordering::Relaxed),
            live_blocks: COUNTERS.size_class_blocks[index].load(Ordering::Relaxed),
        }),
    })
}

/// The size class `size` belongs to.
fn size_class_index(size: u64) -> usize {
    SIZE_CLASS_UPPER_BOUNDS
        .iter()
        .position(|bound| size <= *bound)
        .unwrap_or(SIZE_CLASS_COUNT - 1)
}

fn saturating_add(counter: &AtomicU64, value: u64) {
    let _ = counter.try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

fn saturating_sub(counter: &AtomicU64, value: u64) {
    let _ = counter.try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_sub(value))
    });
}

/// Records a block `size` bytes large handed out by the allocator.
fn record_alloc(size: usize) {
    let size = size as u64;
    let class = size_class_index(size);
    COUNTERS.size_class_bytes[class].fetch_add(size, Ordering::Relaxed);
    COUNTERS.size_class_blocks[class].fetch_add(1, Ordering::Relaxed);
    let current = COUNTERS
        .current_bytes
        .fetch_add(size, Ordering::Relaxed)
        .saturating_add(size);
    COUNTERS.peak_bytes.fetch_max(current, Ordering::Relaxed);
    let live = COUNTERS
        .live_blocks
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    COUNTERS.peak_live_blocks.fetch_max(live, Ordering::Relaxed);
    saturating_add(&COUNTERS.total_bytes, size);
    saturating_add(&COUNTERS.total_blocks, 1);
    // Set last with release ordering: a reader that observes `installed` also
    // observes the counters of this allocation.
    COUNTERS.installed.store(true, Ordering::Release);
    // Site sampling is opt-in and, when disabled, this is one relaxed load.
    sampling::record_allocation(size);
}

/// Records a block `size` bytes large released through the allocator.
///
/// A release larger than the current total saturates at zero instead of
/// wrapping, so a block freed without having been counted cannot publish an
/// absurd byte count.
fn record_dealloc(size: usize) {
    let size = size as u64;
    let class = size_class_index(size);
    saturating_sub(&COUNTERS.size_class_bytes[class], size);
    saturating_sub(&COUNTERS.size_class_blocks[class], 1);
    saturating_sub(&COUNTERS.current_bytes, size);
    saturating_sub(&COUNTERS.live_blocks, 1);
    saturating_add(&COUNTERS.deallocated_bytes, size);
    saturating_add(&COUNTERS.deallocated_blocks, 1);
}

/// Records a successful reallocation from `old_size` to `new_size` bytes.
///
/// The old block is one release and the new block is one allocation, which is
/// how the total block count and the deallocated block count stay additive. The
/// live block itself is unchanged, so `live_blocks` is not touched.
fn record_realloc(old_size: usize, new_size: usize) {
    let old_size = old_size as u64;
    let new_size = new_size as u64;
    let old_class = size_class_index(old_size);
    saturating_sub(&COUNTERS.size_class_bytes[old_class], old_size);
    saturating_sub(&COUNTERS.size_class_blocks[old_class], 1);
    let new_class = size_class_index(new_size);
    COUNTERS.size_class_bytes[new_class].fetch_add(new_size, Ordering::Relaxed);
    COUNTERS.size_class_blocks[new_class].fetch_add(1, Ordering::Relaxed);
    if new_size >= old_size {
        let grown = new_size - old_size;
        let current = COUNTERS
            .current_bytes
            .fetch_add(grown, Ordering::Relaxed)
            .saturating_add(grown);
        COUNTERS.peak_bytes.fetch_max(current, Ordering::Relaxed);
    } else {
        saturating_sub(&COUNTERS.current_bytes, old_size - new_size);
    }
    saturating_add(&COUNTERS.total_bytes, new_size);
    saturating_add(&COUNTERS.deallocated_bytes, old_size);
    saturating_add(&COUNTERS.total_blocks, 1);
    saturating_add(&COUNTERS.deallocated_blocks, 1);
    COUNTERS.installed.store(true, Ordering::Release);
}

struct Counters {
    installed: AtomicBool,
    current_bytes: AtomicU64,
    peak_bytes: AtomicU64,
    total_bytes: AtomicU64,
    deallocated_bytes: AtomicU64,
    live_blocks: AtomicU64,
    peak_live_blocks: AtomicU64,
    total_blocks: AtomicU64,
    deallocated_blocks: AtomicU64,
    size_class_bytes: [AtomicU64; SIZE_CLASS_COUNT],
    size_class_blocks: [AtomicU64; SIZE_CLASS_COUNT],
}

impl Counters {
    const fn new() -> Self {
        Self {
            installed: AtomicBool::new(false),
            current_bytes: AtomicU64::new(0),
            peak_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            deallocated_bytes: AtomicU64::new(0),
            live_blocks: AtomicU64::new(0),
            peak_live_blocks: AtomicU64::new(0),
            total_blocks: AtomicU64::new(0),
            deallocated_blocks: AtomicU64::new(0),
            size_class_bytes: [const { AtomicU64::new(0) }; SIZE_CLASS_COUNT],
            size_class_blocks: [const { AtomicU64::new(0) }; SIZE_CLASS_COUNT],
        }
    }
}

static COUNTERS: Counters = Counters::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_classes_cover_every_size_once() {
        assert_eq!(size_class_index(0), 0);
        assert_eq!(size_class_index(1), 0);
        assert_eq!(size_class_index(64), 0);
        assert_eq!(size_class_index(65), 1);
        assert_eq!(size_class_index(256), 1);
        assert_eq!(size_class_index(1024), 2);
        assert_eq!(size_class_index(1025), 3);
        assert_eq!(size_class_index(16_384), 4);
        assert_eq!(size_class_index(65_536), 5);
        assert_eq!(size_class_index(1_048_576), 6);
        assert_eq!(size_class_index(1_048_577), 7);
        assert_eq!(size_class_index(u64::MAX), SIZE_CLASS_COUNT - 1);

        let mut previous = 0;
        for size in (0u64..=1_100_000).step_by(3) {
            let class = size_class_index(size);
            assert!(
                class >= previous,
                "size class regressed at size {size}: {previous} then {class}"
            );
            previous = class;
            let upper = SIZE_CLASS_UPPER_BOUNDS.get(class).copied();
            match upper {
                Some(upper) => assert!(size <= upper, "{size} must fit class {class}"),
                None => assert!(size > SIZE_CLASS_UPPER_BOUNDS[SIZE_CLASS_COUNT - 2]),
            }
        }
    }

    /// This test binary never installs the allocator, so the counters must be
    /// absent rather than zero.
    #[test]
    fn an_uninstrumented_process_reports_nothing() {
        assert!(
            !installed(),
            "the lib test binary must not install tracking"
        );
        assert_eq!(snapshot(), None);
    }
}
