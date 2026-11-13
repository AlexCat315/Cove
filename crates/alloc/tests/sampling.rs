//! Runs allocation-site sampling against the real process allocator.
//!
//! `harness = false` keeps libtest from allocating on background threads, so a
//! session started and stopped here sees only the allocations this binary makes.
//! A hang instead of a result would mean the sampling hook deadlocked, which is
//! why the concurrent check has a watchdog.

use cove_alloc::{
    AllocationSiteReport, MAX_SITE_FRAMES, SITE_RING_CAPACITY, SamplingError, SamplingSupport,
    TrackingAllocator,
};
use std::alloc::{Layout, alloc, dealloc};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[global_allocator]
static ALLOC: TrackingAllocator = TrackingAllocator::new();

fn main() {
    sampling_is_off_by_default();
    match cove_alloc::sampling_support() {
        SamplingSupport::Supported => supported_path(),
        SamplingSupport::Unsupported { platform, reason } => {
            unsupported_path_is_explicit(platform, reason)
        }
    }
    println!("allocation-site sampling checks passed");
}

fn sampling_is_off_by_default() {
    assert!(!cove_alloc::sampling_enabled());
    assert!(cove_alloc::allocation_sites().is_none());
    assert!(cove_alloc::stop_sampling().is_none());
    assert!(!cove_alloc::reset_allocation_sites());
}

fn supported_path() {
    assert_eq!(
        cove_alloc::start_sampling(0),
        Err(SamplingError::InvalidRate)
    );
    assert!(
        !cove_alloc::sampling_enabled(),
        "a rejected rate must not start a session"
    );

    cove_alloc::start_sampling(1).expect("Apple platforms and GNU Linux capture stacks");
    assert_eq!(
        cove_alloc::start_sampling(1),
        Err(SamplingError::AlreadySampling),
        "a second start is rejected instead of silently reconfiguring"
    );
    let _ = cove_alloc::stop_sampling();
    assert!(!cove_alloc::sampling_enabled());

    busy_loop_records_sites();
    ring_is_bounded_and_reports_drops();
    rate_limits_samples();
    stop_is_a_resetting_snapshot();
    reset_clears_sites_but_keeps_sampling();
    concurrent_allocation_does_not_deadlock();
}

fn busy_loop_records_sites() {
    const ALLOCATIONS: usize = 128;
    const SIZE: usize = 96;

    cove_alloc::start_sampling(1).expect("capture is supported");
    for _ in 0..ALLOCATIONS {
        allocate(SIZE);
    }
    let report = cove_alloc::stop_sampling().expect("a session was running");
    assert_eq!(report.rate, 1);
    assert_eq!(report.capture_failures, 0, "every stack must be captured");
    assert_eq!(report.samples_attempted, ALLOCATIONS as u64);
    assert_eq!(
        report.samples_retained, report.samples_attempted,
        "the ring has room for every sample"
    );
    assert_eq!(report.samples_dropped, 0);
    assert_eq!(report.sampled_blocks, ALLOCATIONS as u64);
    assert_eq!(report.sampled_bytes, (ALLOCATIONS * SIZE) as u64);
    assert_eq!(
        report.sites.len(),
        1,
        "one loop with one allocation call site aggregates into one stack"
    );

    let mut bytes = 0u64;
    let mut blocks = 0u64;
    for site in &report.sites {
        assert!(
            !site.frames.is_empty() && site.frames.len() <= MAX_SITE_FRAMES,
            "a site must carry a bounded, non-empty stack: {:?}",
            site.frames
        );
        assert!(
            site.frames.iter().all(|frame| *frame != 0),
            "a captured frame must be a real address: {:?}",
            site.frames
        );
        assert!(site.bytes > 0, "a sampled site allocates real bytes");
        assert!(site.blocks > 0, "a sampled site has a real block");
        bytes = bytes.saturating_add(site.bytes);
        blocks = blocks.saturating_add(site.blocks);
    }
    assert_eq!(bytes, report.sampled_bytes);
    assert_eq!(blocks, report.sampled_blocks);
}

fn ring_is_bounded_and_reports_drops() {
    const EXTRA: usize = 512;
    let allocations = SITE_RING_CAPACITY + EXTRA;

    cove_alloc::start_sampling(1).expect("capture is supported");
    for _ in 0..allocations {
        allocate(32);
    }
    let report = cove_alloc::stop_sampling().expect("a session was running");
    assert_eq!(report.capture_failures, 0);
    assert_eq!(report.samples_attempted, allocations as u64);
    assert_eq!(
        report.samples_retained, SITE_RING_CAPACITY as u64,
        "the ring retains exactly one full window of samples"
    );
    assert_eq!(
        report.samples_retained + report.samples_dropped,
        report.samples_attempted,
        "every attempted sample is retained or accounted as dropped"
    );
    assert_eq!(report.samples_dropped, EXTRA as u64);
}

fn rate_limits_samples() {
    let one = sample_once_every(1);
    let four = sample_once_every(4);
    assert_eq!(one.samples_attempted, 64);
    assert_eq!(four.samples_attempted, 16);
    assert_eq!(one.samples_retained, 64);
    assert_eq!(four.samples_retained, 16);
    assert_eq!(one.sampled_bytes, 64 * 64);
    assert_eq!(four.sampled_bytes, 16 * 64);
    assert!(
        four.samples_attempted < one.samples_attempted,
        "a larger rate must sample fewer allocations"
    );
}

fn sample_once_every(rate: u32) -> AllocationSiteReport {
    cove_alloc::start_sampling(rate).expect("capture is supported");
    for _ in 0..64 {
        allocate(64);
    }
    cove_alloc::stop_sampling().expect("a session was running")
}

fn stop_is_a_resetting_snapshot() {
    cove_alloc::start_sampling(1).expect("capture is supported");
    for _ in 0..32 {
        allocate(32);
    }
    let first = cove_alloc::stop_sampling().expect("a session was running");
    assert_eq!(first.samples_attempted, 32);
    assert_eq!(first.sampled_bytes, 32 * 32);
    assert!(
        cove_alloc::allocation_sites().is_none(),
        "a stopped session has no live query"
    );
    assert!(
        cove_alloc::stop_sampling().is_none(),
        "stopping twice is harmless and reports nothing"
    );

    cove_alloc::start_sampling(1).expect("capture is supported");
    let fresh = cove_alloc::allocation_sites().expect("the session is active");
    assert_eq!(
        fresh.samples_retained, 0,
        "a new session does not inherit the previous ring"
    );
    assert_eq!(fresh.samples_attempted, 0);
    assert!(fresh.sites.is_empty());
    let _ = cove_alloc::stop_sampling();
}

fn reset_clears_sites_but_keeps_sampling() {
    cove_alloc::start_sampling(1).expect("capture is supported");
    for _ in 0..16 {
        allocate(128);
    }
    let before = cove_alloc::allocation_sites().expect("the session is active");
    assert_eq!(before.samples_retained, 16);
    assert!(before.sampled_bytes > 0);

    assert!(
        cove_alloc::reset_allocation_sites(),
        "a session was running"
    );
    let after = cove_alloc::allocation_sites().expect("sampling continues");
    assert_eq!(after.samples_retained, 0);
    assert_eq!(after.samples_attempted, 0);
    assert_eq!(after.sampled_bytes, 0);
    assert!(cove_alloc::sampling_enabled());

    for _ in 0..8 {
        allocate(128);
    }
    let resumed = cove_alloc::allocation_sites().expect("sampling continues");
    assert_eq!(resumed.samples_retained, 8);
    assert_eq!(resumed.sampled_bytes, 8 * 128);

    let _ = cove_alloc::stop_sampling();
    assert!(!cove_alloc::sampling_enabled());
    assert!(
        !cove_alloc::reset_allocation_sites(),
        "resetting a stopped session reports that nothing was running"
    );
}

fn concurrent_allocation_does_not_deadlock() {
    cove_alloc::start_sampling(8).expect("capture is supported");
    let done = Arc::new(AtomicBool::new(false));
    let watchdog_done = Arc::clone(&done);
    // The process exits when `main` returns, so this thread is abandoned on
    // success; on a deadlock it aborts instead of hanging the test forever.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(15));
        if !watchdog_done.load(Ordering::Acquire) {
            std::process::abort();
        }
    });

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..512 {
                    let mut block = vec![0u8; 256];
                    block[0] = 1;
                    std::hint::black_box(&block);
                }
            });
        }
    });

    let report = cove_alloc::stop_sampling().expect("a session was running");
    assert!(
        report.samples_retained > 0,
        "concurrent allocation must produce samples"
    );
    assert_eq!(report.capture_failures, 0);
    done.store(true, Ordering::Release);
}

fn unsupported_path_is_explicit(platform: &'static str, reason: &'static str) {
    assert!(!platform.is_empty(), "the gap names the platform");
    assert!(!reason.is_empty(), "the gap explains what is missing");
    assert_eq!(
        cove_alloc::start_sampling(1),
        Err(SamplingError::Unsupported { platform, reason }),
        "an unsupported target fails explicitly instead of sampling nothing"
    );
    assert!(!cove_alloc::sampling_enabled());
    assert!(cove_alloc::allocation_sites().is_none());
    assert!(cove_alloc::stop_sampling().is_none());
    assert!(!cove_alloc::reset_allocation_sites());
}

fn allocate(size: usize) {
    let layout = Layout::from_size_align(size, 8).expect("valid layout");
    // SAFETY: the layout has a non-zero size.
    let pointer = unsafe { alloc(layout) };
    assert!(
        !pointer.is_null(),
        "the system allocator serves {size} bytes"
    );
    // SAFETY: `pointer` was allocated with `layout` and is freed once.
    unsafe {
        std::ptr::write_bytes(pointer, 0xA5, size);
        // Keep the block observable so an optimizing build cannot delete the
        // allocation (and with it the hook call) as a dead pair.
        std::hint::black_box(pointer);
        dealloc(pointer, layout);
    }
}
