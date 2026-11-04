use crate::{MediaError, MediaErrorKind, MediaResult, MediaTime};
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8,
    Bgra8,
    Other(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaneLayout {
    pub stride: usize,
    pub offset: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum VideoDynamicRange {
    #[default]
    Unknown,
    Sdr,
    Pq,
    Hlg,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFormat {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub planes: Vec<PlaneLayout>,
    pub pixel_aspect_ratio: (u32, u32),
    pub frame_rate: (u32, u32),
    pub colorimetry: String,
    pub dynamic_range: VideoDynamicRange,
    /// Full negotiated backend format, including memory features and HDR fields.
    pub negotiated_caps: String,
}
/// Copying is explicit. Hardware-backed implementations may reject CPU access.
pub trait VideoStorage: Any + Send + Sync {
    fn copy_planes(&self) -> MediaResult<Vec<Vec<u8>>>;
    fn as_any(&self) -> &dyn Any;
}
#[derive(Clone)]
pub struct VideoFrame {
    pub generation: u64,
    pub sequence: u64,
    pub pts: Option<MediaTime>,
    pub dts: Option<MediaTime>,
    pub duration: Option<MediaTime>,
    pub format: VideoFormat,
    pub storage: Arc<dyn VideoStorage>,
}
impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFrame")
            .field("generation", &self.generation)
            .field("sequence", &self.sequence)
            .field("pts", &self.pts)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

/// Shared latest-frame mailbox. Old snapshots retain their own backing lease.
#[derive(Clone)]
pub struct VideoOutput(Arc<OutputInner>);
struct OutputInner {
    id: u64,
    state: Mutex<OutputState>,
    subscriptions: Mutex<BTreeMap<u64, mpsc::SyncSender<()>>>,
    next_subscription: AtomicU64,
}
struct OutputState {
    generation: u64,
    revision: u64,
    closed: bool,
    latest: Option<Arc<VideoFrame>>,
}
#[derive(Clone, Debug)]
pub struct VideoOutputSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub closed: bool,
    pub frame: Option<Arc<VideoFrame>>,
}
/// A coalesced wake-up channel. Read snapshot after waking; no frame queue is retained.
pub struct VideoSubscription {
    output: Weak<OutputInner>,
    id: u64,
    receiver: mpsc::Receiver<()>,
}
impl VideoSubscription {
    pub fn try_changed(&self) -> bool {
        self.receiver.try_recv().is_ok()
    }
    pub fn wait_changed(&self, timeout: std::time::Duration) -> Result<(), mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}
impl Drop for VideoSubscription {
    fn drop(&mut self) {
        if let Some(output) = self.output.upgrade() {
            output
                .subscriptions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&self.id);
        }
    }
}
impl Default for VideoOutput {
    fn default() -> Self {
        Self::new()
    }
}
impl VideoOutput {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("video output identifiers exhausted");
        Self(Arc::new(OutputInner {
            id,
            state: Mutex::new(OutputState {
                generation: 0,
                revision: 0,
                closed: false,
                latest: None,
            }),
            subscriptions: Mutex::new(BTreeMap::new()),
            next_subscription: AtomicU64::new(1),
        }))
    }
    pub fn id(&self) -> u64 {
        self.0.id
    }
    pub fn revision(&self) -> u64 {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .revision
    }
    pub fn latest(&self) -> Option<Arc<VideoFrame>> {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latest
            .clone()
    }
    pub fn snapshot(&self) -> VideoOutputSnapshot {
        let state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        VideoOutputSnapshot {
            generation: state.generation,
            revision: state.revision,
            closed: state.closed,
            frame: state.latest.clone(),
        }
    }
    pub fn subscribe(&self) -> VideoSubscription {
        let (sender, receiver) = mpsc::sync_channel(1);
        let id = self
            .0
            .next_subscription
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("video subscription identifiers exhausted");
        let _ = sender.try_send(());
        self.0
            .subscriptions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, sender);
        VideoSubscription {
            output: Arc::downgrade(&self.0),
            id,
            receiver,
        }
    }
    fn notify(&self) {
        self.0
            .subscriptions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|_, sender| {
                !matches!(
                    sender.try_send(()),
                    Err(mpsc::TrySendError::Disconnected(_))
                )
            });
    }
    pub fn is_closed(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
    }
    pub fn begin_generation(&self) -> MediaResult<u64> {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(MediaError::new(
                MediaErrorKind::Closed,
                "video output is closed",
            ));
        }
        state.generation = state
            .generation
            .checked_add(1)
            .expect("video generation exhausted");
        state.revision = state
            .revision
            .checked_add(1)
            .expect("video revision exhausted");
        state.latest = None;
        let generation = state.generation;
        drop(state);
        self.notify();
        Ok(generation)
    }
    pub fn publish(&self, frame: VideoFrame) -> bool {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed
            || frame.generation != state.generation
            || state
                .latest
                .as_ref()
                .is_some_and(|last| frame.sequence <= last.sequence)
        {
            return false;
        }
        state.latest = Some(Arc::new(frame));
        state.revision = state
            .revision
            .checked_add(1)
            .expect("video revision exhausted");
        drop(state);
        self.notify();
        true
    }
    pub fn close(&self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return;
        }
        state.closed = true;
        state.latest = None;
        state.revision = state
            .revision
            .checked_add(1)
            .expect("video revision exhausted");
        drop(state);
        self.notify();
    }
}
