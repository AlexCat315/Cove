//! Shared state of one platform-thread and UI-thread pair.
use super::{MainThreadBridge, PlatformState};
use cove_platform::{ScreenColorSample, WindowId, WindowInputEvent};
use cove_shell::EventLoopWaker;
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

/// What the platform thread reports back. Only ever delivered, never awaited.
#[derive(Debug)]
pub enum PlatformMessage {
    Input(WindowInputEvent),
    CloseRequested(Option<WindowId>),
    WindowDestroyed(WindowId),
    WindowFailed(WindowId, String),
    DialogFinished(u64, Vec<String>),
    MenuFinished(u64, Option<i64>),
    ClipboardRead(u64, Option<String>),
    ColorSampled(u64, ScreenColorSample),
}

pub struct PlatformLink {
    pub bridge: MainThreadBridge,
    pub state: PlatformState,
    inbox: Mutex<VecDeque<PlatformMessage>>,
    /// Bumped by every wake. A generation wait cannot lose a wake the way a
    /// coalesced native event can.
    generation: Mutex<u64>,
    changed: Condvar,
    heartbeat: AtomicU64,
    finished: AtomicBool,
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<PlatformLink>>> = const { RefCell::new(None) };
}

impl PlatformLink {
    pub fn new(platform_wake: EventLoopWaker) -> Self {
        Self {
            bridge: MainThreadBridge::new(platform_wake),
            state: PlatformState::default(),
            inbox: Mutex::default(),
            generation: Mutex::new(0),
            changed: Condvar::new(),
            heartbeat: AtomicU64::new(0),
            finished: AtomicBool::new(false),
        }
    }

    /// The link of the calling UI thread. Absent when the caller owns the
    /// platform directly.
    pub fn current() -> Option<Arc<Self>> {
        CURRENT.with(|slot| slot.borrow().clone())
    }

    pub fn install_current(link: Option<Arc<Self>>) {
        CURRENT.with(|slot| *slot.borrow_mut() = link);
    }

    pub fn post(&self, message: PlatformMessage) {
        self.inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(message);
        self.wake_ui();
    }

    pub fn take_messages(&self) -> VecDeque<PlatformMessage> {
        std::mem::take(&mut *self.inbox.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn wake_ui(&self) {
        *self.generation.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.changed.notify_all();
    }

    pub fn generation(&self) -> u64 {
        *self.generation.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Blocks the UI thread until a wake newer than `seen` or the timeout.
    pub fn wait_ui(&self, seen: u64, timeout: Duration) -> bool {
        let guard = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |generation| *generation == seen)
            .unwrap_or_else(|e| e.into_inner());
        *guard != seen
    }

    pub fn beat(&self) {
        self.heartbeat.fetch_add(1, Ordering::Relaxed);
    }

    pub fn heartbeat(&self) -> u64 {
        self.heartbeat.load(Ordering::Relaxed)
    }

    pub fn mark_finished(&self) {
        self.finished.store(true, Ordering::Release);
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}
