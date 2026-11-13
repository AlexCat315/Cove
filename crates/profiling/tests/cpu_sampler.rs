//! End-to-end sampler checks: a real busy thread is sampled and symbolized.

use cove_profiling::{CpuSampler, SampleConfig, Symbolizer};

#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;

/// Kept out of line so a sampled instruction address lands inside it.
#[cfg(target_os = "macos")]
#[inline(never)]
fn cove_profiling_busy_loop(stop: &AtomicBool) {
    let mut accumulator: u64 = 0;
    while !stop.load(Ordering::Relaxed) {
        accumulator = accumulator.wrapping_add(1);
        std::hint::black_box(accumulator);
    }
}

#[cfg(target_os = "macos")]
fn start_busy_thread(name: &str) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || cove_profiling_busy_loop(&thread_stop))
        .expect("spawn busy thread");
    // Give the pthread its name before the sampler enumerates threads.
    thread::sleep(Duration::from_millis(50));
    (stop, handle)
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    #[test]
    fn samples_a_busy_thread_and_resolves_a_known_symbol() {
        let name = format!("cove-profiling-test-{}", std::process::id());
        let (stop, busy) = start_busy_thread(&name);
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
        assert_eq!(sampler.thread_name(), name.as_str());
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
                frame
                    .location
                    .as_ref()
                    .is_some_and(|location| location.file.ends_with("tests/cpu_sampler.rs"))
            }),
            "a busy-loop frame resolved outside its source file: {resolved:?}"
        );
        assert!(
            resolved.iter().any(|frame| frame
                .location
                .as_ref()
                .is_some_and(|location| location.line.is_some())),
            "no busy-loop frame carried a DWARF line: {resolved:?}"
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
        let name = format!("cove-profiling-ring-{}", std::process::id());
        let (stop, busy) = start_busy_thread(&name);
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
        let error = CpuSampler::start(
            "cove-profiling-thread-that-does-not-exist",
            SampleConfig::default(),
        );
        assert!(matches!(
            error,
            Err(cove_profiling::SamplingError::ThreadNotFound(_))
        ));
    }
}

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use super::*;

    #[test]
    fn sampling_and_symbolization_report_the_platform_gap() {
        assert!(matches!(
            cove_profiling::platform_support(),
            cove_profiling::PlatformSupport::Unsupported { reason } if !reason.is_empty()
        ));
        let error = match CpuSampler::start("main", SampleConfig::default()) {
            Ok(_) => panic!("a sampler was created on a platform without one"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            cove_profiling::SamplingError::Unsupported { .. }
        ));
        assert!(matches!(
            Symbolizer::new(),
            Err(cove_profiling::SymbolizationError::Unsupported { .. })
        ));
    }
}
