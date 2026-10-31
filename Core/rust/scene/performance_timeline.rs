//! Translation of CoveUIScene/PerformanceTimeline.swift.
use super::RecompositionReason;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub sequence: u64,
    pub phase: String,
    pub name: String,
    pub scope_id: Option<String>,
    pub start_ms: f64,
    pub duration_ms: f64,
    pub reasons: Vec<RecompositionReason>,
}

/// Bounded, opt-in CPU timeline owned and accessed by the scene thread.
#[derive(Debug)]
pub struct PerformanceTimeline {
    enabled: bool,
    externally_enabled: bool,
    active_recordings: usize,
    buffer: Vec<TimelineEvent>,
    next_index: usize,
    dropped: u64,
    sequence: u64,
    origin: Instant,
    capacity: usize,
}
impl PerformanceTimeline {
    pub const DEFAULT_CAPACITY: usize = 2048;
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: false,
            externally_enabled: false,
            active_recordings: 0,
            buffer: Vec::new(),
            next_index: 0,
            dropped: 0,
            sequence: 0,
            origin: Instant::now(),
            capacity: capacity.clamp(1, 8192),
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
    pub fn latest_sequence(&self) -> u64 {
        self.sequence
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn set_enabled(&mut self, value: bool) {
        self.externally_enabled = value;
        self.synchronize_recording();
    }
    /// Bracket a capture that must run while another consumer is recording.
    pub fn begin_recording(&mut self) {
        self.active_recordings = self.active_recordings.saturating_add(1);
        self.synchronize_recording();
    }
    pub fn end_recording(&mut self) {
        self.active_recordings = self.active_recordings.saturating_sub(1);
        self.synchronize_recording();
    }
    fn synchronize_recording(&mut self) {
        let value = self.externally_enabled || self.active_recordings > 0;
        if value && !self.enabled {
            self.buffer.clear();
            self.next_index = 0;
            self.dropped = 0;
            self.origin = Instant::now();
        }
        self.enabled = value;
    }
    /// Oldest-to-newest, unwrapping the ring.
    pub fn events(&self) -> Vec<TimelineEvent> {
        if self.next_index == 0 {
            return self.buffer.clone();
        }
        self.buffer[self.next_index..]
            .iter()
            .chain(self.buffer[..self.next_index].iter())
            .cloned()
            .collect()
    }
    /// Read only the tail after a connection cursor, without copying old spans.
    pub fn events_after(&self, sequence: u64) -> Vec<TimelineEvent> {
        let Some(last) = self.buffer.last() else {
            return Vec::new();
        };
        let latest = if self.buffer.len() == self.capacity {
            self.buffer[(self.next_index + self.buffer.len() - 1) % self.buffer.len()].sequence
        } else {
            last.sequence
        };
        if latest <= sequence {
            return Vec::new();
        }
        let count = ((latest - sequence) as usize).min(self.buffer.len());
        let first = if self.buffer.len() == self.capacity {
            (self.next_index + self.buffer.len() - count) % self.buffer.len()
        } else {
            self.buffer.len() - count
        };
        (0..count)
            .map(|offset| self.buffer[(first + offset) % self.buffer.len()].clone())
            .collect()
    }
    pub fn begin(&self) -> Option<f64> {
        self.enabled.then(|| self.milliseconds())
    }
    pub fn end(
        &mut self,
        start: Option<f64>,
        phase: impl Into<String>,
        name: impl Into<String>,
        scope_id: Option<String>,
        reasons: &[RecompositionReason],
    ) {
        let (true, Some(start)) = (self.enabled, start) else {
            return;
        };
        self.sequence = self.sequence.wrapping_add(1);
        let name = name.into();
        let event = TimelineEvent {
            sequence: self.sequence,
            phase: phase.into(),
            name: if name.chars().count() > 160 {
                name.chars().take(160).collect()
            } else {
                name
            },
            scope_id,
            start_ms: start,
            duration_ms: (self.milliseconds() - start).max(0.0),
            reasons: reasons.iter().take(16).cloned().collect(),
        };
        if self.buffer.len() < self.capacity {
            self.buffer.push(event);
        } else {
            self.buffer[self.next_index] = event;
            self.next_index = (self.next_index + 1) % self.capacity;
            self.dropped += 1;
        }
    }
    fn milliseconds(&self) -> f64 {
        self.origin.elapsed().as_secs_f64() * 1000.0
    }
}
impl Default for PerformanceTimeline {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recording_wraps_and_reports_tail() {
        let mut timeline = PerformanceTimeline::new(2);
        assert!(timeline.begin().is_none());
        timeline.set_enabled(true);
        for index in 0..3 {
            let start = timeline.begin().unwrap();
            timeline.end(Some(start), "compose", format!("span{index}"), None, &[]);
        }
        let events = timeline.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].name, "span2");
        assert_eq!(timeline.dropped(), 1);
        assert_eq!(timeline.events_after(2).len(), 1);
        assert!(timeline.events_after(3).is_empty());
        timeline.end(None, "compose", "ignored", None, &[]);
        assert_eq!(timeline.latest_sequence(), 3);
    }
    #[test]
    fn disabling_clears_the_buffer_on_reenable() {
        let mut timeline = PerformanceTimeline::new(8);
        timeline.set_enabled(true);
        let start = timeline.begin().unwrap();
        timeline.end(Some(start), "layout", "a", None, &[]);
        timeline.set_enabled(false);
        timeline.set_enabled(true);
        assert!(timeline.events().is_empty());
        assert_eq!(timeline.dropped(), 0);
    }
}
