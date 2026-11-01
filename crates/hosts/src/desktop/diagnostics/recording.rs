//! Translation of Desktop/Sources/CoveUI/Diagnostics/Recording.swift.
//!
//! The Swift file installed DevTools checkpoint/recording closures that reached
//! into `Application` and `PlatformWindowSession`. The host side of those
//! closures is the input recorder plus the replay state machine below; the
//! DevTools adapter installs them through [`RecordingControl`].

use crate::desktop::diagnostics::checkpoint::ApplicationCheckpoint;
use cove_platform::InputEvent;
use std::collections::BTreeMap;
use std::rc::Rc;

/// One recorded input event with the monotonic time it was observed.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedInput {
    pub event: InputEvent,
    pub timestamp: f64,
}

/// A complete recording: the state it started from, the focused control and
/// every event delivered afterwards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputRecording {
    pub initial_state: BTreeMap<String, String>,
    pub focus_target: Option<String>,
    pub events: Vec<RecordedInput>,
}

impl InputRecording {
    /// A recording without initial state can never restore a session.
    pub fn is_valid(&self) -> bool {
        !self.initial_state.is_empty() || !self.events.is_empty()
    }
}

/// Records the events a window receives so DevTools can replay a session.
#[derive(Default)]
pub struct InputRecorder {
    recording: Option<InputRecording>,
    started_at: Option<std::time::Instant>,
}

impl InputRecorder {
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Begins a recording unless one is already running, matching the Swift
    /// `start` guard that returned `false`.
    pub fn start(&mut self, state: BTreeMap<String, String>, focus_target: Option<String>) -> bool {
        if self.recording.is_some() {
            return false;
        }
        self.recording = Some(InputRecording {
            initial_state: state,
            focus_target,
            events: Vec::new(),
        });
        self.started_at = Some(std::time::Instant::now());
        true
    }

    pub fn record(&mut self, event: &InputEvent) {
        let timestamp = self
            .started_at
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        if let Some(recording) = &mut self.recording {
            recording.events.push(RecordedInput {
                event: event.clone(),
                timestamp,
            });
        }
    }

    pub fn stop(&mut self) -> Option<InputRecording> {
        self.started_at = None;
        self.recording.take()
    }

    pub fn recording(&self) -> Option<&InputRecording> {
        self.recording.as_ref()
    }
}

/// The DevTools-facing hooks the Swift extension installed. Each returns the
/// same boolean the Swift closure did, so a rejected request is visible.
#[derive(Clone, Default)]
pub struct RecordingControl {
    pub capture: Option<Rc<dyn Fn() -> BTreeMap<String, String>>>,
    pub restore: Option<Rc<dyn Fn(&BTreeMap<String, String>) -> bool>>,
    pub start: Option<Rc<dyn Fn() -> bool>>,
    pub stop: Option<Rc<dyn Fn()>>,
    pub replay: Option<Rc<dyn Fn(&InputRecording) -> bool>>,
}

impl RecordingControl {
    /// Installs the checkpoint and recorder hooks exactly as
    /// `configureCheckpoint(_:)` did.
    pub fn install(
        &mut self,
        checkpoint: Option<&ApplicationCheckpoint>,
        recorder: Rc<std::cell::RefCell<InputRecorder>>,
    ) {
        let Some(checkpoint) = checkpoint.cloned() else {
            return;
        };
        let capture_checkpoint = checkpoint.clone();
        self.capture = Some(Rc::new(move || capture_checkpoint.capture()));
        let restore_checkpoint = checkpoint.clone();
        self.restore = Some(Rc::new(move |state| restore_checkpoint.restore(state)));

        let start_recorder = recorder.clone();
        let start_checkpoint = checkpoint.clone();
        self.start = Some(Rc::new(move || {
            let state = start_checkpoint.capture();
            start_recorder.borrow_mut().start(state, None)
        }));
        let stop_instance = recorder.clone();
        self.stop = Some(Rc::new(move || {
            let _ = stop_instance.borrow_mut().stop();
        }));
    }
}
