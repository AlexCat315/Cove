use cove_platform::InputEvent;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Instant};

pub const RECORDING_EVENT_LIMIT: usize = 4096;
pub const RECORDING_BYTE_LIMIT: usize = 768 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedInput {
    pub milliseconds: f64,
    pub event: InputEvent,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputRecording {
    pub version: u32,
    pub initial_state: BTreeMap<String, String>,
    pub focus_target: Option<String>,
    pub events: Vec<RecordedInput>,
    #[serde(default)]
    pub truncated: bool,
}
impl InputRecording {
    pub fn new(state: BTreeMap<String, String>, focus: Option<String>) -> Self {
        Self {
            version: 3,
            initial_state: state,
            focus_target: focus,
            events: vec![],
            truncated: false,
        }
    }
    pub fn is_valid(&self) -> bool {
        if self.version != 3
            || self.events.len() > RECORDING_EVENT_LIMIT
            || self.initial_state.len() > 256
            || self
                .initial_state
                .iter()
                .any(|(k, v)| k.len() > 1024 || v.len() > 32768)
            || self.focus_target.as_ref().is_some_and(|s| s.len() > 1024)
        {
            return false;
        }
        let mut previous = 0.0;
        for input in &self.events {
            if !input.milliseconds.is_finite()
                || input.milliseconds < previous
                || input.milliseconds > 86_400_000.0
                || !recordable(&input.event)
            {
                return false;
            }
            previous = input.milliseconds;
        }
        serde_json::to_vec(self).is_ok_and(|data| data.len() <= RECORDING_BYTE_LIMIT)
    }
}
fn recordable(event: &InputEvent) -> bool {
    match event {
        InputEvent::MouseButtonDown(e) | InputEvent::MouseButtonUp(e) => {
            e.x.is_finite() && e.y.is_finite()
        }
        InputEvent::MouseMotion(e) => [e.x, e.y, e.delta_x, e.delta_y]
            .iter()
            .all(|v| v.is_finite()),
        InputEvent::MouseWheel(e) => {
            e.input.timestamp >= 0.0
                && e.input.timestamp.is_finite()
                && [e.x, e.y, e.mouse_x.unwrap_or(0.0), e.mouse_y.unwrap_or(0.0)]
                    .iter()
                    .all(|v| v.is_finite())
        }
        InputEvent::KeyDown(_) | InputEvent::KeyUp(_) => true,
        InputEvent::TextInput(text) => text.len() <= 32768,
        InputEvent::TextEditing(e) => e.text.len() <= 32768 && e.start >= 0 && e.length >= 0,
        _ => false,
    }
}

#[derive(Default)]
pub struct InputRecorder {
    recording: Option<(InputRecording, Instant, usize)>,
}
impl InputRecorder {
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }
    /// Rejects an invalid initial checkpoint before recording starts.
    pub fn start(
        &mut self,
        state: BTreeMap<String, String>,
        focus: Option<String>,
    ) -> Result<(), &'static str> {
        let recording = InputRecording::new(state, focus);
        if !recording.is_valid() {
            return Err("invalid recording checkpoint");
        }
        let size = serde_json::to_vec(&recording)
            .map_err(|_| "invalid recording checkpoint")?
            .len();
        self.recording = Some((recording, Instant::now(), size));
        Ok(())
    }
    pub fn record(&mut self, event: InputEvent) {
        let Some((recording, start, bytes)) = &mut self.recording else {
            return;
        };
        if !recordable(&event) {
            return;
        }
        if recording.events.len() == RECORDING_EVENT_LIMIT {
            recording.truncated = true;
            return;
        }
        let input = RecordedInput {
            milliseconds: start.elapsed().as_secs_f64() * 1000.0,
            event,
        };
        let Ok(size) = serde_json::to_vec(&input).map(|data| data.len() + 1) else {
            recording.truncated = true;
            return;
        };
        if bytes.saturating_add(size) > RECORDING_BYTE_LIMIT {
            recording.truncated = true;
            return;
        }
        *bytes += size;
        recording.events.push(input);
    }
    pub fn stop(&mut self) -> Option<InputRecording> {
        self.recording.take().map(|(recording, _, _)| recording)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recordings_are_bounded_and_replay_times_are_monotonic() {
        let mut recorder = InputRecorder::default();
        recorder.start(BTreeMap::new(), None).unwrap();
        for _ in 0..4100 {
            recorder.record(InputEvent::TextInput("🦀".into()));
        }
        let mut recording = recorder.stop().unwrap();
        assert_eq!(recording.events.len(), RECORDING_EVENT_LIMIT);
        assert!(recording.truncated);
        assert!(recording.is_valid());
        recording.events[1].milliseconds = -1.0;
        assert!(!recording.is_valid());
        assert!(!recorder.is_recording());
    }
}
