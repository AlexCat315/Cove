//! Asynchronous screen-colour sampling, one backend per window system.
//!
//! `cove-platform` owns the contract ([`ScreenColor`], [`ScreenColorSample`],
//! [`ScreenColorCompletion`]). This module owns every native implementation:
//! AppKit on macOS, Win32 on Windows, X11 and the XDG portal on Linux, and an
//! explicit "no backend" answer on sandboxed targets.
//!
//! Nothing here blocks the platform thread. AppKit's `NSColorSampler` starts a
//! system session and returns; the Win32 and Linux backends do their polling,
//! D-Bus conversation and pixel capture on their own worker thread. All of them
//! answer through [`CompletionSink`], which queues the result and wakes the
//! event loop. [`ScreenColorSampler::take_deferred_callbacks`] then runs the
//! completion on the platform thread, where the `Shell` and its ambients live.

use crate::EventLoopWaker;
use cove_platform::{ScreenColorBackend, ScreenColorCompletion, ScreenColorSample};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(target_os = "macos")]
mod appkit;
#[cfg(any(target_os = "linux", test))]
// Compiled off Linux too so the D-Bus wire codec has real coverage on this host.
#[allow(dead_code)]
mod dbus;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "windows", target_os = "linux", test))]
mod pixel;
#[cfg(target_os = "linux")]
mod portal;
#[cfg(target_os = "windows")]
mod win32;
#[cfg(target_os = "linux")]
mod x11;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The thread-safe half of a request. Worker threads and Objective-C blocks
/// hold only this; the `FnOnce` completion never leaves the platform thread.
#[derive(Clone)]
pub(crate) struct CompletionSink {
    queue: Arc<Mutex<VecDeque<(u64, ScreenColorSample)>>>,
    wake: EventLoopWaker,
}

impl CompletionSink {
    pub(crate) fn deliver(&self, request: u64, sample: ScreenColorSample) {
        lock(&self.queue).push_back((request, sample));
        (self.wake)();
    }
}

/// Keeps native state alive for as long as a sampling session runs, and asks
/// worker threads to stop when the session is superseded or the shell exits.
pub(crate) struct Session {
    cancel: Option<Arc<AtomicBool>>,
    native: Option<Box<dyn Any>>,
}

impl Session {
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn native(state: Box<dyn Any>) -> Self {
        Self {
            cancel: None,
            native: Some(state),
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux", test))]
    pub(crate) fn worker(cancel: Arc<AtomicBool>) -> Self {
        Self {
            cancel: Some(cancel),
            native: None,
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        self.native.take();
    }
}

/// Outcome of one backend attempt. `Unavailable` means "this window system is
/// not present, try the next backend"; `Done` is a final answer for the user.
#[cfg(target_os = "linux")]
pub(crate) enum Attempt {
    Done(ScreenColorSample),
    Unavailable(String),
}

/// Seam for tests: production uses [`NativeBackend`], tests drive the queue
/// with a backend that decides when to answer.
trait Backend {
    fn start(&self, request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample>;
}

struct NativeBackend;
impl Backend for NativeBackend {
    fn start(&self, request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
        native_start(request, sink)
    }
}

#[cfg(target_os = "macos")]
fn native_start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    appkit::start(request, sink)
}
#[cfg(target_os = "windows")]
fn native_start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    win32::start(request, sink)
}
#[cfg(target_os = "linux")]
fn native_start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    linux::start(request, sink)
}
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn native_start(_: u64, _: CompletionSink) -> Result<Session, ScreenColorSample> {
    Err(ScreenColorSample::Unsupported(
        "this platform does not expose screen pixels to applications".into(),
    ))
}

/// Which native backend serves sampling, or `None` when no backend can run.
/// Cheap enough to call at startup; never probes a native display synchronously.
pub fn screen_color_backend() -> ScreenColorBackend {
    native_backend()
}

#[cfg(target_os = "macos")]
fn native_backend() -> ScreenColorBackend {
    ScreenColorBackend::AppKit
}
#[cfg(target_os = "windows")]
fn native_backend() -> ScreenColorBackend {
    ScreenColorBackend::Win32
}
#[cfg(target_os = "linux")]
fn native_backend() -> ScreenColorBackend {
    linux::backend()
}
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn native_backend() -> ScreenColorBackend {
    ScreenColorBackend::None
}

/// Owns the in-flight screen-colour sessions of one platform thread.
pub(crate) struct ScreenColorSampler {
    queue: Arc<Mutex<VecDeque<(u64, ScreenColorSample)>>>,
    pending: RefCell<HashMap<u64, ScreenColorCompletion>>,
    sessions: RefCell<HashMap<u64, Session>>,
    next: Cell<u64>,
    wake: EventLoopWaker,
    backend: Box<dyn Backend>,
}

impl ScreenColorSampler {
    pub(crate) fn new(wake: EventLoopWaker) -> Self {
        Self::with_backend(wake, Box::new(NativeBackend))
    }

    fn with_backend(wake: EventLoopWaker, backend: Box<dyn Backend>) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            pending: RefCell::new(HashMap::new()),
            sessions: RefCell::new(HashMap::new()),
            next: Cell::new(0),
            wake,
            backend,
        }
    }

    fn sink(&self) -> CompletionSink {
        CompletionSink {
            queue: Arc::clone(&self.queue),
            wake: Arc::clone(&self.wake),
        }
    }

    /// Starts (or replaces) a pick. Returns before any native work completes.
    pub(crate) fn start(&self, done: ScreenColorCompletion) {
        // Native pickers are modal; a new request supersedes the previous one
        // and that request's completion still gets its single answer.
        self.cancel_active();
        let request = self.next.get().wrapping_add(1);
        self.next.set(request);
        self.pending.borrow_mut().insert(request, done);
        match self.backend.start(request, self.sink()) {
            Ok(session) => {
                self.sessions.borrow_mut().insert(request, session);
            }
            Err(sample) => self.sink().deliver(request, sample),
        }
    }

    /// Completions to run on the platform thread, after the shell borrow ends.
    /// Each session is dropped after its completion runs, not inside the native
    /// callback that queued the result.
    pub(crate) fn take_deferred_callbacks(&self) -> Vec<Box<dyn FnOnce()>> {
        let drained: Vec<(u64, ScreenColorSample)> = lock(&self.queue).drain(..).collect();
        let mut work: Vec<Box<dyn FnOnce()>> = Vec::with_capacity(drained.len());
        for (request, sample) in drained {
            let session = self.sessions.borrow_mut().remove(&request);
            if let Some(done) = self.pending.borrow_mut().remove(&request) {
                work.push(Box::new(move || {
                    done(sample);
                    drop(session);
                }));
            }
        }
        work
    }

    /// Cancels every in-flight session, answering each pending request once.
    pub(crate) fn cancel_all(&self) {
        self.cancel_active();
    }

    fn cancel_active(&self) {
        lock(&self.queue).clear();
        drop(std::mem::take(&mut *self.sessions.borrow_mut()));
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        for (_, done) in pending {
            done(ScreenColorSample::Cancelled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_platform::ScreenColor;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Sinks(Arc<Mutex<Vec<(u64, CompletionSink)>>>);
    impl Sinks {
        fn take_one(&self) -> (u64, CompletionSink) {
            self.0.lock().unwrap().remove(0)
        }
    }

    struct TestBackend {
        sinks: Sinks,
        immediate: Option<ScreenColorSample>,
    }
    impl Backend for TestBackend {
        fn start(&self, request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
            if let Some(sample) = self.immediate.clone() {
                return Err(sample);
            }
            self.sinks.0.lock().unwrap().push((request, sink));
            Ok(Session::worker(Arc::new(AtomicBool::new(false))))
        }
    }

    fn fixture(
        wakes: Arc<Mutex<u32>>,
        immediate: Option<ScreenColorSample>,
    ) -> (ScreenColorSampler, Sinks) {
        let sinks = Sinks::default();
        let wake = Arc::new(move || *wakes.lock().unwrap() += 1) as EventLoopWaker;
        let sampler = ScreenColorSampler::with_backend(
            wake,
            Box::new(TestBackend {
                sinks: sinks.clone(),
                immediate,
            }),
        );
        (sampler, sinks)
    }

    fn record() -> (Arc<Mutex<Vec<ScreenColorSample>>>, ScreenColorCompletion) {
        let results = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&results);
        (
            results,
            Box::new(move |sample| observed.lock().unwrap().push(sample)),
        )
    }

    #[test]
    fn a_request_returns_before_the_backend_answers() {
        let (sampler, _) = fixture(Arc::new(Mutex::new(0)), None);
        let (results, done) = record();
        sampler.start(done);
        assert!(results.lock().unwrap().is_empty());
        assert!(sampler.take_deferred_callbacks().is_empty());
    }

    #[test]
    fn results_are_delivered_once_on_the_deferred_pass_and_wake_the_loop() {
        let wakes = Arc::new(Mutex::new(0));
        let (sampler, sinks) = fixture(Arc::clone(&wakes), None);
        let (results, done) = record();
        sampler.start(done);
        let (request, sink) = sinks.take_one();
        assert_eq!(request, 1);
        let color = ScreenColor::from_rgba8(10, 20, 30, 255);
        sink.deliver(request, ScreenColorSample::Picked(color));
        assert_eq!(*wakes.lock().unwrap(), 1);
        assert!(results.lock().unwrap().is_empty());
        for work in sampler.take_deferred_callbacks() {
            work();
        }
        assert_eq!(
            *results.lock().unwrap(),
            vec![ScreenColorSample::Picked(color)]
        );
        sink.deliver(request, ScreenColorSample::Cancelled);
        for work in sampler.take_deferred_callbacks() {
            work();
        }
        assert_eq!(results.lock().unwrap().len(), 1);
        assert!(sampler.sessions.borrow().is_empty());
    }

    #[test]
    fn a_new_request_supersedes_the_previous_one() {
        let (sampler, _) = fixture(Arc::new(Mutex::new(0)), None);
        let (first, done) = record();
        sampler.start(done);
        let (second, done) = record();
        sampler.start(done);
        assert_eq!(*first.lock().unwrap(), vec![ScreenColorSample::Cancelled]);
        assert!(second.lock().unwrap().is_empty());
        assert_eq!(sampler.sessions.borrow().len(), 1);
    }

    #[test]
    fn an_unsupported_backend_still_answers_through_the_queue() {
        let (sampler, _) = fixture(
            Arc::new(Mutex::new(0)),
            Some(ScreenColorSample::Unsupported("no backend".into())),
        );
        let (results, done) = record();
        sampler.start(done);
        assert!(results.lock().unwrap().is_empty());
        for work in sampler.take_deferred_callbacks() {
            work();
        }
        assert_eq!(
            *results.lock().unwrap(),
            vec![ScreenColorSample::Unsupported("no backend".into())]
        );
    }

    #[test]
    fn shutdown_cancels_every_pending_request_and_drops_workers() {
        let (sampler, sinks) = fixture(Arc::new(Mutex::new(0)), None);
        let (results, done) = record();
        sampler.start(done);
        let (_, sink) = sinks.take_one();
        sampler.cancel_all();
        assert_eq!(*results.lock().unwrap(), vec![ScreenColorSample::Cancelled]);
        assert!(sampler.sessions.borrow().is_empty());
        assert!(sampler.pending.borrow().is_empty());
        sink.deliver(1, ScreenColorSample::Cancelled);
        assert!(sampler.take_deferred_callbacks().is_empty());
    }
}
