//! Context and player control over one FFmpeg pump thread per player.
//!
//! This worker only routes commands, records results and enforces operation
//! deadlines; every raw `AV*` handle lives on the pump thread in [`crate::engine`].
use crate::engine::{Pump, Task};
use crate::types::SharedPlayer;
use crate::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::TICK;

struct ContextInner {
    sender: mpsc::SyncSender<Command>,
    closing: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    live: Arc<AtomicUsize>,
    thread: Mutex<Option<JoinHandle<()>>>,
    next_player: AtomicU64,
    result_capacity: usize,
    event_capacity: usize,
    operation_timeout: Duration,
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Release);
        // Dropping the last owner requests shutdown; it never joins a streaming/UI thread.
        self.thread
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

#[derive(Clone)]
pub struct MediaContext(Arc<ContextInner>);

impl MediaContext {
    pub fn new(options: ContextOptions) -> MediaResult<Self> {
        if options.command_capacity == 0
            || options.result_capacity == 0
            || options.event_capacity == 0
            || options.operation_timeout.is_zero()
            || Instant::now()
                .checked_add(options.operation_timeout)
                .is_none()
        {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "media queue limits must be positive",
            ));
        }
        crate::engine::initialize()?;
        let (sender, receiver) = mpsc::sync_channel(options.command_capacity);
        let closing = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let live = Arc::new(AtomicUsize::new(0));
        let worker_closing = closing.clone();
        let worker_done = done.clone();
        let handle = thread::Builder::new()
            .name("cove-media".into())
            .spawn(move || {
                run(receiver, &worker_closing);
                worker_done.store(true, Ordering::Release);
            })
            .map_err(|error| MediaError::new(MediaErrorKind::Initialization, error.to_string()))?;
        Ok(Self(Arc::new(ContextInner {
            sender,
            closing,
            done,
            live,
            thread: Mutex::new(Some(handle)),
            next_player: AtomicU64::new(1),
            result_capacity: options.result_capacity,
            event_capacity: options.event_capacity,
            operation_timeout: options.operation_timeout,
        })))
    }

    pub fn create_player(&self, options: PlayerOptions) -> MediaResult<Player> {
        if self.0.closing.load(Ordering::Acquire) {
            return Err(closed());
        }
        let id = self.0.next_player.fetch_add(1, Ordering::Relaxed);
        let shared = SharedPlayer::new(self.0.result_capacity, self.0.event_capacity);
        let pump = Pump::spawn(
            shared.clone(),
            options,
            self.0.closing.clone(),
            self.0.live.clone(),
        )?;
        self.0
            .sender
            .try_send(Command::Create {
                player: id,
                shared: shared.clone(),
                pump,
                timeout: self.0.operation_timeout,
            })
            .map_err(send_error)?;
        Ok(Player(Arc::new(PlayerLease {
            id,
            shared,
            context: self.0.clone(),
            closing: AtomicBool::new(false),
        })))
    }

    /// Whether FFmpeg can decode a codec of this name (`h264`, `opus`, …).
    pub fn has_decoder(&self, name: &str) -> bool {
        crate::engine::has_decoder(name)
    }

    /// The FFmpeg version compiled into this build.
    pub fn backend_version(&self) -> String {
        crate::engine::backend_version()
    }

    pub fn shutdown(&self) {
        self.0.closing.store(true, Ordering::Release);
    }

    /// Blocking shutdown wait for hosts/tests. Do not invoke on the UI thread.
    pub fn wait_closed(&self, timeout: Duration) -> MediaResult<()> {
        let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
            MediaError::new(
                MediaErrorKind::InvalidInput,
                "shutdown timeout out of range",
            )
        })?;
        while !self.0.done.load(Ordering::Acquire) || self.0.live.load(Ordering::Acquire) != 0 {
            if Instant::now() >= deadline {
                return Err(MediaError::new(
                    MediaErrorKind::Timeout,
                    "media shutdown timed out",
                ));
            }
            thread::sleep(TICK);
        }
        if let Some(handle) = self
            .0
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            handle.join().map_err(|_| {
                MediaError::new(MediaErrorKind::Initialization, "media worker panicked")
            })?;
        }
        Ok(())
    }
}

struct PlayerLease {
    id: u64,
    shared: Arc<SharedPlayer>,
    context: Arc<ContextInner>,
    closing: AtomicBool,
}

impl Drop for PlayerLease {
    fn drop(&mut self) {
        self.shared.abandoned.store(true, Ordering::Release);
    }
}

/// Clones refer to one playback session. The last clone schedules a nonblocking close.
#[derive(Clone)]
pub struct Player(Arc<PlayerLease>);

impl Player {
    pub fn snapshot(&self) -> PlayerSnapshot {
        self.0
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub fn video_output(&self) -> VideoOutput {
        self.0.shared.output.clone()
    }
    pub fn events_since(&self, cursor: u64) -> PlayerEvents {
        self.0.shared.events_since(cursor)
    }
    pub fn open(&self, source: MediaSource) -> MediaResult<RequestId> {
        self.submit(Operation::Open(source))
    }
    pub fn play(&self) -> MediaResult<RequestId> {
        self.submit(Operation::Play)
    }
    pub fn pause(&self) -> MediaResult<RequestId> {
        self.submit(Operation::Pause)
    }
    pub fn seek(&self, request: SeekRequest) -> MediaResult<RequestId> {
        if request.position.0 > i64::MAX as u64 {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "seek position exceeds the backend's signed time range",
            ));
        }
        self.submit(Operation::Seek(request))
    }
    pub fn select_tracks(&self, tracks: Vec<String>) -> MediaResult<RequestId> {
        self.submit(Operation::Tracks(tracks))
    }
    pub fn set_rate(&self, rate: f64) -> MediaResult<RequestId> {
        if !rate.is_finite() || !(0.1..=16.0).contains(&rate) {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "playback rate must be in 0.1..=16",
            ));
        }
        self.submit(Operation::Rate(rate))
    }
    pub fn set_volume(&self, volume: f64) -> MediaResult<RequestId> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "volume must be in 0..=1",
            ));
        }
        self.submit(Operation::Volume(volume))
    }
    pub fn close(&self) -> MediaResult<RequestId> {
        self.submit(Operation::Close)
    }
    /// Query without acknowledging. Completed results remain until `take_result`.
    pub fn result(&self, request: RequestId) -> MediaResult<Option<CommandOutcome>> {
        self.0
            .shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&request)
            .cloned()
            .ok_or_else(|| {
                MediaError::new(
                    MediaErrorKind::RequestUnknown,
                    "unknown or acknowledged request",
                )
            })
    }
    pub fn take_result(&self, request: RequestId) -> MediaResult<Option<CommandOutcome>> {
        let mut results = self
            .0
            .shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match results.get(&request) {
            Some(Some(_)) => Ok(results.remove(&request).flatten()),
            Some(None) => Ok(None),
            None => Err(MediaError::new(
                MediaErrorKind::RequestUnknown,
                "unknown or acknowledged request",
            )),
        }
    }
    /// Blocking result wait for non-UI consumers. This does not acknowledge the result.
    pub fn wait_result(
        &self,
        request: RequestId,
        timeout: Duration,
    ) -> MediaResult<CommandOutcome> {
        self.0.shared.wait_result(request, timeout)
    }

    fn submit(&self, operation: Operation) -> MediaResult<RequestId> {
        if self.0.context.closing.load(Ordering::Acquire) {
            return Err(closed());
        }
        let mut results = self
            .0
            .shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let closing = matches!(&operation, Operation::Close);
        let was_closing = self.0.closing.load(Ordering::Acquire);
        if was_closing && !closing {
            return Err(closed());
        }
        if results.len() >= self.0.shared.result_capacity {
            return Err(MediaError::new(
                MediaErrorKind::Busy,
                "acknowledge completed media requests before submitting more",
            ));
        }
        let id = RequestId(self.0.shared.next_request.fetch_add(1, Ordering::Relaxed));
        results.insert(id, None);
        if closing {
            self.0.closing.store(true, Ordering::Release);
        }
        if let Err(error) = self.0.context.sender.try_send(Command::Operation {
            player: self.0.id,
            request: id,
            operation,
        }) {
            results.remove(&id);
            if closing {
                self.0.closing.store(was_closing, Ordering::Release);
            }
            return Err(send_error(error));
        }
        Ok(id)
    }
}

enum Operation {
    Open(MediaSource),
    Play,
    Pause,
    Seek(SeekRequest),
    Tracks(Vec<String>),
    Rate(f64),
    Volume(f64),
    Close,
}

impl Operation {
    fn task(self, id: RequestId) -> Task {
        match self {
            Operation::Open(source) => Task::Open {
                id,
                source,
                cancellation: CancellationToken::default(),
            },
            Operation::Play => Task::Play { id },
            Operation::Pause => Task::Pause { id },
            Operation::Seek(request) => Task::Seek { id, request },
            Operation::Tracks(tracks) => Task::Tracks { id, tracks },
            Operation::Rate(rate) => Task::Rate { id, rate },
            Operation::Volume(volume) => Task::Volume { id, volume },
            // Close is handled by the worker before it reaches a pump.
            Operation::Close => unreachable!("close terminates the session at dispatch"),
        }
    }
}

enum Command {
    Create {
        player: u64,
        shared: Arc<SharedPlayer>,
        pump: Pump,
        timeout: Duration,
    },
    Operation {
        player: u64,
        request: RequestId,
        operation: Operation,
    },
}

fn send_error<T>(error: mpsc::TrySendError<T>) -> MediaError {
    match error {
        mpsc::TrySendError::Full(_) => {
            MediaError::new(MediaErrorKind::Busy, "media command queue is full")
        }
        mpsc::TrySendError::Disconnected(_) => closed(),
    }
}

fn closed() -> MediaError {
    MediaError::new(MediaErrorKind::Closed, "media context is closed")
}

fn run(receiver: mpsc::Receiver<Command>, closing: &AtomicBool) {
    let mut players: BTreeMap<u64, Session> = BTreeMap::new();
    while !closing.load(Ordering::Acquire) {
        match receiver.recv_timeout(TICK) {
            Ok(command) => dispatch(command, &mut players),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for _ in 0..32 {
            match receiver.try_recv() {
                Ok(command) => dispatch(command, &mut players),
                Err(_) => break,
            }
        }
        players.retain(|_, session| session.poll());
    }
    // Accepted commands still in the queue must obtain a terminal result.
    for command in receiver.try_iter() {
        match command {
            Command::Create { .. } => {}
            Command::Operation {
                player, request, ..
            } => {
                if let Some(session) = players.get(&player) {
                    session
                        .shared
                        .finish(request, CommandOutcome::Failed(closed()));
                }
            }
        }
    }
    for session in players.values_mut() {
        session.close(None);
    }
}

fn dispatch(command: Command, players: &mut BTreeMap<u64, Session>) {
    match command {
        Command::Create {
            player,
            shared,
            pump,
            timeout,
        } => {
            players.insert(
                player,
                Session {
                    shared,
                    pump: Some(pump),
                    timeout,
                    deadlines: BTreeMap::new(),
                    fatal: None,
                },
            );
        }
        Command::Operation {
            player,
            request,
            operation,
        } => {
            let Some(session) = players.get_mut(&player) else {
                return;
            };
            if let Some(fatal) = &session.fatal {
                session
                    .shared
                    .finish(request, CommandOutcome::Failed(fatal.clone()));
                return;
            }
            if matches!(&operation, Operation::Close) {
                session.close(Some(request));
                return;
            }
            let Some(pump) = &session.pump else {
                session
                    .shared
                    .finish(request, CommandOutcome::Failed(closed()));
                return;
            };
            if pump.send(operation.task(request)) {
                let Some(deadline) = Instant::now().checked_add(session.timeout) else {
                    session.fail(MediaError::new(
                        MediaErrorKind::InvalidInput,
                        "operation deadline out of range",
                    ));
                    return;
                };
                session.deadlines.insert(request, deadline);
            } else {
                session
                    .shared
                    .finish(request, CommandOutcome::Failed(closed()));
            }
        }
    }
}

struct Session {
    shared: Arc<SharedPlayer>,
    pump: Option<Pump>,
    timeout: Duration,
    deadlines: BTreeMap<RequestId, Instant>,
    /// Set once the session can no longer serve commands, with the reason to report.
    fatal: Option<MediaError>,
}

impl Session {
    /// Advances deadline bookkeeping. Returns false when the session is finished.
    fn poll(&mut self) -> bool {
        if self.shared.abandoned.load(Ordering::Acquire) {
            self.close(None);
            return false;
        }
        {
            let results = self
                .shared
                .results
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.deadlines
                .retain(|id, _| results.get(id).is_some_and(Option::is_none));
        }
        if self
            .deadlines
            .values()
            .any(|deadline| Instant::now() >= *deadline)
        {
            self.fail(MediaError::new(
                MediaErrorKind::Timeout,
                "backend did not confirm the media operation before its deadline",
            ));
            return false;
        }
        true
    }

    /// Ends the session after a backend timeout and unblocks its pump thread.
    fn fail(&mut self, error: MediaError) {
        if let Some(pump) = self.pump.take() {
            pump.abort().stop();
        }
        self.deadlines.clear();
        self.fatal = Some(error.clone());
        self.shared.update(|snapshot| {
            if snapshot.state != PlaybackState::Closed {
                snapshot.state = PlaybackState::Failed;
            }
            snapshot.seeking = false;
            snapshot.last_error = Some(error.clone());
        });
        for id in self.pending_requests() {
            self.shared
                .finish(id, CommandOutcome::Failed(error.clone()));
        }
    }

    fn close(&mut self, keep: Option<RequestId>) {
        if let Some(pump) = self.pump.take() {
            pump.abort().stop();
        }
        self.deadlines.clear();
        self.fatal = Some(closed());
        self.shared.output.close();
        if keep.is_some() || self.state() != PlaybackState::Failed {
            self.shared.update(|snapshot| {
                snapshot.state = PlaybackState::Closed;
                snapshot.seeking = false;
            });
        } else {
            // A timed-out session keeps its diagnosis when the context shuts it down.
            self.shared.update(|snapshot| snapshot.seeking = false);
        }
        for id in self.pending_requests() {
            if Some(id) != keep {
                self.shared.finish(id, CommandOutcome::Failed(closed()));
            }
        }
        if let Some(id) = keep {
            let position = self
                .shared
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .position;
            self.shared
                .finish(id, CommandOutcome::Completed { position });
        }
    }

    fn state(&self) -> PlaybackState {
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state
    }

    fn pending_requests(&self) -> Vec<RequestId> {
        self.shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|(id, value)| value.is_none().then_some(*id))
            .collect()
    }
}
