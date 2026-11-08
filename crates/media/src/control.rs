//! The playback contract UI components drive, and the change notification they observe.
use crate::{AudioMeter, MediaResult, PlayerSnapshot, SeekRequest, VideoOutput};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

/// Invoked on the thread that changed the observed value. Keep it short and
/// non-blocking: post to the UI thread instead of doing UI work in place.
pub type ChangeCallback = Arc<dyn Fn() + Send + Sync>;

/// A fan-out of change callbacks. Implementors of [`PlaybackControl`] and custom
/// frame sources reuse it to honour `on_change`.
#[derive(Clone, Default)]
pub struct ChangeNotifier(Arc<NotifierInner>);

#[derive(Default)]
struct NotifierInner {
    callbacks: Mutex<BTreeMap<u64, ChangeCallback>>,
    next: AtomicU64,
}

impl ChangeNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `callback` until the returned listener is dropped.
    pub fn listen(&self, callback: ChangeCallback) -> ChangeListener {
        let id = self.0.next.fetch_add(1, Ordering::Relaxed);
        self.0
            .callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, callback);
        ChangeListener {
            notifier: Arc::downgrade(&self.0),
            id,
        }
    }

    /// Calls every registered callback outside the registration lock.
    pub fn notify(&self) {
        let callbacks: Vec<ChangeCallback> = self
            .0
            .callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        for callback in callbacks {
            callback();
        }
    }

    pub fn listener_count(&self) -> usize {
        self.0
            .callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

/// Keeps a change callback registered; dropping it unregisters.
#[must_use = "dropping the listener unregisters its callback"]
pub struct ChangeListener {
    notifier: Weak<NotifierInner>,
    id: u64,
}

impl Drop for ChangeListener {
    fn drop(&mut self) {
        if let Some(notifier) = self.notifier.upgrade() {
            notifier
                .callbacks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&self.id);
        }
    }
}

/// Fire-and-forget playback control shared by UI components and custom sources.
///
/// Commands are submitted without waiting; their effect is observed through
/// [`PlaybackControl::snapshot`] after a change notification.
pub trait PlaybackControl: Send + Sync {
    fn snapshot(&self) -> PlayerSnapshot;
    /// The frame mailbox of a source with pictures.
    fn video_output(&self) -> Option<VideoOutput>;
    /// Levels and sample history of what the output actually played.
    fn audio_meter(&self) -> Option<AudioMeter> {
        None
    }
    fn play(&self) -> MediaResult<()>;
    fn pause(&self) -> MediaResult<()>;
    fn seek(&self, request: SeekRequest) -> MediaResult<()>;
    /// `0.0..=1.0`.
    fn set_volume(&self, volume: f64) -> MediaResult<()>;
    /// `0.1..=16.0`.
    fn set_rate(&self, rate: f64) -> MediaResult<()>;
    /// Track ids from `PlayerSnapshot::tracks` to decode; sources with one
    /// rendition report `Unsupported`.
    fn select_tracks(&self, tracks: Vec<String>) -> MediaResult<()> {
        let _ = tracks;
        Err(crate::MediaError::new(
            crate::MediaErrorKind::Unsupported,
            "this source has no alternative tracks",
        ))
    }
    /// Called after every snapshot change.
    fn on_change(&self, callback: ChangeCallback) -> ChangeListener;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn dropped_listeners_stop_receiving() {
        let notifier = ChangeNotifier::new();
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        let listener = notifier.listen(Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        }));
        notifier.notify();
        drop(listener);
        notifier.notify();
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert_eq!(notifier.listener_count(), 0);
    }
}
