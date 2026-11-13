//! Windows sampler: suspended-thread context and frame-pointer unwinding.
//!
//! Threads are enumerated with a toolhelp snapshot of this process and matched
//! by the description Rust sets with `SetThreadDescription`. The target is
//! suspended, its registers read with `GetThreadContext`, its stack walked
//! inside the committed region `VirtualQuery` reports, and it is resumed on
//! every exit path.

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, GetCurrentThreadId, GetThreadDescription, OpenThread, ResumeThread,
    SuspendThread, THREAD_GET_CONTEXT, THREAD_QUERY_LIMITED_INFORMATION, THREAD_SUSPEND_RESUME,
};

use crate::config::{MAX_STACK_DEPTH, SampleConfig};
use crate::platform::fp;
use crate::platform::ring::{RawSample, Ring};
use crate::sampler::Collected;
use crate::{PlatformSupport, SamplingError, SymbolizationError};

mod symbols;
mod unwind;

pub(crate) use symbols::Symbolizer;

pub(crate) fn support() -> PlatformSupport {
    PlatformSupport::Supported
}

pub(crate) fn symbolizer() -> Result<Symbolizer, SymbolizationError> {
    Ok(Symbolizer::new())
}

fn last_error() -> i32 {
    // SAFETY: GetLastError only reads the calling thread's last-error code.
    unsafe { GetLastError() as i32 }
}

/// Closes a handle on every exit path.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: this wrapper owns one handle taken from the kernel.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// A running sampler thread and the target handle it samples.
pub(crate) struct Sampler {
    /// Stored as an integer so the sampler stays `Send`.
    thread: isize,
    done: Option<JoinHandle<Collected>>,
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicU64>,
}

impl Sampler {
    pub(crate) fn start(thread_name: &str, config: &SampleConfig) -> Result<Self, SamplingError> {
        let tid = find_thread(thread_name)?;
        // SAFETY: the rights requested are exactly the ones sampling uses.
        let handle = unsafe {
            OpenThread(
                THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_LIMITED_INFORMATION,
                0,
                tid,
            )
        };
        if handle.is_null() {
            return Err(SamplingError::TargetUnavailable(last_error()));
        }
        let thread = handle as isize;
        let stop = Arc::new(AtomicBool::new(false));
        let taken = Arc::new(AtomicU64::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_taken = Arc::clone(&taken);
        let thread_config = config.clone();
        let spawned = thread::Builder::new()
            .name("cove-cpu-sampler".into())
            .spawn(move || run(thread, thread_config, thread_stop, thread_taken));
        let handle = match spawned {
            Ok(handle) => handle,
            Err(error) => {
                // SAFETY: the sampler thread never started, so this closes the
                // only handle taken for the target.
                unsafe { CloseHandle(thread as HANDLE) };
                return Err(SamplingError::ThreadSpawn(error.to_string()));
            }
        };
        Ok(Self {
            thread,
            done: Some(handle),
            stop,
            taken,
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
        // SAFETY: the sampler thread has been joined, so the target handle
        // taken in `start` is closed exactly once.
        unsafe { CloseHandle(self.thread as HANDLE) };
        let collected = joined.map_err(|_| SamplingError::SamplerPanicked)?;
        if let Some(error) = collected.failure {
            return Err(error);
        }
        Ok(collected)
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        if self.done.is_some() {
            let _ = self.stop();
        }
    }
}

/// Finds a live thread of this process by its description, excluding the caller.
fn find_thread(name: &str) -> Result<u32, SamplingError> {
    // SAFETY: a thread snapshot of the whole system is filtered by process id
    // below; the returned handle is owned by this wrapper.
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) });
    if snapshot.0 == INVALID_HANDLE_VALUE {
        return Err(SamplingError::ThreadDiscovery(last_error()));
    }
    // SAFETY: both calls only read the calling thread's and process' identity.
    let (pid, caller) = unsafe { (GetCurrentProcessId(), GetCurrentThreadId()) };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        cntUsage: 0,
        th32ThreadID: 0,
        th32OwnerProcessID: 0,
        tpBasePri: 0,
        tpDeltaPri: 0,
        dwFlags: 0,
    };
    let mut selected = None;
    let mut caller_matched = false;
    // SAFETY: `entry.dwSize` is set as the toolhelp API requires.
    let mut more = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid
            && let Some(description) = thread_description(entry.th32ThreadID)
            && description == name
        {
            if entry.th32ThreadID == caller {
                caller_matched = true;
            } else if selected.is_none() {
                selected = Some(entry.th32ThreadID);
            }
        }
        // SAFETY: the same initialized entry is reused for the next thread.
        more = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
    }
    if caller_matched {
        return Err(SamplingError::TargetIsCaller);
    }
    selected.ok_or_else(|| SamplingError::ThreadNotFound(name.to_owned()))
}

/// Reads one thread's description, or `None` when it has none.
fn thread_description(tid: u32) -> Option<String> {
    // SAFETY: only the right needed to read the description is requested.
    let handle = Handle(unsafe { OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, tid) });
    if handle.0.is_null() {
        return None;
    }
    let mut description = std::ptr::null_mut::<u16>();
    // SAFETY: GetThreadDescription allocates one wide string on success and
    // stores its pointer here; it is freed with LocalFree below.
    let result = unsafe { GetThreadDescription(handle.0, &mut description) };
    if result < 0 || description.is_null() {
        return None;
    }
    let text = wide_string(description);
    // SAFETY: the pointer came from GetThreadDescription, which allocates it
    // with LocalAlloc, and is read exactly once before being freed.
    unsafe { LocalFree(description.cast::<c_void>()) };
    text
}

/// Reads a NUL-terminated UTF-16 string, bounded so a missing terminator
/// cannot run away.
fn wide_string(pointer: *const u16) -> Option<String> {
    const MAX_CHARS: usize = 32_768;
    let mut length = 0;
    while length < MAX_CHARS {
        // SAFETY: the scan stops at the terminator or at MAX_CHARS.
        if unsafe { *pointer.add(length) } == 0 {
            break;
        }
        length += 1;
    }
    if length == 0 {
        return None;
    }
    // SAFETY: `length` characters were read from the same pointer.
    let chars = unsafe { std::slice::from_raw_parts(pointer, length) };
    Some(String::from_utf16_lossy(chars))
}

fn run(
    thread: isize,
    config: SampleConfig,
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicU64>,
) -> Collected {
    let started = Instant::now();
    let mut ring = Ring::new(config.max_samples);
    let mut failure = None;
    while !stop.load(Ordering::Acquire) {
        let tick = Instant::now();
        match sample_once(thread as HANDLE, config.max_stack_depth, started.elapsed()) {
            Ok(sample) => {
                ring.push(sample);
                taken.fetch_add(1, Ordering::Relaxed);
            }
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
        let spent = tick.elapsed();
        if spent < config.interval {
            thread::sleep(config.interval - spent);
        }
    }
    let duration = started.elapsed();
    let (samples, samples_dropped) = ring.finish();
    Collected {
        duration,
        samples_taken: taken.load(Ordering::Relaxed),
        samples_dropped,
        samples,
        failure,
    }
}

fn sample_once(
    thread: HANDLE,
    max_stack_depth: usize,
    offset: Duration,
) -> Result<RawSample, SamplingError> {
    // SAFETY: `thread` carries THREAD_SUSPEND_RESUME for a thread of this
    // process, and is never the calling thread.
    let previous = unsafe { SuspendThread(thread) };
    if previous == u32::MAX {
        return Err(SamplingError::TargetUnavailable(last_error()));
    }
    let mut suspension = Suspension {
        thread,
        suspended: true,
    };
    let registers = unwind::read_registers(thread)?;
    let bounds = unwind::stack_bounds(registers.sp);
    let mut frames = [0_u64; MAX_STACK_DEPTH];
    let depth = fp::unwind(
        registers,
        bounds,
        max_stack_depth,
        unwind::ADDRESS_MASK,
        &mut frames,
    );
    suspension.resume()?;
    Ok(RawSample {
        offset,
        depth: depth as u8,
        frames,
    })
}

/// Resumes the thread on every exit path, including a panic while unwinding.
struct Suspension {
    thread: HANDLE,
    suspended: bool,
}

impl Suspension {
    fn resume(&mut self) -> Result<(), SamplingError> {
        if !self.suspended {
            return Ok(());
        }
        // SAFETY: this balances one successful SuspendThread on `thread`.
        let result = unsafe { ResumeThread(self.thread) };
        if result != u32::MAX {
            self.suspended = false;
            Ok(())
        } else {
            Err(SamplingError::TargetUnavailable(last_error()))
        }
    }
}

impl Drop for Suspension {
    fn drop(&mut self) {
        if self.suspended {
            // SAFETY: the successful suspend in `sample_once` is still not
            // balanced; retry once and ignore a dead-target error.
            unsafe { ResumeThread(self.thread) };
        }
    }
}
