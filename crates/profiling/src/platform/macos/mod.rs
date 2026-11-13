//! macOS sampler: Mach thread state, frame-pointer unwinding, bounded ring.
//!
//! Threads are enumerated with `task_threads` on the current task's port —
//! never `task_for_pid` — and matched by the kernel thread name. A matched
//! send right is kept for `thread_suspend`/`thread_get_state`/`thread_resume`.

use std::ffi::CStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_port::mach_port_deallocate;
use mach2::mach_types::{thread_act_array_t, thread_act_t};
use mach2::message::mach_msg_type_number_t;
use mach2::port::mach_port_t;
use mach2::task::task_threads;
use mach2::thread_act::{thread_resume, thread_suspend};
use mach2::vm::mach_vm_deallocate;
use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};

use crate::config::{MAX_STACK_DEPTH, SampleConfig};
use crate::platform::ring::{RawSample, Ring};
use crate::sampler::Collected;
use crate::{PlatformSupport, SamplingError, SymbolizationError};

mod symbols;
mod unwind;

pub(crate) use symbols::Symbolizer;

use unwind::StackBounds;

unsafe extern "C" {
    /// Returns the current task's port name. Declared here because the libc
    /// binding for this symbol is deprecated.
    fn mach_task_self() -> mach_port_t;
}

pub(crate) fn support() -> PlatformSupport {
    PlatformSupport::Supported
}

pub(crate) fn symbolizer() -> Result<Symbolizer, SymbolizationError> {
    Ok(Symbolizer::new())
}

/// A running sampler thread and the target port it samples.
pub(crate) struct Sampler {
    port: mach_port_t,
    done: Option<JoinHandle<Collected>>,
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicU64>,
}

impl Sampler {
    pub(crate) fn start(thread_name: &str, config: &SampleConfig) -> Result<Self, SamplingError> {
        let target = find_thread(thread_name)?;
        let port = target.port;
        let pthread = target.pthread;
        let stop = Arc::new(AtomicBool::new(false));
        let taken = Arc::new(AtomicU64::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_taken = Arc::clone(&taken);
        let thread_config = config.clone();
        let handle = match thread::Builder::new()
            .name("cove-cpu-sampler".into())
            .spawn(move || run(port, pthread, thread_config, thread_stop, thread_taken))
        {
            Ok(handle) => handle,
            Err(error) => {
                // SAFETY: the target port selected by `find_thread` is a send
                // right owned by this task; the sampler thread never started.
                unsafe { mach_port_deallocate(task_port(), port) };
                return Err(SamplingError::ThreadSpawn(error.to_string()));
            }
        };
        Ok(Self {
            port,
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
        // SAFETY: the sampler thread has been joined, so the target port send
        // right taken in `start` is released exactly once.
        unsafe { mach_port_deallocate(task_port(), self.port) };
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

fn task_port() -> mach_port_t {
    // SAFETY: mach_task_self only reads a process-wide Mach port name.
    unsafe { mach_task_self() }
}

struct TargetThread {
    port: mach_port_t,
    pthread: libc::pthread_t,
}

/// Finds a live thread by its pthread name, without `task_for_pid`.
fn find_thread(name: &str) -> Result<TargetThread, SamplingError> {
    let task = task_port();
    let mut list: thread_act_array_t = std::ptr::null_mut();
    let mut count: mach_msg_type_number_t = 0;
    // SAFETY: the task port is the current task; the kernel fills `list` with
    // `count` newly referenced thread ports that this task must release.
    let result = unsafe { task_threads(task, &mut list, &mut count) };
    if result != KERN_SUCCESS {
        return Err(SamplingError::ThreadDiscovery(result));
    }
    // SAFETY: both calls only read the calling thread's handle.
    let caller_port = unsafe { libc::pthread_mach_thread_np(libc::pthread_self()) };

    let mut selected = None;
    let mut caller_matched = false;
    for index in 0..count as usize {
        // SAFETY: `list` holds `count` valid thread ports.
        let port = unsafe { *list.add(index) };
        if selected.is_none()
            && !caller_matched
            && let Some((thread_name, pthread)) = named_thread(port)
            && thread_name == name
        {
            if port == caller_port {
                caller_matched = true;
            } else {
                selected = Some((port, pthread));
                continue;
            }
        }
        // SAFETY: every enumerated port is a send right owned by this task.
        unsafe { mach_port_deallocate(task, port) };
    }

    if !list.is_null() && count > 0 {
        // SAFETY: `task_threads` allocated this array with vm_allocate in the
        // current task; the size is exactly `count` thread ports.
        unsafe {
            mach_vm_deallocate(
                task,
                list as mach_vm_address_t,
                mach_vm_size_t::from(count) * std::mem::size_of::<thread_act_t>() as mach_vm_size_t,
            );
        }
    }

    if caller_matched {
        return Err(SamplingError::TargetIsCaller);
    }
    selected
        .map(|(port, pthread)| TargetThread { port, pthread })
        .ok_or_else(|| SamplingError::ThreadNotFound(name.to_owned()))
}

/// Stack bounds read while the target is suspended and therefore alive.
fn stack_bounds(pthread: libc::pthread_t) -> Option<StackBounds> {
    // SAFETY: `pthread` was recorded for a Mach thread that the caller has
    // suspended, so its pthread metadata cannot be reclaimed concurrently.
    let (top, size) = unsafe {
        (
            libc::pthread_get_stackaddr_np(pthread),
            libc::pthread_get_stacksize_np(pthread),
        )
    };
    StackBounds::new(top as u64, size as u64)
}

/// Reads the pthread name of one Mach thread port through the kernel, so no
/// pthread metadata of another thread is dereferenced.
fn named_thread(port: mach_port_t) -> Option<(String, libc::pthread_t)> {
    let mut identifier = std::mem::MaybeUninit::<libc::thread_identifier_info>::zeroed();
    let mut count = libc::THREAD_IDENTIFIER_INFO_COUNT;
    // SAFETY: `port` names a live thread in this task; the kernel writes at
    // most one `thread_identifier_info` into the zeroed storage.
    let result = unsafe {
        libc::thread_info(
            port,
            libc::THREAD_IDENTIFIER_INFO as libc::thread_flavor_t,
            identifier.as_mut_ptr().cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if result != KERN_SUCCESS {
        return None;
    }
    // SAFETY: a successful `thread_info` initialized the identifier.
    let identifier = unsafe { identifier.assume_init() };

    let mut info = std::mem::MaybeUninit::<libc::thread_extended_info>::zeroed();
    let mut count = libc::THREAD_EXTENDED_INFO_COUNT;
    // SAFETY: the kernel copies the thread record into the correctly sized buffer.
    let result = unsafe {
        libc::thread_info(
            port,
            libc::THREAD_EXTENDED_INFO as libc::thread_flavor_t,
            info.as_mut_ptr().cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if result != KERN_SUCCESS || count != libc::THREAD_EXTENDED_INFO_COUNT {
        return None;
    }
    // SAFETY: a successful call filled the record, including the fixed-size name.
    let info = unsafe { info.assume_init() };
    let bytes = info.pth_name.map(|value| value as u8);
    let name = CStr::from_bytes_until_nul(&bytes)
        .ok()?
        .to_string_lossy()
        .into_owned();
    Some((name, identifier.thread_handle as libc::pthread_t))
}

fn run(
    port: mach_port_t,
    pthread: libc::pthread_t,
    config: SampleConfig,
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicU64>,
) -> Collected {
    let started = Instant::now();
    let mut ring = Ring::new(config.max_samples);
    let mut failure = None;
    while !stop.load(Ordering::Acquire) {
        let tick = Instant::now();
        match sample_once(port, pthread, config.max_stack_depth, started.elapsed()) {
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
    port: mach_port_t,
    pthread: libc::pthread_t,
    max_stack_depth: usize,
    offset: Duration,
) -> Result<RawSample, SamplingError> {
    // SAFETY: `port` is a send right to a thread in this task.
    let result = unsafe { thread_suspend(port) };
    if result != KERN_SUCCESS {
        return Err(SamplingError::TargetUnavailable(result));
    }
    let mut suspension = Suspension {
        port,
        suspended: true,
    };
    let stack = stack_bounds(pthread);
    let registers = unwind::read_registers(port)?;
    let mut frames = [0_u64; MAX_STACK_DEPTH];
    let depth = unwind::unwind(
        registers,
        stack,
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
    port: mach_port_t,
    suspended: bool,
}

impl Suspension {
    fn resume(&mut self) -> Result<(), SamplingError> {
        if !self.suspended {
            return Ok(());
        }
        // SAFETY: this balances one successful `thread_suspend` on `port`.
        let result = unsafe { thread_resume(self.port) };
        if result == KERN_SUCCESS {
            self.suspended = false;
            Ok(())
        } else {
            Err(SamplingError::TargetUnavailable(result))
        }
    }
}

impl Drop for Suspension {
    fn drop(&mut self) {
        if self.suspended {
            // SAFETY: the successful suspend in `sample_once` is still not
            // balanced; retry once and ignore a dead-target error.
            unsafe { thread_resume(self.port) };
        }
    }
}
