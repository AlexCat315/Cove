//! Checks the uninstrumented path reports nothing, then proves the counters
//! switch on only after the allocator has served an allocation.
//!
//! This binary never installs a `#[global_allocator]`, so `std::alloc` (and the
//! test harness itself) use the system allocator; the tracker is used only
//! through its own methods, which makes the accounting exact.

use cove_alloc::TrackingAllocator;
use std::alloc::{GlobalAlloc, Layout, alloc, dealloc, realloc};

fn main() {
    // No tracker has served anything, so there are no counters to publish.
    assert!(!cove_alloc::installed());
    assert_eq!(cove_alloc::snapshot(), None);

    // Ordinary system allocations do not install the tracker.
    let layout = Layout::from_size_align(1 << 16, 8).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { alloc(layout) };
    assert!(!pointer.is_null());
    let grown_layout = Layout::from_size_align(1 << 17, 8).expect("valid layout");
    // SAFETY: `pointer` came from `alloc` with `layout` and the new size keeps
    // the alignment.
    let grown = unsafe { realloc(pointer, layout, 1 << 17) };
    assert!(!grown.is_null());
    // SAFETY: `grown` came from `realloc` with the 128 KiB layout.
    unsafe { dealloc(grown, grown_layout) };
    assert!(
        !cove_alloc::installed(),
        "allocations outside the tracker must not install it"
    );
    assert_eq!(
        cove_alloc::snapshot(),
        None,
        "an uninstrumented process must omit the counters instead of reporting zeros"
    );

    // Serving the first allocation through a tracker flips it on.
    let tracker = TrackingAllocator::new();
    let layout = Layout::from_size_align(4096, 16).expect("valid layout");
    // SAFETY: the layout has a non-zero size, and the tracker forwards to the
    // system allocator.
    let pointer = unsafe { tracker.alloc(layout) };
    assert!(!pointer.is_null());
    assert!(
        cove_alloc::installed(),
        "one served allocation installs the counters"
    );
    let served = cove_alloc::snapshot().expect("the counters are installed");
    assert_eq!(served.current_bytes, 4096);
    assert_eq!(served.live_blocks, 1);
    assert_eq!(served.total_blocks, 1);
    assert_eq!(served.deallocated_bytes, 0);
    assert_eq!(served.deallocated_blocks, 0);
    assert_eq!(served.peak_bytes, 4096);
    assert_eq!(served.peak_live_blocks, 1);
    assert_eq!(served.size_classes[3].live_bytes, 4096);
    assert_eq!(served.size_classes[3].live_blocks, 1);

    // A zero-sized reallocation is a release, not an allocation.
    // SAFETY: `pointer` came from `alloc` with `layout`; a zero `new_size`
    // releases it.
    let released = unsafe { tracker.realloc(pointer, layout, 0) };
    assert!(released.is_null(), "a zero-sized realloc returns null");
    let after_release = cove_alloc::snapshot().expect("the counters stay installed");
    assert_eq!(after_release.current_bytes, 0);
    assert_eq!(after_release.live_blocks, 0);
    assert_eq!(after_release.deallocated_bytes, 4096);
    assert_eq!(after_release.deallocated_blocks, 1);
    assert_eq!(after_release.total_blocks, 1);
    assert_eq!(after_release.peak_bytes, 4096);
    assert_eq!(after_release.size_classes[3].live_bytes, 0);

    // A plain deallocation is counted against the block's layout size.
    let tiny = Layout::from_size_align(100, 8).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { tracker.alloc(tiny) };
    assert!(!pointer.is_null());
    // SAFETY: `pointer` came from `alloc` with `tiny`.
    unsafe { tracker.dealloc(pointer, tiny) };
    let after_dealloc = cove_alloc::snapshot().expect("the counters stay installed");
    assert_eq!(after_dealloc.current_bytes, 0);
    assert_eq!(after_dealloc.live_blocks, 0);
    assert_eq!(after_dealloc.total_blocks, 2);
    assert_eq!(after_dealloc.deallocated_blocks, 2);
    assert_eq!(after_dealloc.deallocated_bytes, 4196);
    assert_eq!(after_dealloc.size_classes[0].live_bytes, 0);

    println!(
        "no-install path: omitted, then current={} peak={} total={} deallocated={}",
        after_dealloc.current_bytes,
        after_dealloc.peak_bytes,
        after_dealloc.total_bytes,
        after_dealloc.deallocated_bytes
    );
}
