//! Linux and Android sampler: SIGPROF delivered to one thread with `tgkill`.
//!
//! The handler runs on the sampled thread itself, reads the registers the
//! kernel saved in its signal context and walks the frame-pointer chain inside
//! that thread's own stack mapping. No thread is suspended, and no ptrace,
//! perf event or elevated right is required.
//!
//! The handler is async-signal-safe: it allocates nothing, takes no lock and
//! calls only `gettid` and `clock_gettime`. Stack bounds come from a snapshot
//! of `/proc/self/maps` taken before the handler was installed, so the handler
//! never opens a file — reading maps from a handler could deadlock against a
//! thread interrupted inside `mmap`.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::{MAX_STACK_DEPTH, SampleConfig};
use crate::platform::fp::{self, StackBounds};
use crate::sampler::Collected;
use crate::{PlatformSupport, Sample, SamplingError, SymbolizationError};

mod symbols;
mod unwind;

pub(crate) use symbols::Symbolizer;

/// The kernel truncates a thread name to this many bytes (`TASK_COMM_LEN - 1`).
const COMM_LEN: usize = 15;
/// Longest `stop` waits for an in-flight handler before leaking its state.
const HANDLER_DRAIN: Duration = Duration::from_millis(100);

/// One process-wide SIGPROF sampler at a time.
static CLAIMED: AtomicBool = AtomicBool::new(false);
/// Published while the handler may run; cleared before the state is released.
static SHARED: AtomicPtr<Shared> = AtomicPtr::new(std::ptr::null_mut());
/// Handlers that entered but have not finished, including ones that found
/// `SHARED` empty. Incremented before the pointer is read so `stop` cannot
/// reclaim state a handler is about to use.
static IN_HANDLER: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn support() -> PlatformSupport {
    PlatformSupport::Supported
}

pub(crate) fn symbolizer() -> Result<Symbolizer, SymbolizationError> {
    Ok(Symbolizer::new())
}

/// State the signal handler reads. Immutable except for the ring and counters.
struct Shared {
    tid: libc::pid_t,
    started_ns: u64,
    max_stack_depth: usize,
    /// Sorted writable mappings, immutable for the whole run.
    writable: Vec<(u64, u64)>,
    slots: UnsafeCell<Box<[RawSample]>>,
    head: AtomicUsize,
    taken: Arc<AtomicU64>,
}

// SAFETY: the only writer is the SIGPROF handler, which runs on the single
// sampled thread and is not re-entered (SIGPROF is blocked during its own
// handler). The consumer reads the slots only after the handler has been
// uninstalled and every in-flight invocation has finished.
unsafe impl Sync for Shared {}

/// A running sampler thread and the thread it signals.
pub(crate) struct Sampler {
    shared: Arc<Shared>,
    done: Option<JoinHandle<Collected>>,
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicU64>,
    previous: libc::sigaction,
}

impl Sampler {
    pub(crate) fn start(thread_name: &str, config: &SampleConfig) -> Result<Self, SamplingError> {
        let tid = find_thread(thread_name)?;
        if CLAIMED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(SamplingError::SamplerBusy);
        }
        let started_ns = monotonic_ns();
        let taken = Arc::new(AtomicU64::new(0));
        let shared = Arc::new(Shared {
            tid,
            started_ns,
            max_stack_depth: config.max_stack_depth,
            writable: writable_mappings(),
            slots: UnsafeCell::new(vec![RawSample::EMPTY; config.max_samples].into_boxed_slice()),
            head: AtomicUsize::new(0),
            taken: Arc::clone(&taken),
        });
        let previous = match install_handler() {
            Ok(previous) => previous,
            Err(error) => {
                CLAIMED.store(false, Ordering::Release);
                return Err(error);
            }
        };
        SHARED.store(Arc::as_ptr(&shared).cast_mut(), Ordering::Release);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_taken = Arc::clone(&taken);
        let thread_config = config.clone();
        let handle = thread::Builder::new()
            .name("cove-cpu-sampler".into())
            .spawn(move || run(tid, thread_config, thread_stop, thread_taken));
        let handle = match handle {
            Ok(handle) => handle,
            Err(error) => {
                uninstall_handler(&previous);
                SHARED.store(std::ptr::null_mut(), Ordering::Release);
                CLAIMED.store(false, Ordering::Release);
                return Err(SamplingError::ThreadSpawn(error.to_string()));
            }
        };
        Ok(Self {
            shared,
            done: Some(handle),
            stop,
            taken,
            previous,
        })
    }

    pub(crate) fn samples_taken(&self) -> u64 {
        self.taken.load(Ordering::Relaxed)
    }

    pub(crate) fn stop(&mut self) -> Result<Collected, SamplingError> {
        self.stop.store(true, Ordering::Release);
        let Some(handle) = self.done.take() else {
            return Err(SamplingError::SamplerPanicked);
        };
        let joined = handle.join();
        // Uninstall first so no new invocation can start, then wait for the
        // ones already inside before the ring is read.
        uninstall_handler(&self.previous);
        if !drain_handler() {
            // A handler is still running after the bound. Keep its state alive
            // for the rest of the process rather than freeing it underneath.
            std::mem::forget(Arc::clone(&self.shared));
        }
        SHARED.store(std::ptr::null_mut(), Ordering::Release);
        CLAIMED.store(false, Ordering::Release);
        let collected = joined.map_err(|_| SamplingError::SamplerPanicked)?;
        if let Some(error) = collected.failure {
            return Err(error);
        }
        let (samples, samples_dropped) = self.shared.drain();
        Ok(Collected {
            samples,
            samples_dropped,
            ..collected
        })
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        if self.done.is_some() {
            let _ = self.stop();
        }
    }
}

impl Shared {
    /// Turns the retained ring into chronological samples.
    fn drain(&self) -> (Vec<Sample>, u64) {
        let taken = self.taken.load(Ordering::Acquire);
        // SAFETY: the handler has been uninstalled and drained, so the slots
        // are no longer written; this is the only reader.
        let slots = unsafe { &*self.slots.get() };
        let capacity = slots.len();
        let retained = taken.min(capacity as u64) as usize;
        let head = self.head.load(Ordering::Acquire);
        let first = if taken >= capacity as u64 { head } else { 0 };
        let samples = (0..retained)
            .map(|offset| {
                let slot = &slots[(first + offset) % capacity];
                Sample {
                    offset: Duration::from_nanos(slot.offset_ns),
                    stack: slot.frames[..usize::from(slot.depth)].to_vec(),
                }
            })
            .collect();
        (samples, taken - retained as u64)
    }
}

#[derive(Clone, Copy)]
struct RawSample {
    offset_ns: u64,
    depth: u8,
    frames: [u64; MAX_STACK_DEPTH],
}

impl RawSample {
    const EMPTY: Self = Self {
        offset_ns: 0,
        depth: 0,
        frames: [0; MAX_STACK_DEPTH],
    };
}

/// Finds a live thread by its kernel name, excluding the calling thread.
fn find_thread(name: &str) -> Result<libc::pid_t, SamplingError> {
    // SAFETY: gettid only reads the calling thread's id.
    let caller = unsafe { libc::syscall(libc::SYS_gettid) } as libc::pid_t;
    let entries = std::fs::read_dir("/proc/self/task")
        .map_err(|error| SamplingError::ThreadDiscovery(error.raw_os_error().unwrap_or(0)))?;
    let mut selected = None;
    let mut caller_matched = false;
    for entry in entries.flatten() {
        let Some(tid) = entry
            .file_name()
            .to_str()
            .and_then(|text| text.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        let Ok(mut comm) = std::fs::read(entry.path().join("comm")) else {
            continue;
        };
        if comm.last() == Some(&b'\n') {
            comm.pop();
        }
        if !name_matches(&comm, name.as_bytes()) {
            continue;
        }
        if tid == caller {
            caller_matched = true;
        } else if selected.is_none() {
            selected = Some(tid);
        }
    }
    if caller_matched {
        return Err(SamplingError::TargetIsCaller);
    }
    selected.ok_or_else(|| SamplingError::ThreadNotFound(name.to_owned()))
}

/// The kernel stores at most [`COMM_LEN`] bytes of a pthread name.
fn name_matches(comm: &[u8], name: &[u8]) -> bool {
    comm == name || (name.len() > COMM_LEN && comm == &name[..COMM_LEN])
}

/// Writable mappings, sorted so the handler can binary-search them.
fn writable_mappings() -> Vec<(u64, u64)> {
    let Ok(text) = std::fs::read_to_string("/proc/self/maps") else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(range), Some(permissions)) = (fields.next(), fields.next()) else {
            continue;
        };
        if !permissions.starts_with("rw") {
            continue;
        }
        let Some((start, end)) = range.split_once('-') else {
            continue;
        };
        if let (Ok(start), Ok(end)) = (
            u64::from_str_radix(start, 16),
            u64::from_str_radix(end, 16),
        ) && end > start
        {
            ranges.push((start, end));
        }
    }
    ranges.sort_unstable();
    ranges
}

/// The mapping that holds `sp`, which is the sampled thread's stack.
fn stack_bounds(writable: &[(u64, u64)], sp: u64) -> Option<StackBounds> {
    let index = writable.partition_point(|(start, _)| *start <= sp);
    let (start, end) = *writable.get(index.checked_sub(1)?)?;
    if sp < start || sp >= end {
        return None;
    }
    StackBounds::new(end, end - start)
}

fn monotonic_ns() -> u64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime is async-signal-safe and writes one timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return 0;
    }
    (now.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(now.tv_nsec as u64)
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Installs the handler, refusing to displace one the application owns.
fn install_handler() -> Result<libc::sigaction, SamplingError> {
    // SAFETY: a zeroed sigaction is a valid empty disposition for the kernel
    // to overwrite with the current one.
    let mut previous = unsafe { std::mem::zeroed::<libc::sigaction>() };
    // SAFETY: a null action asks for the current disposition only.
    if unsafe { libc::sigaction(libc::SIGPROF, std::ptr::null(), &mut previous) } != 0 {
        return Err(SamplingError::SignalSetup(errno()));
    }
    if previous.sa_sigaction != libc::SIG_DFL && previous.sa_sigaction != libc::SIG_IGN {
        return Err(SamplingError::SamplerBusy);
    }
    // SAFETY: a zeroed sigaction is valid; every field used below is set.
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = sample_signal as *const () as usize;
    action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
    // SAFETY: sigemptyset initializes the mask to empty. SIGPROF is blocked
    // during its own handler by default, so the handler cannot re-enter.
    unsafe { libc::sigemptyset(&mut action.sa_mask) };
    // SAFETY: `action` holds a valid handler, flags and mask.
    if unsafe { libc::sigaction(libc::SIGPROF, &action, std::ptr::null_mut()) } != 0 {
        return Err(SamplingError::SignalSetup(errno()));
    }
    Ok(previous)
}

fn uninstall_handler(previous: &libc::sigaction) {
    // SAFETY: `previous` is the disposition read before installation.
    unsafe { libc::sigaction(libc::SIGPROF, previous, std::ptr::null_mut()) };
}

/// Waits for in-flight handlers; false when the bound elapsed first.
fn drain_handler() -> bool {
    let deadline = Instant::now() + HANDLER_DRAIN;
    while IN_HANDLER.load(Ordering::Acquire) != 0 {
        if Instant::now() >= deadline {
            return false;
        }
        thread::yield_now();
    }
    true
}

/// Records one sample on the sampled thread. Allocates nothing and takes no
/// lock, so it is safe to run wherever SIGPROF arrives.
extern "C" fn sample_signal(
    _signal: libc::c_int,
    _info: *mut libc::siginfo_t,
    context: *mut libc::c_void,
) {
    IN_HANDLER.fetch_add(1, Ordering::AcqRel);
    record(context);
    IN_HANDLER.fetch_sub(1, Ordering::AcqRel);
}

fn record(context: *mut libc::c_void) {
    let shared = SHARED.load(Ordering::Acquire);
    if shared.is_null() {
        return;
    }
    // SAFETY: `stop` keeps the owning `Arc` alive until `IN_HANDLER` reaches
    // zero, and this invocation is counted before the pointer was read.
    let shared = unsafe { &*shared };
    // SAFETY: gettid only reads the calling thread's id.
    let tid = unsafe { libc::syscall(libc::SYS_gettid) } as libc::pid_t;
    if tid != shared.tid {
        return;
    }
    let Some(registers) = unwind::read_registers(context.cast::<libc::ucontext_t>()) else {
        return;
    };
    let bounds = stack_bounds(&shared.writable, registers.sp);
    // SAFETY: this is the only writer, the index is below the capacity, and
    // the consumer reads only after the handler is uninstalled and drained.
    let slots = unsafe { (&mut *shared.slots.get()).as_mut_ptr() };
    let capacity = shared.max_samples_capacity();
    let index = shared.head.fetch_add(1, Ordering::Relaxed) % capacity;
    // SAFETY: `index` is below the capacity of the preallocated ring.
    let slot = unsafe { &mut *slots.add(index) };
    slot.frames = [0; MAX_STACK_DEPTH];
    let depth = fp::unwind(
        registers,
        bounds,
        shared.max_stack_depth,
        unwind::ADDRESS_MASK,
        &mut slot.frames,
    );
    slot.depth = depth as u8;
    slot.offset_ns = monotonic_ns().saturating_sub(shared.started_ns);
    shared.taken.fetch_add(1, Ordering::Release);
}

impl Shared {
    fn max_samples_capacity(&self) -> usize {
        // SAFETY: the ring is never resized while the handler runs.
        unsafe { (&*self.slots.get()).len() }
    }
}

fn run(
    tid: libc::pid_t,
    config: SampleConfig,
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicU64>,
) -> Collected {
    let started = Instant::now();
    let pid = std::process::id() as libc::c_long;
    let mut failure = None;
    while !stop.load(Ordering::Acquire) {
        let tick = Instant::now();
        // SAFETY: tgkill delivers SIGPROF to one thread of this process; the
        // arguments are widened to the syscall's register width.
        let result = unsafe {
            libc::syscall(
                libc::SYS_tgkill,
                pid,
                libc::c_long::from(tid),
                libc::c_long::from(libc::SIGPROF),
            )
        };
        if result != 0 {
            failure = Some(SamplingError::TargetUnavailable(errno()));
            break;
        }
        let spent = tick.elapsed();
        if spent < config.interval {
            thread::sleep(config.interval - spent);
        }
    }
    Collected {
        duration: started.elapsed(),
        samples_taken: taken.load(Ordering::Relaxed),
        samples_dropped: 0,
        samples: Vec::new(),
        failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_names_the_kernel_truncated() {
        assert!(name_matches(b"cove-ui", b"cove-ui"));
        assert!(name_matches(b"cove-ui-thread-", b"cove-ui-thread-1234"));
        assert!(!name_matches(b"cove-ui", b"cove-u"));
        assert!(!name_matches(b"other", b"cove-ui"));
    }

    #[test]
    fn finds_the_mapping_that_holds_the_stack_pointer() {
        let writable = [(0x1000, 0x2000), (0x7000, 0x9000), (0x20000, 0x30000)];
        let bounds = stack_bounds(&writable, 0x8000).expect("bounds");
        assert_eq!(bounds.low(), 0x7000);
        assert_eq!(bounds.top(), 0x9000);
        assert!(stack_bounds(&writable, 0x9000).is_none());
        assert!(stack_bounds(&writable, 0x500).is_none());
        assert!(stack_bounds(&[], 0x8000).is_none());
    }

    #[test]
    fn ring_keeps_the_newest_samples_in_order() {
        let taken = Arc::new(AtomicU64::new(0));
        let shared = Shared {
            tid: 0,
            started_ns: 0,
            max_stack_depth: 4,
            writable: Vec::new(),
            slots: UnsafeCell::new(vec![RawSample::EMPTY; 3].into_boxed_slice()),
            head: AtomicUsize::new(0),
            taken: Arc::clone(&taken),
        };
        for offset in 0..5_u64 {
            let index = shared.head.fetch_add(1, Ordering::Relaxed) % 3;
            // SAFETY: single-threaded test writer, index below capacity.
            let slot = unsafe { &mut *(&mut *shared.slots.get()).as_mut_ptr().add(index) };
            slot.offset_ns = offset * 1_000_000;
            slot.depth = 1;
            slot.frames[0] = 0x1000 + offset;
            taken.fetch_add(1, Ordering::Release);
        }
        let (samples, dropped) = shared.drain();
        assert_eq!(dropped, 2);
        assert_eq!(
            samples.iter().map(|s| s.offset).collect::<Vec<_>>(),
            vec![
                Duration::from_millis(2),
                Duration::from_millis(3),
                Duration::from_millis(4)
            ]
        );
        assert_eq!(samples[2].stack, vec![0x1004]);
    }

    #[test]
    fn ring_does_not_report_drops_before_it_is_full() {
        let taken = Arc::new(AtomicU64::new(0));
        let shared = Shared {
            tid: 0,
            started_ns: 0,
            max_stack_depth: 4,
            writable: Vec::new(),
            slots: UnsafeCell::new(vec![RawSample::EMPTY; 4].into_boxed_slice()),
            head: AtomicUsize::new(0),
            taken: Arc::clone(&taken),
        };
        for offset in 0..2_u64 {
            let index = shared.head.fetch_add(1, Ordering::Relaxed) % 4;
            // SAFETY: single-threaded test writer, index below capacity.
            let slot = unsafe { &mut *(&mut *shared.slots.get()).as_mut_ptr().add(index) };
            slot.offset_ns = offset;
            slot.depth = 1;
            taken.fetch_add(1, Ordering::Release);
        }
        let (samples, dropped) = shared.drain();
        assert_eq!(samples.len(), 2);
        assert_eq!(dropped, 0);
    }
}
