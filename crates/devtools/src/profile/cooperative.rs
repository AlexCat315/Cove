//! Cooperative phase timing for runtimes that cannot be sampled.
//!
//! A browser runtime runs WebAssembly on the page's only thread: nothing can
//! suspend it at an arbitrary instruction, so a stack sample cannot be taken
//! from the outside. What *can* be measured is where a frame's CPU time goes,
//! because the runtime already times its own phases while it renders. This
//! module accumulates those measured durations into a profile-shaped answer:
//!
//! - one root stack `frame`, plus one stack `frame → phase` per recorded phase;
//! - every millisecond is the duration the runtime measured around that phase,
//!   never an estimate, and `source` is reported as
//!   [`ProfileSource::Instrumented`] so a reader can tell the difference;
//! - `samples` count measured milliseconds (one unit = one millisecond), so a
//!   stack's unit count is its share of the measured frame time and the tree's
//!   units add up to the frame total instead of over-counting;
//! - `hz` and `sample_interval_ms` stay `0` because there is no sampling rate.
//!
//! The recorder is a thread-local because a browser runtime is single-threaded.
//! Recording is cheap and silent unless a run is active, so the render path can
//! call [`record_phase`] unconditionally.

use std::cell::RefCell;
use std::time::Instant;

use cove_devtools_protocol::control::Fault;
use cove_devtools_protocol::{
    ProfileFrame, ProfileHotspot, ProfileSource, ProfileStack, ProfileStatus, UiHotspots, UiProfile,
};

/// Distinct phases one run keeps; further labels are counted as dropped.
pub(crate) const MAX_DISTINCT_PHASES: usize = 12;
/// Longest cooperative run the probe accepts, matching the sampler's bound.
pub(crate) const MAX_DURATION_MS: u32 = 10_000;
/// Most stacks one answer carries.
const MAX_STACKS: usize = 24;

#[derive(Clone, Copy)]
struct PhaseRow {
    label: &'static str,
    self_ms: f64,
}

struct Recorder {
    status: ProfileStatus,
    started: Option<Instant>,
    duration_ms: u32,
    finished_ms: Option<f64>,
    frames: u64,
    total_ms: f64,
    pending_ms: f64,
    frame_self_ms: f64,
    dropped: u64,
    rows: Vec<PhaseRow>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            status: ProfileStatus::Idle,
            started: None,
            duration_ms: 0,
            finished_ms: None,
            frames: 0,
            total_ms: 0.0,
            pending_ms: 0.0,
            frame_self_ms: 0.0,
            dropped: 0,
            rows: Vec::with_capacity(MAX_DISTINCT_PHASES),
        }
    }
}

impl Recorder {
    fn clear_run(&mut self) {
        self.started = None;
        self.finished_ms = None;
        self.frames = 0;
        self.total_ms = 0.0;
        self.pending_ms = 0.0;
        self.frame_self_ms = 0.0;
        self.dropped = 0;
        self.rows.clear();
    }

    fn begin(&mut self, duration_ms: u32) {
        self.clear_run();
        self.status = ProfileStatus::Running;
        self.started = Some(Instant::now());
        self.duration_ms = duration_ms;
    }

    fn elapsed_ms(&self) -> f64 {
        if let Some(finished) = self.finished_ms {
            return finished;
        }
        self.started
            .map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0)
    }

    fn finish(&mut self) {
        if self.status != ProfileStatus::Running {
            return;
        }
        self.finished_ms = Some(self.elapsed_ms());
        self.status = ProfileStatus::Ready;
    }

    /// Ends the run once its duration has passed. A browser renders on demand,
    /// so a quiet page may produce no further frame: the bound must not depend
    /// on a frame arriving.
    fn finish_if_expired(&mut self) {
        if self.status == ProfileStatus::Running
            && self.duration_ms > 0
            && self.elapsed_ms() >= f64::from(self.duration_ms)
        {
            self.finish();
        }
    }

    fn record_phase(&mut self, label: &'static str, ms: f64) {
        if self.status != ProfileStatus::Running || !(ms.is_finite() && ms > 0.0) {
            return;
        }
        self.pending_ms += ms;
        if let Some(row) = self.rows.iter_mut().find(|row| row.label == label) {
            row.self_ms += ms;
            return;
        }
        if self.rows.len() < MAX_DISTINCT_PHASES {
            self.rows.push(PhaseRow { label, self_ms: ms });
        } else {
            // The frame still accounts for the time; it just has no row of its own.
            self.dropped += 1;
        }
    }

    fn record_frame(&mut self, total_ms: f64) {
        if self.status != ProfileStatus::Running || !(total_ms.is_finite() && total_ms > 0.0) {
            return;
        }
        self.frames += 1;
        self.total_ms += total_ms;
        self.frame_self_ms += (total_ms - self.pending_ms).max(0.0);
        self.pending_ms = 0.0;
        if self.duration_ms > 0 && self.elapsed_ms() >= f64::from(self.duration_ms) {
            self.finish();
        }
    }

    fn phase_share(&self, ms: f64) -> f64 {
        if self.total_ms <= 0.0 {
            0.0
        } else {
            (ms / self.total_ms * 100.0).clamp(0.0, 100.0)
        }
    }

    /// One unit is one measured millisecond, so the unit counts of a frame's
    /// self time and its phases partition the frame total instead of
    /// over-counting it the way one unit per occurrence would.
    fn units(&self) -> (u64, Vec<u64>) {
        let total = self.total_ms.round().max(0.0) as u64;
        let phases: Vec<u64> = self
            .rows
            .iter()
            .map(|row| row.self_ms.round().max(0.0) as u64)
            .collect();
        let assigned: u64 = phases.iter().sum();
        (total.saturating_sub(assigned), phases)
    }

    fn unit_share(&self, units: u64, total: u64) -> f64 {
        if total == 0 {
            0.0
        } else {
            (units as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
        }
    }

    fn summary(&self, thread: &str, status: ProfileStatus) -> UiProfile {
        let elapsed_ms = self.elapsed_ms();
        let mut stacks = Vec::new();
        let (self_units, phase_units) = self.units();
        let total_units: u64 = self_units + phase_units.iter().sum::<u64>();
        if self.frames > 0 {
            stacks.push(ProfileStack {
                samples: self_units,
                percent: self.unit_share(self_units, total_units),
                frames: vec![frame_node("frame", "Cove frame")],
            });
            for (row, units) in self
                .rows
                .iter()
                .zip(phase_units.iter())
                .take(MAX_STACKS - 1)
            {
                stacks.push(ProfileStack {
                    samples: *units,
                    percent: self.unit_share(*units, total_units),
                    frames: vec![
                        frame_node("frame", "Cove frame"),
                        frame_node(row.label, row.label),
                    ],
                });
            }
        }
        UiProfile {
            status,
            source: ProfileSource::Instrumented,
            thread: thread.to_owned(),
            hz: 0,
            duration_ms: elapsed_ms,
            sample_interval_ms: 0.0,
            samples_taken: self.frames,
            samples_dropped: self.dropped,
            retained_samples: total_units,
            samples_per_second: (elapsed_ms > 0.0)
                .then(|| self.frames as f64 / (elapsed_ms / 1000.0)),
            stacks_truncated: self.rows.len() > MAX_STACKS - 1 || self.dropped > 0,
            stacks,
        }
    }

    fn hotspots(&self, thread: &str, limit: usize) -> UiHotspots {
        let limit = limit.clamp(1, crate::profile::MAX_HOTSPOTS);
        let (self_units, phase_units) = self.units();
        let total_units: u64 = self_units + phase_units.iter().sum::<u64>();
        let mut entries: Vec<ProfileHotspot> = Vec::with_capacity(self.rows.len() + 1);
        entries.push(ProfileHotspot {
            label: "frame".to_owned(),
            function: Some("Cove frame".to_owned()),
            file_path: None,
            line: None,
            self_samples: self_units,
            inclusive_samples: total_units,
            self_ms: self.frame_self_ms,
            inclusive_ms: self.total_ms,
            self_percent: self.phase_share(self.frame_self_ms),
            inclusive_percent: if self.total_ms > 0.0 { 100.0 } else { 0.0 },
        });
        for (row, units) in self.rows.iter().zip(phase_units.iter()) {
            entries.push(ProfileHotspot {
                label: row.label.to_owned(),
                function: Some(row.label.to_owned()),
                file_path: None,
                line: None,
                self_samples: *units,
                inclusive_samples: *units,
                self_ms: row.self_ms,
                inclusive_ms: row.self_ms,
                self_percent: self.phase_share(row.self_ms),
                inclusive_percent: self.phase_share(row.self_ms),
            });
        }
        entries.sort_by(|left, right| {
            right
                .inclusive_ms
                .total_cmp(&left.inclusive_ms)
                .then_with(|| right.self_ms.total_cmp(&left.self_ms))
        });
        UiHotspots {
            status: self.status,
            source: ProfileSource::Instrumented,
            thread: thread.to_owned(),
            hz: 0,
            duration_ms: self.elapsed_ms(),
            sample_interval_ms: 0.0,
            samples_taken: self.frames,
            samples_dropped: self.dropped,
            retained_samples: total_units,
            truncated: entries.len() > limit,
            hotspots: entries.into_iter().take(limit).collect(),
        }
    }
}

fn frame_node(label: &str, function: &str) -> ProfileFrame {
    ProfileFrame {
        label: label.to_owned(),
        function: Some(function.to_owned()),
        file_path: None,
        line: None,
    }
}

thread_local! {
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder::default());
}

fn with<R>(f: impl FnOnce(&mut Recorder) -> R) -> R {
    RECORDER.with(|recorder| f(&mut recorder.borrow_mut()))
}

/// Whether a cooperative run is recording; the render path checks it before
/// spending anything on a phase it would only discard.
pub fn is_recording() -> bool {
    with(|recorder| recorder.status == ProfileStatus::Running)
}

/// Records one measured phase of the current frame. `label` must be a static
/// name so the profile stays allocation-free on the render path.
pub fn record_phase(label: &'static str, ms: f64) {
    with(|recorder| recorder.record_phase(label, ms));
}

/// Closes a frame with its measured total. The difference between the total and
/// the phases recorded since the previous frame becomes the frame's self time.
pub fn record_frame(total_ms: f64) {
    with(|recorder| recorder.record_frame(total_ms));
}

// The control functions are driven by the wasm profiler and by the tests; a
// native build keeps them compiled but unused.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn start(duration_ms: u32) -> Result<(), Fault> {
    if duration_ms > MAX_DURATION_MS {
        return Err(Fault::new(
            "invalid_request",
            format!("duration_ms must be at most {MAX_DURATION_MS}"),
        ));
    }
    with(|recorder| {
        if recorder.status == ProfileStatus::Running {
            return Err(Fault::new(
                "invalid_request",
                "a cooperative profile is already running",
            ));
        }
        recorder.begin(duration_ms.max(1));
        Ok(())
    })
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn stop() -> Result<(), Fault> {
    with(|recorder| {
        recorder.finish_if_expired();
        if recorder.status == ProfileStatus::Idle {
            return Err(Fault::new(
                "invalid_request",
                "no cooperative profile has been started",
            ));
        }
        recorder.finish();
        Ok(())
    })
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn read(thread: &str) -> UiProfile {
    with(|recorder| {
        recorder.finish_if_expired();
        let status = recorder.status;
        recorder.summary(thread, status)
    })
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn hotspots(thread: &str, limit: usize) -> UiHotspots {
    with(|recorder| {
        recorder.finish_if_expired();
        recorder.hotspots(thread, limit)
    })
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn reset() {
    with(|recorder| {
        recorder.clear_run();
        recorder.status = ProfileStatus::Idle;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs on one thread; the recorder is thread-local, so tests reset it.
    fn profile_frame(phases: &[(&'static str, f64)], total: f64) {
        for (label, ms) in phases {
            record_phase(label, *ms);
        }
        record_frame(total);
    }

    #[test]
    fn accumulates_measured_phases_into_one_root_and_one_stack_per_phase() {
        reset();
        start(0).unwrap();
        profile_frame(
            &[("recompose", 2.0), ("layout", 6.0), ("encode", 3.0)],
            12.0,
        );
        profile_frame(&[("layout", 5.0), ("encode", 2.0)], 9.0);
        stop().unwrap();

        let profile = read("web");
        assert_eq!(profile.source, ProfileSource::Instrumented);
        assert_eq!(profile.status, ProfileStatus::Ready);
        assert_eq!(profile.samples_taken, 2, "one recorded frame per frame");
        assert_eq!(profile.retained_samples, 21, "21 ms of measured frame time");
        assert_eq!(profile.hz, 0, "there is no sampling rate to report");
        assert_eq!(profile.stacks.len(), 4, "root plus three phases");

        let layout = profile
            .stacks
            .iter()
            .find(|stack| stack.frames.len() > 1 && stack.frames[1].label == "layout")
            .expect("layout stack");
        assert_eq!(layout.samples, 11, "11 ms of layout across both frames");
        assert!((layout.percent - 11.0 / 21.0 * 100.0).abs() < 0.01);

        let root = &profile.stacks[0];
        assert_eq!(root.samples, 3, "3 ms outside the phases (12-11 and 9-7)");
        assert!((root.percent - 3.0 / 21.0 * 100.0).abs() < 0.01);

        // The units partition the frame total: root self plus the phases.
        let units: u64 = profile.stacks.iter().map(|stack| stack.samples).sum();
        assert_eq!(units, profile.retained_samples);
    }

    #[test]
    fn frame_self_time_is_time_outside_the_instrumented_phases() {
        reset();
        start(0).unwrap();
        profile_frame(&[("layout", 6.0)], 10.0);
        stop().unwrap();

        let hotspots = hotspots("web", 10);
        assert_eq!(hotspots.hotspots[0].label, "frame");
        assert!((hotspots.hotspots[0].self_ms - 4.0).abs() < 0.001);
        assert!((hotspots.hotspots[0].inclusive_ms - 10.0).abs() < 0.001);
        let layout = hotspots
            .hotspots
            .iter()
            .find(|entry| entry.label == "layout")
            .expect("layout hotspot");
        assert!((layout.self_ms - 6.0).abs() < 0.001);
        assert!((layout.self_percent - 60.0).abs() < 0.01);
    }

    #[test]
    fn a_duration_bound_finishes_the_run_without_a_stop_command() {
        reset();
        start(1).unwrap();
        profile_frame(&[], 3.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
        profile_frame(&[], 3.0);
        assert_eq!(read("web").status, ProfileStatus::Ready);
        assert!(!is_recording());
        reset();
    }

    #[test]
    fn a_read_finishes_a_run_whose_duration_expired_without_a_frame() {
        reset();
        start(1).unwrap();
        profile_frame(&[("layout", 2.0)], 3.0);
        // A browser renders on demand: no new frame may arrive before the bound.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let profile = read("web");
        assert_eq!(
            profile.status,
            ProfileStatus::Ready,
            "the bound does not wait for a frame"
        );
        assert_eq!(profile.samples_taken, 1);
        reset();
    }

    #[test]
    fn distinct_phases_are_bounded_and_reported_as_dropped() {
        reset();
        start(0).unwrap();
        // Static labels, so the first twelve fill the bound and the rest drop.
        for index in 0..(MAX_DISTINCT_PHASES + 4) {
            let static_label: &'static str = match index {
                0 => "p0",
                1 => "p1",
                2 => "p2",
                3 => "p3",
                4 => "p4",
                5 => "p5",
                6 => "p6",
                7 => "p7",
                8 => "p8",
                9 => "p9",
                10 => "p10",
                11 => "p11",
                12 => "p12",
                13 => "p13",
                14 => "p14",
                _ => "p15",
            };
            record_phase(static_label, 1.0);
        }
        record_frame(20.0);
        stop().unwrap();
        let profile = read("web");
        assert_eq!(profile.stacks.len(), MAX_DISTINCT_PHASES + 1);
        assert_eq!(profile.samples_dropped, 4);
        assert!(profile.stacks_truncated);
        reset();
    }

    #[test]
    fn a_run_without_frames_answers_without_inventing_stacks() {
        reset();
        start(0).unwrap();
        stop().unwrap();
        let profile = read("web");
        assert_eq!(profile.samples_taken, 0);
        assert!(profile.stacks.is_empty());
        assert_eq!(hotspots("web", 10).hotspots.len(), 1, "the frame row only");
    }
}
