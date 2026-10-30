//! Translation of Android/CoveAndroidApp/Frame/GPUTimings.swift.
//!
//! The RHI profile completes asynchronously, after the GPU retires a frame, so
//! these numbers describe recently finished work rather than the frame being
//! recorded. Reported once a second so steady state produces one line per
//! second instead of one line per frame.

use cove_graphics::rhi::GpuTimestampProfile;
use cove_graphics::rhi::Device;
use cove_logging::{Level, LogEvent, LogSystem};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default)]
struct Accumulator {
    count: u64,
    total: u64,
    worst: u64,
}

/// Rolling GPU pass timings for one surface.
pub struct GpuTimingReporter {
    profile: Option<Arc<GpuTimestampProfile>>,
    reported: usize,
    window: BTreeMap<String, Accumulator>,
    window_start: u64,
}

impl GpuTimingReporter {
    const HEARTBEAT_NANOS: u64 = 1_000_000_000;

    /// Attaches a profile when the backend can timestamp and the caller asked
    /// for timings. Otherwise the reporter stays inactive and does no work.
    pub fn new(device: &Device, enabled: bool) -> Self {
        if !enabled || !device.supports_gpu_timestamps() {
            return Self {
                profile: None,
                reported: 0,
                window: BTreeMap::new(),
                window_start: 0,
            };
        }
        // Headroom for a long run: this reporter only needs the retention bound
        // not to truncate a session before its summary lines are read.
        let created = Arc::new(GpuTimestampProfile::new(1 << 16));
        device.set_gpu_timestamp_profile(Some(created.clone()));
        Self { profile: Some(created), reported: 0, window: BTreeMap::new(), window_start: 0 }
    }

    pub fn is_recording(&self) -> bool {
        self.profile.is_some()
    }

    pub fn tick(&mut self, timestamp: u64) {
        let Some(profile) = self.profile.clone() else { return };
        if self.window_start == 0 {
            self.window_start = timestamp;
        }
        let markers = profile.recorded_markers();
        if markers.len() > self.reported {
            for marker in &markers[self.reported..] {
                let entry = self.window.entry(marker.label.clone()).or_default();
                entry.count += 1;
                entry.total = entry.total.saturating_add(marker.nanoseconds);
                entry.worst = entry.worst.max(marker.nanoseconds);
            }
            self.reported = markers.len();
        }
        // The heartbeat is unconditional: a window with no passes distinguishes
        // "nothing was sampled" from "sampling is not running at all".
        let elapsed = timestamp.saturating_sub(self.window_start);
        if elapsed < Self::HEARTBEAT_NANOS {
            return;
        }
        let window_milliseconds = elapsed as f64 / 1_000_000.0;
        self.window_start = timestamp;
        if self.window.is_empty() {
            LogSystem::logger("com.cove.android.timing").log(&LogEvent::new(
                Level::Info,
                format!(
                    "[timing] android.gpu window={}ms passes=0 status={}",
                    text(window_milliseconds),
                    profile.status()
                ),
            ));
            return;
        }
        let summary = self
            .window
            .iter()
            .filter(|(_, entry)| entry.count > 0)
            .map(|(label, entry)| {
                let average = entry.total as f64 / entry.count as f64 / 1_000_000.0;
                let worst = entry.worst as f64 / 1_000_000.0;
                // `key=value` pairs with word characters only, so the
                // repository's telemetry parser can read this line like the
                // other timing lines.
                let key = parseable(label);
                format!("{key}_avg={}ms {key}_max={}ms", text(average), text(worst))
            })
            .collect::<Vec<_>>()
            .join(" ");
        LogSystem::logger("com.cove.android.timing").log(&LogEvent::new(
            Level::Info,
            format!(
                "[timing] android.gpu window={}ms passes={} {}",
                text(window_milliseconds),
                self.window.len(),
                summary
            ),
        ));
        self.window.clear();
    }
}

/// Pass labels are authored for humans (`ui-segment`, `backdrop-blur-h`);
/// telemetry keys may only use word characters.
fn parseable(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character == '_' { character } else { '_' }
        })
        .collect()
}

fn text(milliseconds: f64) -> String {
    format!("{milliseconds:.3}")
}
