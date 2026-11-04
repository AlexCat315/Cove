use std::fmt;
#[cfg(any(feature = "gst", target_arch = "wasm32"))]
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaErrorKind {
    Initialization,
    InvalidInput,
    Busy,
    Closed,
    Source,
    Decode,
    Negotiation,
    Unsupported,
    Device,
    Cancelled,
    Timeout,
    RequestUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaError {
    pub kind: MediaErrorKind,
    pub message: String,
}
impl MediaError {
    pub fn new(kind: MediaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for MediaError {}
pub type MediaResult<T> = Result<T, MediaError>;

/// Integer nanoseconds. Unknown timestamps are represented by Option<MediaTime>.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MediaTime(pub u64);
impl MediaTime {
    pub fn from_seconds(seconds: f64) -> MediaResult<Self> {
        let nanos = seconds * 1_000_000_000.0;
        if !seconds.is_finite() || seconds < 0.0 || nanos >= u64::MAX as f64 {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "media time is out of range",
            ));
        }
        Ok(Self(nanos.round() as u64))
    }
    pub fn seconds(self) -> f64 {
        self.0 as f64 / 1_000_000_000.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(pub u64);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Empty,
    Opening,
    Paused,
    Playing,
    Ended,
    Failed,
    Closed,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackIntent {
    #[default]
    Paused,
    Playing,
    Stopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    Audio,
    Video,
    Subtitle,
    Other,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackInfo {
    pub id: String,
    pub kind: TrackKind,
    pub caps: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekAccuracy {
    Keyframe,
    Exact,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekRequest {
    pub position: MediaTime,
    pub accuracy: SeekAccuracy,
}
impl SeekRequest {
    pub fn exact(position: MediaTime) -> Self {
        Self {
            position,
            accuracy: SeekAccuracy::Exact,
        }
    }
    pub fn keyframe(position: MediaTime) -> Self {
        Self {
            position,
            accuracy: SeekAccuracy::Keyframe,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Completed { position: Option<MediaTime> },
    Superseded,
    Failed(MediaError),
}

/// Events are a bounded change log; request results remain queryable independently.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerEvent {
    pub sequence: u64,
    pub source_generation: u64,
    pub kind: PlayerEventKind,
}
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PlayerEventKind {
    SnapshotChanged(PlayerSnapshot),
    CommandCompleted {
        request: RequestId,
        outcome: CommandOutcome,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerEvents {
    pub events: Vec<PlayerEvent>,
    /// Pass this value to the next events_since call. Zero starts at the oldest event.
    pub next_cursor: u64,
    /// Events were overwritten. Refresh the snapshot and query outstanding requests.
    pub missed: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSnapshot {
    pub state: PlaybackState,
    pub requested: PlaybackIntent,
    pub source_generation: u64,
    pub frame_generation: u64,
    pub revision: u64,
    pub position: Option<MediaTime>,
    pub duration: Option<MediaTime>,
    pub seekable_range: Option<(MediaTime, Option<MediaTime>)>,
    pub buffering_percent: Option<u8>,
    pub seeking: bool,
    pub rate: f64,
    pub volume: f64,
    pub tracks: Vec<TrackInfo>,
    pub selected_tracks: Vec<String>,
    pub last_error: Option<MediaError>,
    pub decoder_elements: Vec<String>,
}
impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            state: PlaybackState::Empty,
            requested: PlaybackIntent::Paused,
            source_generation: 0,
            frame_generation: 0,
            revision: 0,
            position: None,
            duration: None,
            seekable_range: None,
            buffering_percent: None,
            seeking: false,
            rate: 1.0,
            volume: 1.0,
            tracks: Vec::new(),
            selected_tracks: Vec::new(),
            last_error: None,
            decoder_elements: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContextOptions {
    pub command_capacity: usize,
    /// Includes pending and completed, unacknowledged requests.
    pub result_capacity: usize,
    pub event_capacity: usize,
    /// Backend operations that never confirm completion fail and stop the session.
    pub operation_timeout: std::time::Duration,
}
impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            command_capacity: 128,
            result_capacity: 256,
            event_capacity: 128,
            operation_timeout: std::time::Duration::from_secs(30),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioOutput {
    #[default]
    DefaultDevice,
    Silent,
}
#[derive(Clone, Debug, Default)]
pub struct PlayerOptions {
    pub autoplay: bool,
    pub audio: AudioOutput,
}

#[cfg(any(feature = "gst", target_arch = "wasm32"))]
struct EventJournal {
    next: u64,
    capacity: usize,
    events: VecDeque<PlayerEvent>,
}
#[cfg(any(feature = "gst", target_arch = "wasm32"))]
pub(crate) struct SharedPlayer {
    pub snapshot: Mutex<PlayerSnapshot>,
    pub results: Mutex<BTreeMap<RequestId, Option<CommandOutcome>>>,
    pub changed: Condvar,
    pub output: crate::VideoOutput,
    pub abandoned: std::sync::atomic::AtomicBool,
    pub next_request: std::sync::atomic::AtomicU64,
    pub result_capacity: usize,
    events: Mutex<EventJournal>,
}
#[cfg(any(feature = "gst", target_arch = "wasm32"))]
impl SharedPlayer {
    pub fn new(result_capacity: usize, event_capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            snapshot: Mutex::new(PlayerSnapshot::default()),
            results: Mutex::new(BTreeMap::new()),
            changed: Condvar::new(),
            output: crate::VideoOutput::new(),
            abandoned: std::sync::atomic::AtomicBool::new(false),
            next_request: std::sync::atomic::AtomicU64::new(1),
            result_capacity,
            events: Mutex::new(EventJournal {
                next: 1,
                capacity: event_capacity,
                events: VecDeque::new(),
            }),
        })
    }
    pub fn update(&self, operation: impl FnOnce(&mut PlayerSnapshot)) {
        let mut state = self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = state.clone();
        let revision = state.revision;
        operation(&mut state);
        state.revision = revision;
        if *state == previous {
            return;
        }
        state.revision = revision.checked_add(1).expect("media revision exhausted");
        self.record(
            state.source_generation,
            PlayerEventKind::SnapshotChanged(state.clone()),
        );
        self.changed.notify_all();
    }
    pub fn finish(&self, request: RequestId, result: CommandOutcome) {
        let generation = self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .source_generation;
        let mut results = self
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(slot) = results.get_mut(&request) else {
            return;
        };
        if slot.is_some() {
            return;
        }
        *slot = Some(result.clone());
        self.record(
            generation,
            PlayerEventKind::CommandCompleted {
                request,
                outcome: result,
            },
        );
        drop(results);
        self.changed.notify_all();
    }
    fn record(&self, source_generation: u64, kind: PlayerEventKind) {
        let mut journal = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sequence = journal.next;
        journal.next = sequence
            .checked_add(1)
            .expect("media event sequence exhausted");
        if journal.events.len() == journal.capacity {
            journal.events.pop_front();
        }
        journal.events.push_back(PlayerEvent {
            sequence,
            source_generation,
            kind,
        });
    }
    pub fn events_since(&self, cursor: u64) -> PlayerEvents {
        let journal = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let oldest = journal
            .events
            .front()
            .map_or(journal.next, |event| event.sequence);
        let requested = if cursor == 0 { oldest } else { cursor };
        PlayerEvents {
            events: journal
                .events
                .iter()
                .filter(|event| event.sequence >= requested)
                .cloned()
                .collect(),
            next_cursor: journal.next,
            missed: oldest.saturating_sub(requested),
        }
    }
    pub fn wait_result(&self, id: RequestId, timeout: Duration) -> MediaResult<CommandOutcome> {
        let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
            MediaError::new(MediaErrorKind::InvalidInput, "timeout is out of range")
        })?;
        let mut results = self
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            match results.get(&id) {
                Some(Some(result)) => return Ok(result.clone()),
                None => {
                    return Err(MediaError::new(
                        MediaErrorKind::RequestUnknown,
                        "unknown or acknowledged request",
                    ));
                }
                Some(None) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(MediaError::new(
                    MediaErrorKind::Timeout,
                    "media command timed out",
                ));
            }
            results = self
                .changed
                .wait_timeout(results, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}
