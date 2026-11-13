//! Opt-in allocation-site sampling.
//!
//! [`start_sampling`] turns on probabilistic stack capture for allocations that
//! pass through [`TrackingAllocator`](crate::TrackingAllocator). Every
//! `rate`-th successful `alloc`/`alloc_zeroed` captures up to
//! [`MAX_SITE_FRAMES`](crate::MAX_SITE_FRAMES) raw instruction addresses into a fixed
//! lock-free ring. [`stop_sampling`] halts capture and returns the retained
//! samples aggregated by identical stack, with bytes and blocks per site.
//!
//! # What is real
//!
//! Stacks are captured for real on every platform Cove ships on: `libc::backtrace`
//! on Apple and GNU Linux, `RtlCaptureStackBackTrace` on Windows, and the Itanium
//! unwinder's `_Unwind_Backtrace` on Android and musl Linux. Each backend skips its
//! own capture frame, so the innermost retained address is the sampling hook on
//! every platform. The addresses are raw instruction pointers; symbolization is a
//! later layer. Identical addresses within one process identify one allocation
//! site, so aggregation by stack is exact for the (unsymbolized) data that was
//! captured.
//!
//! # Rules
//!
//! - Sampling is off until `start_sampling` is called. A disabled hook is one
//!   relaxed atomic load, and no stack is captured.
//! - The hook never allocates: the ring is fixed, its writes are atomics, and
//!   aggregation happens only in `stop_sampling`/`allocation_sites`, where
//!   allocation is allowed.
//! - A thread-local guard prevents recursion when the unwinder itself
//!   allocates. The allocator counters still account for that allocation; only
//!   the nested sample is skipped.
//! - Capture is only attempted for allocations that returned a non-null
//!   pointer. A failed capture increments `capture_failures`; no site is
//!   fabricated.
//! - `stop_sampling` and `allocation_sites` briefly halt capture so a reader
//!   sees a stable ring. Allocations during that pause are not sampled.
//! - The ring holds [`SITE_RING_CAPACITY`] samples; older samples are
//!   overwritten and counted in `samples_dropped`.
//!
//! # Platform gaps
//!
//! wasm and any other target with no stack walker — including a Linux libc that
//! is neither glibc nor musl — report [`SamplingSupport::Unsupported`] with a
//! reason. [`start_sampling`] returns [`SamplingError::Unsupported`] there
//! instead of enabling a sampler that cannot capture stacks.

mod capture;
mod ring;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Most instruction addresses retained for one allocation site.
pub const MAX_SITE_FRAMES: usize = ring::MAX_FRAMES;

/// Number of samples the ring can hold before overwriting the oldest.
pub const SITE_RING_CAPACITY: usize = ring::CAPACITY;

/// Whether allocation-site sampling can capture real stacks on this target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplingSupport {
    /// A platform stack walker is available and sites are real.
    Supported,
    /// This target cannot capture stacks; the reason names the gap.
    Unsupported {
        /// Operating system reported by `std::env::consts::OS`.
        platform: &'static str,
        /// What is missing and what to use instead.
        reason: &'static str,
    },
}

/// Why sampling could not be enabled.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SamplingError {
    /// The target has no stack capture; no samples are produced.
    #[error("allocation-site sampling is not available on {platform}: {reason}")]
    Unsupported {
        /// Operating system reported by `std::env::consts::OS`.
        platform: &'static str,
        /// What is missing and what to use instead.
        reason: &'static str,
    },
    /// A rate of zero would never sample.
    #[error("allocation-site sampling rate must be at least 1")]
    InvalidRate,
    /// A session is already running; call `stop_sampling` first.
    #[error("allocation-site sampling is already running")]
    AlreadySampling,
}

/// One allocation site: a captured stack and what was allocated through it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllocationSite {
    /// Raw instruction addresses, innermost caller first.
    ///
    /// Addresses are absolute in-process values returned by the platform's stack
    /// walker; they are not symbolized and are not comparable across processes.
    /// The innermost few frames belong to the sampling hook itself; symbolization
    /// is the layer that filters them from the caller's view.
    pub frames: Vec<u64>,
    /// Bytes allocated through this exact stack.
    pub bytes: u64,
    /// Blocks allocated through this exact stack.
    pub blocks: u64,
}

/// Aggregated sites from one sampling session or query.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllocationSiteReport {
    /// The 1-in-`rate` sampling rate this report was collected with.
    pub rate: u32,
    /// Sites sorted by bytes descending, then blocks, then stack.
    pub sites: Vec<AllocationSite>,
    /// Sum of `sites[*].bytes`.
    pub sampled_bytes: u64,
    /// Sum of `sites[*].blocks`.
    pub sampled_blocks: u64,
    /// Sampling triggers (every `rate`-th successful allocation).
    pub samples_attempted: u64,
    /// Samples actually present in the ring.
    pub samples_retained: u64,
    /// Samples overwritten by newer ones, lost to a slot claim race, or
    /// rejected as incomplete.
    pub samples_dropped: u64,
    /// Sampling triggers where the platform's stack walker produced no caller frame.
    pub capture_failures: u64,
}

/// Reports whether this target can capture allocation stacks.
pub fn sampling_support() -> SamplingSupport {
    capture::support()
}

/// Whether a sampling session is currently active.
pub fn sampling_enabled() -> bool {
    SAMPLING.enabled.load(Ordering::Acquire)
}

/// Starts sampling every `rate`-th successful allocation.
///
/// `rate` is a positive 1-in-N divisor. The ring and all session counters are
/// cleared, so a report after this call contains only allocations that happened
/// after it. Fails with [`SamplingError::Unsupported`] where stacks cannot be
/// captured, and [`SamplingError::AlreadySampling`] if a session is running.
pub fn start_sampling(rate: u32) -> Result<(), SamplingError> {
    if let SamplingSupport::Unsupported { platform, reason } = capture::support() {
        return Err(SamplingError::Unsupported { platform, reason });
    }
    if rate == 0 {
        return Err(SamplingError::InvalidRate);
    }
    let _control = lock_control();
    if SAMPLING.enabled.load(Ordering::SeqCst) {
        return Err(SamplingError::AlreadySampling);
    }
    // Run the unwinder's lazy initialization here, while the allocator hook is
    // disabled, so the first sampled allocation does not trigger it.
    capture::prewarm();
    clear_samples();
    SAMPLING.rate.store(rate, Ordering::Relaxed);
    // A seq-cst store orders the cleared ring and rate counter before any hook
    // that observes the session as enabled.
    SAMPLING.enabled.store(true, Ordering::SeqCst);
    Ok(())
}

/// Stops sampling, returns the aggregated sites and clears the session.
///
/// Returns `None` when no session is active. On an unsupported target no
/// session can start, so this also returns `None` there.
pub fn stop_sampling() -> Option<AllocationSiteReport> {
    let _control = lock_control();
    if !SAMPLING.enabled.load(Ordering::SeqCst) {
        return None;
    }
    halt();
    let report = build_report();
    clear_samples();
    Some(report)
}

/// Returns the sites collected so far without ending the session.
///
/// Capture is halted while the ring is read so the answer is a stable snapshot;
/// allocations during that pause are not sampled. Returns `None` when no
/// session is active.
pub fn allocation_sites() -> Option<AllocationSiteReport> {
    if !SAMPLING.enabled.load(Ordering::Acquire) {
        return None;
    }
    let _control = lock_control();
    if !SAMPLING.enabled.load(Ordering::SeqCst) {
        return None;
    }
    halt();
    let report = build_report();
    SAMPLING.enabled.store(true, Ordering::SeqCst);
    Some(report)
}

/// Clears the retained sites and session counters, keeping sampling active.
///
/// Returns `false` when no session is active. Capture is halted while the ring
/// is cleared, so a sample cannot be half-written across the reset.
pub fn reset_allocation_sites() -> bool {
    let _control = lock_control();
    if !SAMPLING.enabled.load(Ordering::SeqCst) {
        return false;
    }
    halt();
    clear_samples();
    SAMPLING.enabled.store(true, Ordering::SeqCst);
    true
}

/// Called by the allocator after a successful allocation has been counted.
///
/// The first check is the disabled fast path: a relaxed load, no TLS access and
/// no atomic write.
#[inline]
pub(crate) fn record_allocation(bytes: u64) {
    if SAMPLING.enabled.load(Ordering::Relaxed) {
        record_allocation_enabled(bytes);
    }
}

#[inline(never)]
fn record_allocation_enabled(bytes: u64) {
    // Check recursion before touching any shared state: an allocation caused by
    // the unwinder must not register a producer or advance the rate counter.
    let Some(_recursion) = RecursionGuard::enter() else {
        return;
    };
    let Some(_producer) = ActiveProducer::enter() else {
        return;
    };
    let rate = SAMPLING.rate.load(Ordering::Relaxed);
    let count = SAMPLING
        .allocations
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    if rate == 0 || !count.is_multiple_of(u64::from(rate)) {
        return;
    }
    SAMPLING.attempts.fetch_add(1, Ordering::Relaxed);
    let mut frames = [0usize; MAX_SITE_FRAMES];
    match capture::capture_into(&mut frames) {
        Some(depth) if depth > 0 => {
            SAMPLING.ring.push(&frames[..depth], bytes as usize, 1);
        }
        _ => {
            SAMPLING.capture_failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Disables capture and waits until every in-flight hook has finished.
///
/// A producer registers itself before re-checking `enabled`, and a producer
/// that registers after this store observes the store and aborts, so waiting
/// for the count to reach zero cannot miss an in-flight writer.
fn halt() {
    SAMPLING.enabled.store(false, Ordering::SeqCst);
    while SAMPLING.active_producers.load(Ordering::SeqCst) != 0 {
        std::hint::spin_loop();
    }
}

fn clear_samples() {
    SAMPLING.ring.clear();
    SAMPLING.allocations.store(0, Ordering::Relaxed);
    SAMPLING.attempts.store(0, Ordering::Relaxed);
    SAMPLING.capture_failures.store(0, Ordering::Relaxed);
}

fn build_report() -> AllocationSiteReport {
    let mut totals: BTreeMap<Vec<u64>, (u64, u64)> = BTreeMap::new();
    let retained = SAMPLING.ring.visit(|frames, bytes, blocks| {
        let key: Vec<u64> = frames.iter().map(|frame| *frame as u64).collect();
        let entry = totals.entry(key).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(bytes as u64);
        entry.1 = entry.1.saturating_add(blocks as u64);
    });
    let mut sites: Vec<AllocationSite> = totals
        .into_iter()
        .map(|(frames, (bytes, blocks))| AllocationSite {
            frames,
            bytes,
            blocks,
        })
        .collect();
    sites.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| right.blocks.cmp(&left.blocks))
            .then_with(|| left.frames.cmp(&right.frames))
    });
    let sampled_bytes = sites
        .iter()
        .fold(0u64, |sum, site| sum.saturating_add(site.bytes));
    let sampled_blocks = sites
        .iter()
        .fold(0u64, |sum, site| sum.saturating_add(site.blocks));
    let attempts = SAMPLING.attempts.load(Ordering::Relaxed);
    let capture_failures = SAMPLING.capture_failures.load(Ordering::Relaxed);
    AllocationSiteReport {
        rate: SAMPLING.rate.load(Ordering::Relaxed),
        sites,
        sampled_bytes,
        sampled_blocks,
        samples_attempted: attempts,
        samples_retained: retained,
        samples_dropped: attempts
            .saturating_sub(capture_failures)
            .saturating_sub(retained),
        capture_failures,
    }
}

fn lock_control() -> MutexGuard<'static, ()> {
    SAMPLING
        .control
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Skips nested sampling on the current thread.
///
/// The flag is `const`-initialized so reading it does not allocate on the TLS
/// paths this crate supports. `try_with` reports a destroyed TLS slot; that is
/// treated as "cannot sample safely" and the sample is skipped.
struct RecursionGuard;

thread_local! {
    static IN_SAMPLE: Cell<bool> = const { Cell::new(false) };
}

impl RecursionGuard {
    fn enter() -> Option<Self> {
        IN_SAMPLE
            .try_with(|in_sample| {
                if in_sample.get() {
                    None
                } else {
                    in_sample.set(true);
                    Some(Self)
                }
            })
            .ok()
            .flatten()
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        let _ = IN_SAMPLE.try_with(|in_sample| in_sample.set(false));
    }
}

/// Keeps `active_producers` nonzero until the hook has finished.
struct ActiveProducer;

impl ActiveProducer {
    fn enter() -> Option<Self> {
        SAMPLING.active_producers.fetch_add(1, Ordering::SeqCst);
        if SAMPLING.enabled.load(Ordering::SeqCst) {
            Some(Self)
        } else {
            SAMPLING.active_producers.fetch_sub(1, Ordering::SeqCst);
            None
        }
    }
}

impl Drop for ActiveProducer {
    fn drop(&mut self) {
        SAMPLING.active_producers.fetch_sub(1, Ordering::SeqCst);
    }
}

struct Sampling {
    enabled: AtomicBool,
    rate: AtomicU32,
    allocations: AtomicU64,
    attempts: AtomicU64,
    capture_failures: AtomicU64,
    active_producers: AtomicUsize,
    ring: ring::SampleRing,
    control: Mutex<()>,
}

impl Sampling {
    const fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            rate: AtomicU32::new(0),
            allocations: AtomicU64::new(0),
            attempts: AtomicU64::new(0),
            capture_failures: AtomicU64::new(0),
            active_producers: AtomicUsize::new(0),
            ring: ring::SampleRing::new(),
            control: Mutex::new(()),
        }
    }
}

static SAMPLING: Sampling = Sampling::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_names_a_real_capture_backend() {
        let support = sampling_support();
        let has_walker = cfg!(any(
            target_vendor = "apple",
            all(target_os = "linux", target_env = "gnu"),
            target_os = "windows",
            target_os = "android",
            all(target_os = "linux", target_env = "musl"),
        ));
        assert_eq!(
            matches!(support, SamplingSupport::Supported),
            has_walker,
            "supported must mean a real platform stack walker exists"
        );
        if let SamplingSupport::Unsupported { platform, reason } = support {
            assert!(!platform.is_empty(), "an unsupported answer names the OS");
            assert!(!reason.is_empty(), "an unsupported answer explains the gap");
        }
    }

    #[test]
    fn a_zero_rate_is_rejected_before_a_session_starts() {
        // Only meaningful where a session could otherwise start.
        if sampling_support() == SamplingSupport::Supported {
            assert_eq!(start_sampling(0), Err(SamplingError::InvalidRate));
            assert!(!sampling_enabled());
        }
    }
}
