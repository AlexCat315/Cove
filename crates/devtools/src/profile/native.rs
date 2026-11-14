//! The native UI-thread profiler behind `profile_start`/`profile_stop`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cove_devtools_protocol::control::Fault;
use cove_devtools_protocol::{
    ProfileFrame, ProfileHotspot, ProfileSource, ProfileStack, ProfileStatus, UiHotspots,
    UiProfile,
};
use cove_profiling::{
    CpuSampler, MAX_SAMPLES, PlatformSupport, SampleConfig, SamplingError, SymbolizedFrame,
    SymbolizedProfile, Symbolizer, platform_support,
};
use serde_json::{Value, json};

/// Longest sampling run the probe accepts; the worker stops itself at the bound.
pub(crate) const MAX_DURATION_MS: u32 = 10_000;
/// Highest rate the wire accepts.
pub(crate) const MAX_HZ: u32 = 1_000;
pub(crate) use super::MAX_HOTSPOTS;
/// Aggregated stacks in one profile answer.
const MAX_STACKS: usize = 24;
/// Frames recorded per sample.
const STACK_DEPTH: usize = 32;
/// How often the worker publishes live counters and checks the stop flag.
const POLL: Duration = Duration::from_millis(4);

/// `None` where a real sampler exists, otherwise the reason to report.
pub(crate) fn unavailable_reason() -> Option<&'static str> {
    match platform_support() {
        PlatformSupport::Supported => None,
        PlatformSupport::Unsupported { reason } => Some(reason),
    }
}

fn invalid(message: String) -> Fault {
    Fault::new("invalid_request", message)
}

fn sampling_fault(error: &SamplingError) -> Fault {
    match error {
        SamplingError::Unsupported { .. } => Fault::new("unavailable", error.to_string()),
        _ => Fault::new("profile_failed", error.to_string()),
    }
}

fn percent(samples: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        samples as f64 / total as f64 * 100.0
    }
}

/// One resolved frame aggregated across the retained samples.
struct Entry {
    label: String,
    function: Option<String>,
    file_path: Option<String>,
    line: Option<u32>,
    self_samples: u64,
    inclusive_samples: u64,
    self_ms: f64,
    inclusive_ms: f64,
}

impl Entry {
    fn new(label: &str, frame: &SymbolizedFrame) -> Self {
        Self {
            label: label.to_owned(),
            function: frame.function.clone(),
            file_path: frame
                .location
                .as_ref()
                .map(|location| location.file.clone()),
            line: frame.location.as_ref().and_then(|location| location.line),
            self_samples: 0,
            inclusive_samples: 0,
            self_ms: 0.0,
            inclusive_ms: 0.0,
        }
    }

    fn hotspot(&self, retained_samples: u64) -> ProfileHotspot {
        ProfileHotspot {
            label: self.label.clone(),
            function: self.function.clone(),
            file_path: self.file_path.clone(),
            line: self.line,
            self_samples: self.self_samples,
            inclusive_samples: self.inclusive_samples,
            self_ms: self.self_ms,
            inclusive_ms: self.inclusive_ms,
            self_percent: percent(self.self_samples, retained_samples),
            inclusive_percent: percent(self.inclusive_samples, retained_samples),
        }
    }
}

/// Identical frame sequences collapsed into one stack with a sample count.
struct StackGroup {
    labels: Vec<String>,
    frames: Vec<ProfileFrame>,
    samples: u64,
}

/// A finished, symbolized run: the only data a read or hotspot answer exposes.
struct Report {
    thread: String,
    hz: u32,
    duration_ms: f64,
    sample_interval_ms: f64,
    samples_taken: u64,
    samples_dropped: u64,
    retained_samples: u64,
    samples_per_second: Option<f64>,
    entries: Vec<Entry>,
    stacks: Vec<StackGroup>,
    stack_count: usize,
}

impl Report {
    fn empty(thread: String, hz: u32) -> Self {
        Self {
            thread,
            hz,
            duration_ms: 0.0,
            sample_interval_ms: if hz > 0 { 1000.0 / f64::from(hz) } else { 0.0 },
            samples_taken: 0,
            samples_dropped: 0,
            retained_samples: 0,
            samples_per_second: None,
            entries: Vec::new(),
            stacks: Vec::new(),
            stack_count: 0,
        }
    }

    fn build(profile: SymbolizedProfile, hz: u32) -> Self {
        let retained_samples = profile.samples.len() as u64;
        let sample_interval_ms = profile.interval.as_secs_f64() * 1000.0;
        let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
        let mut stacks: BTreeMap<Vec<String>, StackGroup> = BTreeMap::new();
        for (index, sample) in profile.samples.iter().enumerate() {
            // Each sample owns the gap until the next one, so reported times
            // follow the sampler's real cadence instead of its nominal rate.
            let next = profile
                .samples
                .get(index + 1)
                .map_or(profile.duration, |next| next.offset);
            let weight_ms = next.saturating_sub(sample.offset).as_secs_f64() * 1000.0;
            let mut included = BTreeSet::new();
            let mut labels = Vec::with_capacity(sample.frames.len());
            let mut frames = Vec::with_capacity(sample.frames.len());
            for (depth, frame) in sample.frames.iter().enumerate() {
                let label = frame.label();
                let entry = entries
                    .entry(label.clone())
                    .or_insert_with(|| Entry::new(&label, frame));
                if depth == 0 {
                    entry.self_samples += 1;
                    entry.self_ms += weight_ms;
                }
                // Recursion repeats a frame in one stack; inclusive time counts
                // that frame once for the sample, not once per call.
                if included.insert(label.clone()) {
                    entry.inclusive_samples += 1;
                    entry.inclusive_ms += weight_ms;
                }
                frames.push(ProfileFrame {
                    label: label.clone(),
                    function: frame.function.clone(),
                    file_path: frame
                        .location
                        .as_ref()
                        .map(|location| location.file.clone()),
                    line: frame.location.as_ref().and_then(|location| location.line),
                });
                labels.push(label);
            }
            if labels.is_empty() {
                continue;
            }
            stacks
                .entry(labels.clone())
                .or_insert_with(|| StackGroup {
                    labels,
                    frames,
                    samples: 0,
                })
                .samples += 1;
        }
        let stack_count = stacks.len();
        let mut stacks: Vec<StackGroup> = stacks.into_values().collect();
        stacks.sort_by(|left, right| {
            right
                .samples
                .cmp(&left.samples)
                .then_with(|| left.labels.cmp(&right.labels))
        });
        stacks.truncate(MAX_STACKS);
        let mut entries: Vec<Entry> = entries.into_values().collect();
        entries.sort_by(|left, right| {
            right
                .self_samples
                .cmp(&left.self_samples)
                .then_with(|| right.inclusive_samples.cmp(&left.inclusive_samples))
                .then_with(|| left.label.cmp(&right.label))
        });
        let seconds = profile.duration.as_secs_f64();
        Self {
            thread: profile.thread_name,
            hz,
            duration_ms: seconds * 1000.0,
            sample_interval_ms,
            samples_taken: profile.samples_taken,
            samples_dropped: profile.samples_dropped,
            retained_samples,
            samples_per_second: (seconds > 0.0).then(|| profile.samples_taken as f64 / seconds),
            entries,
            stacks,
            stack_count,
        }
    }

    fn summary(&self, status: ProfileStatus) -> UiProfile {
        UiProfile {
            status,
            source: ProfileSource::Sampler,
            thread: self.thread.clone(),
            hz: self.hz,
            duration_ms: self.duration_ms,
            sample_interval_ms: self.sample_interval_ms,
            samples_taken: self.samples_taken,
            samples_dropped: self.samples_dropped,
            retained_samples: self.retained_samples,
            samples_per_second: self.samples_per_second,
            stacks_truncated: self.stack_count > self.stacks.len(),
            stacks: self
                .stacks
                .iter()
                .map(|group| ProfileStack {
                    samples: group.samples,
                    percent: percent(group.samples, self.retained_samples),
                    frames: group.frames.clone(),
                })
                .collect(),
        }
    }

    fn hotspots(&self, status: ProfileStatus, limit: usize) -> UiHotspots {
        let limit = limit.clamp(1, MAX_HOTSPOTS);
        UiHotspots {
            status,
            source: ProfileSource::Sampler,
            thread: self.thread.clone(),
            hz: self.hz,
            duration_ms: self.duration_ms,
            sample_interval_ms: self.sample_interval_ms,
            samples_taken: self.samples_taken,
            samples_dropped: self.samples_dropped,
            retained_samples: self.retained_samples,
            truncated: self.entries.len() > limit,
            hotspots: self
                .entries
                .iter()
                .take(limit)
                .map(|entry| entry.hotspot(self.retained_samples))
                .collect(),
        }
    }
}

enum State {
    Idle,
    Ready(Report),
    Failed(Fault),
}

/// Samples one named thread and keeps the last symbolized profile.
///
/// The probe constructs this on the UI thread with that thread's name. `start`
/// spawns the worker, which is what calls [`CpuSampler::start`] — the sampler
/// rejects sampling its own caller. Typed methods return wire payloads; an
/// unavailable platform fails every command instead of returning samples.
pub(crate) struct UiProfiler {
    thread: String,
    state: Arc<Mutex<State>>,
    live: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    started: Option<Instant>,
    hz: u32,
}

impl UiProfiler {
    pub(crate) fn new(thread: String) -> Self {
        Self {
            thread: if thread.is_empty() {
                "main".to_owned()
            } else {
                thread
            },
            state: Arc::new(Mutex::new(State::Idle)),
            live: Arc::new(AtomicU64::new(0)),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
            started: None,
            hz: 1,
        }
    }

    /// The UI thread name the sampler follows.
    pub(crate) fn thread_name(&self) -> &str {
        &self.thread
    }

    pub(crate) fn start(&mut self, hz: u32, duration_ms: u32) -> Result<Value, Fault> {
        if !(1..=MAX_HZ).contains(&hz) {
            return Err(invalid(format!("hz must be between 1 and {MAX_HZ}.")));
        }
        if !(1..=MAX_DURATION_MS).contains(&duration_ms) {
            return Err(invalid(format!(
                "duration_ms must be between 1 and {MAX_DURATION_MS}."
            )));
        }
        if let Some(reason) = unavailable_reason() {
            return Err(Fault::new("unavailable", reason));
        }
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            return Err(Fault::new(
                "busy",
                "A UI profile is already sampling; stop it first.",
            ));
        }
        let _ = self.join();
        *self.state.lock().unwrap() = State::Idle;
        self.live.store(0, Ordering::Relaxed);
        self.started = None;

        let interval = Duration::from_secs_f64(1.0 / f64::from(hz));
        let max_samples = (u64::from(duration_ms) * u64::from(hz) / 1000 + 16) as usize;
        let config = SampleConfig {
            interval,
            max_samples: max_samples.clamp(16, MAX_SAMPLES),
            max_stack_depth: STACK_DEPTH,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let live = Arc::new(AtomicU64::new(0));
        let state = Arc::clone(&self.state);
        let thread_name = self.thread.clone();
        let worker_stop = Arc::clone(&stop);
        let worker_live = Arc::clone(&live);
        let duration = Duration::from_millis(u64::from(duration_ms));
        let handle = thread::Builder::new()
            .name("cove.devtools.profile".into())
            .spawn(move || {
                run(
                    thread_name,
                    hz,
                    config,
                    duration,
                    state,
                    worker_stop,
                    worker_live,
                )
            })
            .map_err(|error| {
                Fault::new(
                    "profile_failed",
                    format!("The profiler thread could not start: {error}"),
                )
            })?;
        self.stop = stop;
        self.live = live;
        self.worker = Some(handle);
        self.started = Some(Instant::now());
        self.hz = hz;
        Ok(json!(self.running()))
    }

    pub(crate) fn stop(&mut self) -> Result<Value, Fault> {
        if let Some(reason) = unavailable_reason() {
            return Err(Fault::new("unavailable", reason));
        }
        if self.worker.is_none() {
            return Err(Fault::new(
                "no_profile",
                "Start a UI profile before stopping one.",
            ));
        }
        self.join()?;
        let profile = {
            let state = self.state.lock().unwrap();
            match &*state {
                State::Ready(report) => report.summary(ProfileStatus::Ready),
                State::Failed(fault) => return Err(fault.clone()),
                State::Idle => {
                    return Err(Fault::new(
                        "profile_failed",
                        "The profiler thread ended without a profile.",
                    ));
                }
            }
        };
        self.started = None;
        Ok(json!(profile))
    }

    pub(crate) fn read(&self) -> Result<Value, Fault> {
        if let Some(reason) = unavailable_reason() {
            return Err(Fault::new("unavailable", reason));
        }
        let profile = {
            let state = self.state.lock().unwrap();
            match &*state {
                State::Ready(report) => report.summary(ProfileStatus::Ready),
                State::Failed(fault) => return Err(fault.clone()),
                State::Idle => match self.worker.as_ref() {
                    Some(worker) if !worker.is_finished() => self.running(),
                    Some(_) => {
                        return Err(Fault::new(
                            "profile_failed",
                            "The profiler thread ended without a profile.",
                        ));
                    }
                    None => {
                        Report::empty(self.thread.clone(), self.hz).summary(ProfileStatus::Idle)
                    }
                },
            }
        };
        Ok(json!(profile))
    }

    pub(crate) fn hotspots(&self, limit: usize) -> Result<Value, Fault> {
        if !(1..=MAX_HOTSPOTS).contains(&limit) {
            return Err(invalid(format!(
                "limit must be between 1 and {MAX_HOTSPOTS}."
            )));
        }
        if let Some(reason) = unavailable_reason() {
            return Err(Fault::new("unavailable", reason));
        }
        let result = {
            let state = self.state.lock().unwrap();
            match &*state {
                State::Ready(report) => report.hotspots(ProfileStatus::Ready, limit),
                State::Failed(fault) => return Err(fault.clone()),
                State::Idle => match self.worker.as_ref() {
                    Some(worker) if !worker.is_finished() => self.running_hotspots(),
                    Some(_) => {
                        return Err(Fault::new(
                            "profile_failed",
                            "The profiler thread ended without a profile.",
                        ));
                    }
                    None => UiHotspots {
                        status: ProfileStatus::Idle,
                        source: ProfileSource::Sampler,
                        thread: self.thread.clone(),
                        hz: self.hz,
                        duration_ms: 0.0,
                        sample_interval_ms: if self.hz > 0 {
                            1000.0 / f64::from(self.hz)
                        } else {
                            0.0
                        },
                        samples_taken: 0,
                        samples_dropped: 0,
                        retained_samples: 0,
                        truncated: false,
                        hotspots: Vec::new(),
                    },
                },
            }
        };
        Ok(json!(result))
    }

    /// Stops a live run and forgets it; used when the probe disconnects.
    pub(crate) fn reset(&mut self) {
        let _ = self.join();
        *self.state.lock().unwrap() = State::Idle;
        self.live.store(0, Ordering::Relaxed);
        self.started = None;
    }

    fn running(&self) -> UiProfile {
        let elapsed_ms = self
            .started
            .map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0);
        let samples_taken = self.live.load(Ordering::Relaxed);
        UiProfile {
            status: ProfileStatus::Running,
            source: ProfileSource::Sampler,
            thread: self.thread.clone(),
            hz: self.hz,
            duration_ms: elapsed_ms,
            sample_interval_ms: if self.hz > 0 {
                1000.0 / f64::from(self.hz)
            } else {
                0.0
            },
            samples_taken,
            samples_dropped: 0,
            retained_samples: 0,
            samples_per_second: (elapsed_ms > 0.0)
                .then(|| samples_taken as f64 / (elapsed_ms / 1000.0)),
            stacks_truncated: false,
            stacks: Vec::new(),
        }
    }

    fn running_hotspots(&self) -> UiHotspots {
        let elapsed_ms = self
            .started
            .map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0);
        UiHotspots {
            status: ProfileStatus::Running,
            source: ProfileSource::Sampler,
            thread: self.thread.clone(),
            hz: self.hz,
            duration_ms: elapsed_ms,
            sample_interval_ms: if self.hz > 0 {
                1000.0 / f64::from(self.hz)
            } else {
                0.0
            },
            samples_taken: self.live.load(Ordering::Relaxed),
            samples_dropped: 0,
            retained_samples: 0,
            truncated: false,
            hotspots: Vec::new(),
        }
    }

    fn join(&mut self) -> Result<(), Fault> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        self.stop.store(true, Ordering::Release);
        worker.join().map_err(|_| {
            Fault::new(
                "profile_failed",
                "The profiler thread stopped unexpectedly.",
            )
        })
    }
}

impl Drop for UiProfiler {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

fn run(
    thread_name: String,
    hz: u32,
    config: SampleConfig,
    duration: Duration,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    live: Arc<AtomicU64>,
) {
    let sampler = match CpuSampler::start(&thread_name, config) {
        Ok(sampler) => sampler,
        Err(error) => {
            *state.lock().unwrap() = State::Failed(sampling_fault(&error));
            return;
        }
    };
    let deadline = Instant::now() + duration;
    while !stop.load(Ordering::Acquire) {
        live.store(sampler.samples_taken(), Ordering::Relaxed);
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        thread::sleep(POLL.min(deadline - now));
    }
    let profile = match sampler.stop() {
        Ok(profile) => profile,
        Err(error) => {
            *state.lock().unwrap() = State::Failed(sampling_fault(&error));
            return;
        }
    };
    live.store(profile.samples_taken, Ordering::Relaxed);
    let mut symbolizer = match Symbolizer::new() {
        Ok(symbolizer) => symbolizer,
        Err(error) => {
            *state.lock().unwrap() = State::Failed(Fault::new("unavailable", error.to_string()));
            return;
        }
    };
    let symbolized = profile.symbolize(&mut symbolizer);
    *state.lock().unwrap() = State::Ready(Report::build(symbolized, hz));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[inline(never)]
    fn cove_devtools_profile_busy_loop(stop: &AtomicBool) {
        let mut accumulator: u64 = 0;
        while !stop.load(Ordering::Relaxed) {
            accumulator = accumulator.wrapping_add(1);
            std::hint::black_box(accumulator);
        }
    }

    #[cfg(target_os = "macos")]
    fn wait_until_ready(profiler: &UiProfiler) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let profile = profiler.read().expect("a running or ready profile");
            match profile["status"].as_str() {
                Some("ready") => return profile,
                Some("running") => {}
                other => panic!("unexpected profile status {other:?}: {profile}"),
            }
            assert!(
                Instant::now() < deadline,
                "the sampler never published a profile"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_real_sampler_run_resolves_the_busy_threads_function_and_line() {
        let name = format!("cove-profile-test-{}", std::process::id());
        let stop = Arc::new(AtomicBool::new(false));
        let busy_stop = Arc::clone(&stop);
        let busy = thread::Builder::new()
            .name(name.clone())
            .spawn(move || cove_devtools_profile_busy_loop(&busy_stop))
            .expect("spawn busy thread");
        // Give the pthread its name before the sampler enumerates threads.
        thread::sleep(Duration::from_millis(50));

        let mut profiler = UiProfiler::new(name.clone());
        assert_eq!(profiler.thread_name(), name.as_str());
        profiler.start(500, 500).expect("start sampling");

        let profile = wait_until_ready(&profiler);
        stop.store(true, Ordering::Release);
        busy.join().expect("busy thread");

        assert_eq!(profile["thread"], name.as_str());
        assert_eq!(profile["hz"], 500);
        let samples = profile["samplesTaken"].as_u64().unwrap();
        assert!(samples > 0, "no live samples were taken: {profile}");
        let retained = profile["retainedSamples"].as_u64().unwrap();
        assert!(retained > 0, "no samples were retained: {profile}");
        assert!(retained <= samples, "retained more samples than were taken");

        let hotspots = profiler.hotspots(MAX_HOTSPOTS).expect("hotspots");
        let entries = hotspots["hotspots"].as_array().unwrap();
        let busy: Vec<&Value> = entries
            .iter()
            .filter(|entry| {
                entry["function"]
                    .as_str()
                    .is_some_and(|function| function.contains("cove_devtools_profile_busy_loop"))
            })
            .collect();
        assert!(
            !busy.is_empty(),
            "the busy loop is absent from the hotspots: {entries:?}"
        );
        assert!(
            busy.iter().all(|entry| entry["filePath"]
                .as_str()
                .is_some_and(|path| path.ends_with("profile/native.rs"))),
            "the busy loop resolved outside its source file: {busy:?}"
        );
        // One sampled instruction of a function can sit outside the line table;
        // at least one of the busy loop's addresses must carry function file:line.
        let with_line = busy
            .iter()
            .find(|entry| entry["line"].as_u64().is_some())
            .unwrap_or_else(|| panic!("the busy loop has no DWARF line: {busy:#?}"));
        assert!(
            with_line["label"]
                .as_str()
                .is_some_and(|label| label.contains("profile/native.rs:")),
            "the label does not carry function file:line: {with_line}"
        );
        assert!(
            busy.iter()
                .any(|entry| entry["selfSamples"].as_u64().unwrap() > 0),
            "no sampled instruction landed in the busy loop"
        );

        let stopped = profiler.stop().expect("stop sampling");
        assert_eq!(stopped["status"], "ready");
        assert_eq!(stopped["samplesTaken"], profile["samplesTaken"]);
        assert_eq!(
            profiler.stop().unwrap_err().code,
            "no_profile",
            "a second stop has no active run"
        );
        assert_eq!(
            profiler.read().unwrap()["status"],
            "ready",
            "the last profile stays readable after stop"
        );
        profiler.reset();
        assert_eq!(profiler.read().unwrap()["status"], "idle");
    }

    #[test]
    fn initial_profile_state_matches_platform_support() {
        let mut profiler = UiProfiler::new("main".into());
        let Some(reason) = unavailable_reason() else {
            assert_eq!(profiler.read().unwrap()["status"], "idle");
            return;
        };
        let fault = profiler.start(100, 100).unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert_eq!(fault.message, reason);
        assert_eq!(profiler.stop().unwrap_err().code, "unavailable");
        assert_eq!(profiler.read().unwrap_err().code, "unavailable");
        assert_eq!(
            profiler.hotspots(10).unwrap_err().code,
            "unavailable",
            "no hotspot is invented without samples"
        );
    }

    #[test]
    fn request_bounds_are_rejected_before_sampling_starts() {
        let mut profiler = UiProfiler::new("main".into());
        for (hz, duration_ms) in [
            (0, 100),
            (MAX_HZ + 1, 100),
            (100, 0),
            (100, MAX_DURATION_MS + 1),
        ] {
            let fault = profiler.start(hz, duration_ms).unwrap_err();
            assert_eq!(fault.code, "invalid_request", "{hz} Hz / {duration_ms} ms");
        }
        assert_eq!(profiler.hotspots(0).unwrap_err().code, "invalid_request");
        assert_eq!(
            profiler.hotspots(MAX_HOTSPOTS + 1).unwrap_err().code,
            "invalid_request"
        );
        assert_eq!(profiler.thread_name(), "main");
    }
}
