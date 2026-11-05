//! The per-session pump thread: it owns every FFmpeg handle for one player.
use super::session::Engine;
use super::{Abort, TICK, initialize};
use crate::types::SharedPlayer;
use crate::{MediaError, MediaErrorKind, MediaResult, MediaSource, PlayerOptions, RequestId};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// A command routed from the context worker to one session's pump thread.
pub(crate) enum Task {
    Open {
        id: RequestId,
        source: MediaSource,
        cancellation: crate::CancellationToken,
    },
    Play {
        id: RequestId,
    },
    Pause {
        id: RequestId,
    },
    Seek {
        id: RequestId,
        request: crate::SeekRequest,
    },
    Tracks {
        id: RequestId,
        tracks: Vec<String>,
    },
    Rate {
        id: RequestId,
        rate: f64,
    },
    Volume {
        id: RequestId,
        volume: f64,
    },
    /// The context is shutting down or the last handle was dropped.
    Stop,
}

/// Owns the pump thread. Dropping the sender ends the session.
pub(crate) struct Pump {
    sender: mpsc::Sender<Task>,
    thread: Option<JoinHandle<()>>,
    abort: Arc<Abort>,
}

impl Pump {
    pub(crate) fn spawn(
        shared: Arc<SharedPlayer>,
        options: PlayerOptions,
        stopping: Arc<AtomicBool>,
        live: Arc<AtomicUsize>,
    ) -> MediaResult<Self> {
        initialize()?;
        let (sender, receiver) = mpsc::channel();
        live.fetch_add(1, Ordering::AcqRel);
        let abort = Arc::new(Abort::new());
        let engine_abort = abort.clone();
        let counted = live.clone();
        let thread = thread::Builder::new()
            .name("cove-media-pump".into())
            .spawn(move || {
                let guard = Leave(counted);
                run(shared, options, receiver, stopping, engine_abort);
                drop(guard);
            })
            .map_err(|error| {
                live.fetch_sub(1, Ordering::AcqRel);
                MediaError::new(MediaErrorKind::Initialization, error.to_string())
            })?;
        Ok(Self {
            sender,
            thread: Some(thread),
            abort,
        })
    }

    pub(crate) fn send(&self, task: Task) -> bool {
        self.sender.send(task).is_ok()
    }

    /// Aborts a blocking backend call; the pump thread then exits.
    pub(crate) fn abort(&self) -> &Arc<Abort> {
        &self.abort
    }
}

impl Drop for Pump {
    fn drop(&mut self) {
        // Unblocks a pending read so the detached thread can observe the closed
        // channel; joining here would stall the thread that dropped the context.
        self.abort.stop();
        let _ = self.thread.take();
    }
}

/// Counts the threads that still owe the context a shutdown (`wait_closed`).
pub(crate) struct Leave(Arc<AtomicUsize>);
impl Drop for Leave {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn run(
    shared: Arc<SharedPlayer>,
    options: PlayerOptions,
    receiver: mpsc::Receiver<Task>,
    stopping: Arc<AtomicBool>,
    abort: Arc<Abort>,
) {
    let mut engine = Engine::new(shared, options, abort);
    loop {
        let task = match receiver.recv_timeout(TICK) {
            Ok(task) => Some(task),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => Some(Task::Stop),
        };
        let mut stop = matches!(task, Some(Task::Stop));
        if let Some(task) = task.filter(|task| !matches!(task, Task::Stop)) {
            engine.handle(task);
        }
        for _ in 0..31 {
            match receiver.try_recv() {
                Ok(Task::Stop) => stop = true,
                Ok(task) => engine.handle(task),
                Err(_) => break,
            }
        }
        if stop || stopping.load(Ordering::Acquire) {
            break;
        }
        engine.tick();
    }
    engine.shutdown();
}
