//! End-to-end sampler checks: a real busy thread is sampled and symbolized.
//!
//! The same assertions run on every backend. Where no sampler exists, the
//! platform-gap test covers the reported error instead and the sampling tests
//! return early rather than inventing a profile.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use cove_profiling::{
    CpuSampler, PlatformSupport, SampleConfig, SamplingError, SymbolizationError, Symbolizer,
    platform_support,
};

/// Linux and Android install one process-wide SIGPROF handler, so sampling
/// tests must not overlap on any platform.
static SAMPLER_LOCK: Mutex<()> = Mutex::new(());

fn sampler_lock() -> MutexGuard<'static, ()> {
    SAMPLER_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn supported() -> bool {
    matches!(platform_support(), PlatformSupport::Supported)
}

/// Kept out of line so a sampled instruction address lands inside it.
#[inline(never)]
fn cove_profiling_busy_loop(stop: &AtomicBool) {
    let mut accumulator: u64 = 0;
    while !stop.load(Ordering::Relaxed) {
        accumulator = accumulator.wrapping_add(1);
        std::hint::black_box(accumulator);
    }
}

/// Names stay within the 15 bytes the Linux kernel keeps in `comm`, so two
/// tests cannot collide on a truncated prefix.
fn start_busy_thread(name: &str) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || cove_profiling_busy_loop(&thread_stop))
        .expect("spawn busy thread");
    // Give the thread its name before the sampler enumerates threads.
    thread::sleep(Duration::from_millis(50));
    (stop, handle)
}

/// Source paths are recorded with the host's separator.
fn source_path(path: &str) -> String {
    path.replace('\\', "/")
}

mod sampled {
    use super::*;

    #[test]
    fn samples_a_busy_thread_and_resolves_a_known_symbol() {
        if !supported() {
            return;
        }
        let _guard = sampler_lock();
        let name = "cove-prof-busy";
        let (stop, busy) = start_busy_thread(name);
        let config = SampleConfig {
            interval: Duration::from_millis(1),
            max_samples: 1024,
            max_stack_depth: 32,
        };
        let sampler = CpuSampler::start(&name, config.clone()).expect("start sampler");
        thread::sleep(Duration::from_millis(400));
        assert!(
            sampler.samples_taken() > 0,
            "the sampler reported no live samples"
        );
        assert_eq!(sampler.thread_name(), name);
        assert!(sampler.elapsed() >= Duration::from_millis(350));
        let profile = sampler.stop().expect("stop sampler");
        stop.store(true, Ordering::Release);
        busy.join().expect("busy thread");

        let expected = profile.duration.as_secs_f64() / config.interval.as_secs_f64();
        let taken = profile.samples_taken as f64;
        assert!(
            taken >= expected * 0.25,
            "expected about {expected:.0} samples, got {taken:.0} in {:?}",
            profile.duration
        );
        assert!(
            taken <= expected * 2.5 + 8.0,
            "sampled far faster than requested: expected {expected:.0}, got {taken:.0}"
        );
        assert!(
            profile.samples.len() as u64 <= config.max_samples as u64,
            "ring exceeded its bound"
        );
        assert!(!profile.samples.is_empty(), "no samples captured");
        assert!(
            profile
                .samples
                .windows(2)
                .all(|pair| pair[0].offset <= pair[1].offset),
            "sample offsets are not chronological"
        );
        assert!(
            profile
                .samples
                .iter()
                .all(|sample| !sample.stack.is_empty()),
            "a captured sample has no stack frames"
        );

        let mut symbolizer = Symbolizer::new().expect("symbolizer");
        let symbolized = profile.symbolize(&mut symbolizer);
        let frames = symbolized
            .samples
            .iter()
            .flat_map(|sample| sample.frames.iter())
            .collect::<Vec<_>>();
        let resolved = frames
            .iter()
            .filter(|frame| {
                frame
                    .function
                    .as_deref()
                    .is_some_and(|name| name.contains("cove_profiling_busy_loop"))
            })
            .collect::<Vec<_>>();
        assert!(
            !resolved.is_empty(),
            "no frame resolved to the busy loop; first frames: {:?}",
            frames.iter().take(6).collect::<Vec<_>>()
        );
        assert!(
            resolved.iter().all(|frame| {
                frame.location.as_ref().is_some_and(|location| {
                    source_path(&location.file).ends_with("tests/cpu_sampler.rs")
                })
            }),
            "a busy-loop frame resolved outside its source file: {resolved:?}"
        );
        assert!(
            resolved.iter().any(|frame| frame
                .location
                .as_ref()
                .is_some_and(|location| location.line.is_some())),
            "no busy-loop frame carried a line number: {resolved:?}"
        );
        let labelled = resolved
            .iter()
            .find(|frame| {
                frame
                    .location
                    .as_ref()
                    .is_some_and(|location| location.line.is_some())
            })
            .expect("a busy-loop frame with a line");
        assert!(labelled.label().contains("cove_profiling_busy_loop"));
        assert!(labelled.label().contains("cpu_sampler.rs:"));
    }

    #[test]
    fn ring_buffer_is_bounded_and_keeps_the_newest_samples() {
        if !supported() {
            return;
        }
        let _guard = sampler_lock();
        let name = "cove-prof-ring";
        let (stop, busy) = start_busy_thread(name);
        let config = SampleConfig {
            interval: Duration::from_millis(1),
            max_samples: 16,
            max_stack_depth: 8,
        };
        let sampler = CpuSampler::start(&name, config.clone()).expect("start sampler");
        thread::sleep(Duration::from_millis(300));
        let profile = sampler.stop().expect("stop sampler");
        stop.store(true, Ordering::Release);
        busy.join().expect("busy thread");

        assert_eq!(profile.samples.len(), config.max_samples);
        assert!(
            profile.samples_taken > config.max_samples as u64,
            "run was too short to exercise overwriting"
        );
        assert_eq!(
            profile.samples_dropped,
            profile.samples_taken - config.max_samples as u64
        );
        assert!(
            profile
                .samples
                .windows(2)
                .all(|pair| pair[0].offset < pair[1].offset),
            "retained samples are not the newest ones in order"
        );
        assert!(profile.samples.iter().all(|sample| sample.stack.len() <= 8));
    }

    #[test]
    fn reports_unknown_thread_names_instead_of_inventing_a_profile() {
        if !supported() {
            return;
        }
        let _guard = sampler_lock();
        let error = CpuSampler::start("cove-prof-absent", SampleConfig::default());
        assert!(matches!(error, Err(SamplingError::ThreadNotFound(_))));
    }

    #[test]
    fn refuses_to_sample_the_calling_thread() {
        if !supported() {
            return;
        }
        let _guard = sampler_lock();
        let outcome = thread::Builder::new()
            .name("cove-prof-self".to_owned())
            .spawn(|| {
                // Let the name reach the kernel before the sampler looks for it.
                thread::sleep(Duration::from_millis(20));
                CpuSampler::start("cove-prof-self", SampleConfig::default()).map(|_| ())
            })
            .expect("spawn")
            .join()
            .expect("join");
        assert!(matches!(outcome, Err(SamplingError::TargetIsCaller)));
    }
}

#[test]
fn platform_support_matches_the_sampler_outcome() {
    match platform_support() {
        PlatformSupport::Supported => {
            let _guard = sampler_lock();
            assert!(matches!(
                CpuSampler::start("cove-prof-absent", SampleConfig::default()),
                Err(SamplingError::ThreadNotFound(_))
            ));
            assert!(Symbolizer::new().is_ok());
        }
        PlatformSupport::Unsupported { reason } => {
            assert!(!reason.is_empty());
            let error = match CpuSampler::start("main", SampleConfig::default()) {
                Ok(_) => panic!("a sampler was created on a platform without one"),
                Err(error) => error,
            };
            assert!(matches!(error, SamplingError::Unsupported { .. }));
            assert!(matches!(
                Symbolizer::new(),
                Err(SymbolizationError::Unsupported { .. })
            ));
        }
    }
}
