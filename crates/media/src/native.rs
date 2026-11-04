use crate::gst::GstVideoEndpoint;
use crate::types::SharedPlayer;
use crate::*;
use gst::prelude::*;
use gstreamer as gst;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(5);

struct ContextInner {
    sender: mpsc::SyncSender<Command>,
    closing: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
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
        crate::gst::initialize()?;
        let (sender, receiver) = mpsc::sync_channel(options.command_capacity);
        let closing = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let worker_closing = closing.clone();
        let worker_done = done.clone();
        let handle = thread::Builder::new()
            .name("cove-media".into())
            .spawn(move || {
                let context = gst::glib::MainContext::new();
                let result =
                    context.with_thread_default(|| run(receiver, &worker_closing, &context));
                if let Err(error) = result {
                    cove_logging::LogSystem::logger("com.cove.media").log(
                        &cove_logging::LogEvent::new(cove_logging::Level::Error, error.to_string()),
                    );
                }
                worker_done.store(true, Ordering::Release);
            })
            .map_err(|e| MediaError::new(MediaErrorKind::Initialization, e.to_string()))?;
        Ok(Self(Arc::new(ContextInner {
            sender,
            closing,
            done,
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
        self.0
            .sender
            .try_send(Command::Create {
                id,
                shared: shared.clone(),
                options,
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
    pub fn has_element(&self, factory: &str) -> bool {
        !factory.contains('\0') && gst::ElementFactory::find(factory).is_some()
    }
    pub fn backend_version(&self) -> String {
        gst::version_string().to_string()
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
        while !self.0.done.load(Ordering::Acquire) {
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
/// Clones refer to one playback session. The last clone schedules nonblocking close.
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
        self.submit(Operation::State(gst::State::Playing))
    }
    pub fn pause(&self) -> MediaResult<RequestId> {
        self.submit(Operation::State(gst::State::Paused))
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
    /// Query without acknowledging. Completed results remain until take_result is called.
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
    State(gst::State),
    Seek(SeekRequest),
    Tracks(Vec<String>),
    Rate(f64),
    Volume(f64),
    Close,
}
enum Command {
    Create {
        id: u64,
        shared: Arc<SharedPlayer>,
        options: PlayerOptions,
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
fn backend_error(error: impl std::fmt::Display) -> MediaError {
    MediaError::new(MediaErrorKind::Decode, error.to_string())
}

fn run(receiver: mpsc::Receiver<Command>, closing: &AtomicBool, context: &gst::glib::MainContext) {
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
        for _ in 0..32 {
            if !context.pending() {
                break;
            }
            context.iteration(false);
        }
        players.retain(|_, session| {
            if session.shared.abandoned.load(Ordering::Acquire) {
                session.close();
                false
            } else {
                session.poll();
                true
            }
        });
    }
    // Accepted commands still in the queue must obtain a terminal result.
    for command in receiver.try_iter() {
        match command {
            Command::Create {
                id,
                shared,
                options,
                timeout,
            } => {
                players.insert(id, Session::new(shared, options, timeout));
            }
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
        session.close();
    }
}
fn dispatch(command: Command, players: &mut BTreeMap<u64, Session>) {
    match command {
        Command::Create {
            id,
            shared,
            options,
            timeout,
        } => {
            players.insert(id, Session::new(shared, options, timeout));
        }
        Command::Operation {
            player,
            request,
            operation,
        } => {
            if let Some(session) = players.get_mut(&player) {
                let opening = matches!(&operation, Operation::Open(_));
                if let Err(error) = session.execute(request, operation) {
                    if opening {
                        session.fail(error.clone());
                    }
                    session
                        .shared
                        .finish(request, CommandOutcome::Failed(error));
                } else {
                    session
                        .deadlines
                        .insert(request, Instant::now() + session.timeout);
                }
            }
        }
    }
}

struct Session {
    shared: Arc<SharedPlayer>,
    options: PlayerOptions,
    pipeline: Option<gst::Pipeline>,
    endpoint: Option<GstVideoEndpoint>,
    source_cancel: Arc<Mutex<CancellationToken>>,
    opening: Option<RequestId>,
    state_request: Option<(RequestId, gst::State)>,
    seeking: Option<(RequestId, u64)>,
    track_selection: Option<(RequestId, Vec<String>, Option<gst::Seqnum>)>,
    selection_preference: Arc<Mutex<Option<Vec<String>>>>,
    confirmed_selection: Option<gst::Seqnum>,
    selection_refresh: Option<MediaTime>,
    last_position_update: Instant,
    timeout: Duration,
    deadlines: BTreeMap<RequestId, Instant>,
    live: bool,
}
impl Session {
    fn new(shared: Arc<SharedPlayer>, options: PlayerOptions, timeout: Duration) -> Self {
        Self {
            shared,
            options,
            pipeline: None,
            endpoint: None,
            source_cancel: Arc::new(Mutex::new(CancellationToken::default())),
            opening: None,
            state_request: None,
            seeking: None,
            track_selection: None,
            selection_preference: Arc::new(Mutex::new(None)),
            confirmed_selection: None,
            selection_refresh: None,
            last_position_update: Instant::now(),
            timeout,
            deadlines: BTreeMap::new(),
            live: false,
        }
    }
    fn execute(&mut self, id: RequestId, operation: Operation) -> MediaResult<()> {
        if self
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state
            == PlaybackState::Closed
        {
            return if matches!(operation, Operation::Close) {
                self.complete(id);
                Ok(())
            } else {
                Err(closed())
            };
        }
        match operation {
            Operation::Open(source) => self.open(id, source),
            Operation::Close => {
                self.close_preserving(Some(id));
                self.complete(id);
                Ok(())
            }
            Operation::State(target) => {
                self.require_pipeline()?
                    .set_state(target)
                    .map_err(backend_error)?;
                if let Some((previous, _)) = self.state_request.take() {
                    self.shared.finish(previous, CommandOutcome::Superseded);
                }
                self.state_request = Some((id, target));
                self.shared.update(|s| {
                    s.requested = if target == gst::State::Playing {
                        PlaybackIntent::Playing
                    } else {
                        PlaybackIntent::Paused
                    }
                });
                Ok(())
            }
            Operation::Seek(request) => self.seek(id, request, None),
            Operation::Rate(rate) => {
                let position = self
                    .require_pipeline()?
                    .query_position::<gst::ClockTime>()
                    .ok_or_else(|| {
                        MediaError::new(
                            MediaErrorKind::Unsupported,
                            "cannot change rate while the current position is unknown",
                        )
                    })?;
                self.seek(
                    id,
                    SeekRequest::exact(MediaTime(position.nseconds())),
                    Some(rate),
                )
            }
            Operation::Volume(volume) => {
                if let Some(pipeline) = &self.pipeline {
                    pipeline.set_property("volume", volume);
                }
                self.shared.update(|s| s.volume = volume);
                self.complete(id);
                Ok(())
            }
            Operation::Tracks(tracks) => {
                if self.opening.is_some()
                    || self.seeking.is_some()
                    || self.track_selection.is_some()
                {
                    return Err(MediaError::new(
                        MediaErrorKind::Busy,
                        "wait for the pending media operation before selecting tracks",
                    ));
                }
                if tracks.is_empty() {
                    return Err(MediaError::new(
                        MediaErrorKind::InvalidInput,
                        "select at least one track",
                    ));
                }
                let known = self
                    .shared
                    .snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .tracks
                    .clone();
                if tracks
                    .iter()
                    .any(|id| !known.iter().any(|track| &track.id == id))
                {
                    return Err(MediaError::new(
                        MediaErrorKind::InvalidInput,
                        "selected track is not in the current collection",
                    ));
                }
                if !known.iter().any(|track| {
                    tracks.contains(&track.id)
                        && matches!(track.kind, TrackKind::Audio | TrackKind::Video)
                }) {
                    return Err(MediaError::new(
                        MediaErrorKind::Unsupported,
                        "playback requires an audio or video track",
                    ));
                }
                let selected = self
                    .shared
                    .snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .selected_tracks
                    .clone();
                if tracks
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != tracks.len()
                {
                    return Err(MediaError::new(
                        MediaErrorKind::InvalidInput,
                        "track selection contains duplicates",
                    ));
                }
                if self.track_selection.is_none()
                    && tracks.len() == selected.len()
                    && tracks.iter().all(|id| selected.contains(id))
                {
                    self.complete(id);
                    return Ok(());
                }
                let pipeline = self.require_pipeline()?;
                let (_, state, _) = pipeline.state(gst::ClockTime::ZERO);
                let mut query = gst::query::Seeking::new(gst::Format::Time);
                let seekable = pipeline.query(&mut query) && query.result().0;
                let refresh = if seekable {
                    Some(pipeline.query_position::<gst::ClockTime>().ok_or_else(|| {
                        MediaError::new(
                            MediaErrorKind::Unsupported,
                            "cannot refresh track selection without a known position",
                        )
                    })?)
                } else if state == gst::State::Paused {
                    return Err(MediaError::new(
                        MediaErrorKind::Unsupported,
                        "paused track changes require a seekable source; resume a live source before changing tracks",
                    ));
                } else {
                    None
                };
                if let Some(position) = refresh {
                    if let Err(error) =
                        self.restart_tracks(id, tracks, MediaTime(position.nseconds()))
                    {
                        self.fail(error.clone());
                        return Err(error);
                    }
                    return Ok(());
                }
                *self
                    .selection_preference
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(tracks.clone());
                let event = gst::event::SelectStreams::new(tracks.iter().map(String::as_str));
                let sequence = event.seqnum();
                if !self.require_pipeline()?.send_event(event) {
                    return Err(MediaError::new(
                        MediaErrorKind::Unsupported,
                        "pipeline refused track selection",
                    ));
                }
                self.track_request(id, tracks, Some(sequence));
                Ok(())
            }
        }
    }
    fn require_pipeline(&self) -> MediaResult<&gst::Pipeline> {
        self.pipeline.as_ref().ok_or_else(|| {
            MediaError::new(
                MediaErrorKind::InvalidInput,
                "open media before controlling playback",
            )
        })
    }
    fn open(&mut self, id: RequestId, source: MediaSource) -> MediaResult<()> {
        self.stop_pipeline();
        self.supersede_pending();
        let endpoint = GstVideoEndpoint::new(self.shared.output.clone())?;
        let pipeline = gst::ElementFactory::make("playbin3")
            .build()
            .map_err(backend_error)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| {
                MediaError::new(MediaErrorKind::Initialization, "playbin3 is not a pipeline")
            })?;
        *self
            .selection_preference
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        let preference = self.selection_preference.clone();
        let decodebin = pipeline.by_name("uridecodebin3").ok_or_else(|| {
            MediaError::new(
                MediaErrorKind::Initialization,
                "playbin3 has no uridecodebin3",
            )
        })?;
        decodebin.connect("select-stream", false, move |values| {
            let choice = values[2]
                .get::<gst::Stream>()
                .ok()
                .and_then(|stream| stream.stream_id())
                .and_then(|id| {
                    preference
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .as_ref()
                        .map(|tracks| i32::from(tracks.iter().any(|track| track == id.as_str())))
                })
                .unwrap_or(-1);
            Some(choice.to_value())
        });
        pipeline.set_property("video-sink", endpoint.element());
        let audio = audio_sink(self.options.audio)?;
        endpoint.attach_audio(&audio)?;
        pipeline.set_property("audio-sink", &audio);
        let cancel = Arc::new(Mutex::new(CancellationToken::default()));
        let uri = match source {
            MediaSource::File(path) => {
                let path = std::fs::canonicalize(path)
                    .map_err(|e| MediaError::new(MediaErrorKind::Source, e.to_string()))?;
                gst::glib::filename_to_uri(&path, None)
                    .map_err(|e| MediaError::new(MediaErrorKind::Source, e.to_string()))?
                    .to_string()
            }
            MediaSource::Uri(uri) => {
                crate::source::validate_uri(&uri)?;
                uri
            }
            MediaSource::Bytes(bytes) => {
                install_input(
                    &pipeline,
                    Box::new(std::io::Cursor::new(bytes)),
                    cancel.clone(),
                );
                "appsrc://cove-media".into()
            }
            MediaSource::Input(input) => {
                install_input(&pipeline, input, cancel.clone());
                "appsrc://cove-media".into()
            }
        };
        pipeline.set_property("uri", &uri);
        let volume = self
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .volume;
        pipeline.set_property("volume", volume);
        let shared = Arc::downgrade(&self.shared);
        pipeline.connect("element-setup", false, move |values| {
            if let Some(shared) = shared.upgrade()
                && let Ok(element) = values[1].get::<gst::Element>()
                && let Some(factory) = element.factory()
                && factory
                    .metadata("klass")
                    .is_some_and(|klass| klass.contains("Decoder"))
            {
                let name = factory.name().to_string();
                shared.update(|s| {
                    if !s.decoder_elements.contains(&name) {
                        s.decoder_elements.push(name);
                    }
                });
            }
            None
        });
        let generation = endpoint.generation();
        self.shared.update(|s| {
            let source_generation = s
                .source_generation
                .checked_add(1)
                .expect("source generation exhausted");
            *s = PlayerSnapshot {
                state: PlaybackState::Opening,
                requested: if self.options.autoplay {
                    PlaybackIntent::Playing
                } else {
                    PlaybackIntent::Paused
                },
                source_generation,
                volume,
                frame_generation: generation,
                ..PlayerSnapshot::default()
            };
        });
        self.pipeline = Some(pipeline);
        self.endpoint = Some(endpoint);
        self.source_cancel = cancel;
        self.opening = Some(id);
        self.live = self
            .require_pipeline()?
            .set_state(if self.options.autoplay {
                gst::State::Playing
            } else {
                gst::State::Paused
            })
            .map_err(backend_error)?
            == gst::StateChangeSuccess::NoPreroll;
        Ok(())
    }
    fn seek(&mut self, id: RequestId, request: SeekRequest, rate: Option<f64>) -> MediaResult<()> {
        let pipeline = self.require_pipeline()?;
        let mut query = gst::query::Seeking::new(gst::Format::Time);
        if !pipeline.query(&mut query) || !query.result().0 {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "current media is not time-seekable",
            ));
        }
        if self.opening.is_some() || self.track_selection.is_some() {
            return Err(MediaError::new(
                MediaErrorKind::Busy,
                "media is opening or changing tracks",
            ));
        }
        let rate = rate.unwrap_or_else(|| {
            self.shared
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .rate
        });
        let seqnum = gst::Seqnum::next();
        let flags = gst::SeekFlags::FLUSH
            | if request.accuracy == SeekAccuracy::Exact {
                gst::SeekFlags::ACCURATE
            } else {
                gst::SeekFlags::KEY_UNIT
            };
        let event = gst::event::Seek::builder(
            rate,
            flags,
            gst::SeekType::Set,
            gst::ClockTime::from_nseconds(request.position.0),
            gst::SeekType::None,
            gst::ClockTime::NONE,
        )
        .seqnum(seqnum)
        .build();
        let endpoint = self.endpoint.as_ref().ok_or_else(|| {
            MediaError::new(MediaErrorKind::Initialization, "missing video endpoint")
        })?;
        let previous_frame = self.shared.output.latest();
        let generation = endpoint.prepare_seek(seqnum)?;
        if !pipeline.send_event(event) {
            endpoint.reject_seek();
            if let Some((previous, _)) = self.seeking.take() {
                self.shared.finish(previous, CommandOutcome::Superseded);
            }
            self.shared.update(|s| {
                s.seeking = false;
                s.frame_generation = generation;
            });
            if self.shared.output.latest().is_none()
                && let Some(previous) = previous_frame
            {
                let mut frame = (*previous).clone();
                frame.generation = generation;
                self.shared.output.publish(frame);
            }
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "pipeline refused seek",
            ));
        }
        if let Some((previous, _)) = self.seeking.take() {
            self.shared.finish(previous, CommandOutcome::Superseded);
        }
        self.seeking = Some((id, generation));
        self.shared.update(|s| {
            s.seeking = true;
            s.frame_generation = generation;
            s.rate = rate;
        });
        Ok(())
    }
    fn complete(&self, id: RequestId) {
        let position = self
            .pipeline
            .as_ref()
            .and_then(|p| p.query_position::<gst::ClockTime>())
            .map(|t| MediaTime(t.nseconds()));
        self.shared
            .finish(id, CommandOutcome::Completed { position });
    }
    fn supersede_pending(&mut self) {
        for id in [
            self.opening.take(),
            self.state_request.take().map(|p| p.0),
            self.seeking.take().map(|p| p.0),
            self.track_selection.take().map(|p| p.0),
        ]
        .into_iter()
        .flatten()
        {
            self.shared.finish(id, CommandOutcome::Superseded);
        }
    }
    fn stop_pipeline(&mut self) {
        self.selection_refresh = None;
        self.confirmed_selection = None;
        self.source_cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cancel();
        if let Some(pipeline) = self.pipeline.take()
            && let Err(error) = pipeline.set_state(gst::State::Null)
        {
            cove_logging::LogSystem::logger("com.cove.media").log(&cove_logging::LogEvent::new(
                cove_logging::Level::Warning,
                format!("pipeline stop failed: {error}"),
            ));
        }
        self.endpoint = None;
    }
    fn close(&mut self) {
        self.close_preserving(None);
    }
    fn close_preserving(&mut self, keep: Option<RequestId>) {
        self.stop_pipeline();
        self.supersede_pending();
        self.shared.output.close();
        self.shared.update(|s| {
            s.state = PlaybackState::Closed;
            s.seeking = false;
        });
        let pending: Vec<_> = self
            .shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|(id, value)| value.is_none().then_some(*id))
            .collect();
        for id in pending {
            if Some(id) != keep {
                self.shared.finish(id, CommandOutcome::Failed(closed()));
            }
        }
    }
    fn fail(&mut self, error: MediaError) {
        let pending = [
            self.opening.take(),
            self.state_request.take().map(|p| p.0),
            self.seeking.take().map(|p| p.0),
            self.track_selection.take().map(|p| p.0),
        ];
        self.opening = None;
        self.state_request = None;
        self.seeking = None;
        self.track_selection = None;
        self.stop_pipeline();
        self.shared.update(|s| {
            s.state = PlaybackState::Failed;
            s.seeking = false;
            s.last_error = Some(error.clone());
        });
        for id in pending.into_iter().flatten() {
            self.shared
                .finish(id, CommandOutcome::Failed(error.clone()));
        }
    }
    fn poll(&mut self) {
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
            self.deadlines.clear();
            return;
        }
        if let Some(error) = self
            .endpoint
            .as_ref()
            .and_then(GstVideoEndpoint::take_error)
        {
            self.fail(error);
            return;
        }
        let Some(pipeline) = self.pipeline.clone() else {
            return;
        };
        if let Some(bus) = pipeline.bus() {
            for _ in 0..64 {
                let Some(message) = bus.pop() else { break };
                match message.view() {
                    gst::MessageView::Error(error) => {
                        self.fail(MediaError::new(
                            MediaErrorKind::Decode,
                            format!(
                                "{}: {} ({})",
                                error
                                    .src()
                                    .map(|s| s.path_string().to_string())
                                    .unwrap_or_default(),
                                error.error(),
                                error.debug().map(|s| s.to_string()).unwrap_or_default()
                            ),
                        ));
                        return;
                    }
                    gst::MessageView::Eos(_) => {
                        self.shared.update(|s| s.state = PlaybackState::Ended);
                    }
                    gst::MessageView::Buffering(buffering) => {
                        let percent = buffering.percent().clamp(0, 100) as u8;
                        self.shared.update(|s| s.buffering_percent = Some(percent));
                        let requested = self
                            .shared
                            .snapshot
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .requested;
                        if self.seeking.is_none() && !self.live {
                            let _ = pipeline.set_state(
                                if percent < 100 || requested != PlaybackIntent::Playing {
                                    gst::State::Paused
                                } else {
                                    gst::State::Playing
                                },
                            );
                        }
                    }
                    gst::MessageView::StreamCollection(collection) => {
                        let collection = collection.stream_collection();
                        let tracks = collection
                            .iter()
                            .filter_map(|stream| {
                                let id = stream.stream_id()?.to_string();
                                let kind = stream.stream_type();
                                Some(TrackInfo {
                                    id,
                                    kind: if kind.contains(gst::StreamType::VIDEO) {
                                        TrackKind::Video
                                    } else if kind.contains(gst::StreamType::AUDIO) {
                                        TrackKind::Audio
                                    } else if kind.contains(gst::StreamType::TEXT) {
                                        TrackKind::Subtitle
                                    } else {
                                        TrackKind::Other
                                    },
                                    caps: stream.caps().map(|caps| caps.to_string()),
                                })
                            })
                            .collect();
                        self.shared.update(|s| s.tracks = tracks);
                    }
                    gst::MessageView::StreamsSelected(selected) => {
                        let ids = selected
                            .streams()
                            .filter_map(|s| s.stream_id().map(|s| s.to_string()))
                            .collect();
                        self.shared.update(|s| s.selected_tracks = ids);
                        self.complete_track_selection(message.seqnum());
                    }
                    _ => {}
                }
            }
        }
        let (_, current, pending_state) = pipeline.state(gst::ClockTime::ZERO);
        if let Some(position) = self.selection_refresh
            && pending_state == gst::State::VoidPending
            && matches!(current, gst::State::Paused | gst::State::Playing)
            && self.track_selection.as_ref().is_some_and(|selection| {
                selection.2.is_some() && selection.2 == self.confirmed_selection
            })
        {
            let selection = self.track_selection.take().expect("pending track refresh");
            self.selection_refresh = None;
            let result = self.seek(selection.0, SeekRequest::exact(position), None);
            self.track_selection = Some(selection);
            if let Err(error) = result {
                self.fail(error);
                return;
            }
        }
        let (_, state, pending) = pipeline.state(gst::ClockTime::ZERO);
        if pending == gst::State::VoidPending
            && matches!(state, gst::State::Paused | gst::State::Playing)
        {
            self.shared.update(|s| {
                if s.state != PlaybackState::Ended {
                    s.state = if state == gst::State::Playing {
                        PlaybackState::Playing
                    } else {
                        PlaybackState::Paused
                    };
                }
            });
            if let Some(id) = self.opening.take() {
                self.complete(id);
            }
            if self
                .state_request
                .as_ref()
                .is_some_and(|(_, target)| *target == state)
                && let Some((id, _)) = self.state_request.take()
            {
                self.complete(id);
            }
        }
        if let Some((id, generation)) = self.seeking {
            let has_video = {
                let state = self
                    .shared
                    .snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.tracks.iter().any(|track| {
                    track.kind == TrackKind::Video
                        && (state.selected_tracks.is_empty()
                            || state.selected_tracks.contains(&track.id))
                })
            };
            if let Some(position) = self
                .endpoint
                .as_ref()
                .and_then(|e| e.seek_witness(has_video, generation))
            {
                let position = position.or_else(|| {
                    pipeline
                        .query_position::<gst::ClockTime>()
                        .map(|t| MediaTime(t.nseconds()))
                });
                self.seeking = None;
                if let Some(endpoint) = &self.endpoint {
                    endpoint.acknowledge_seek();
                }
                self.shared.update(|s| {
                    s.seeking = false;
                    s.position = position;
                });
                if self
                    .track_selection
                    .as_ref()
                    .is_some_and(|selection| selection.0 == id)
                {
                    if let Some(sequence) = self
                        .track_selection
                        .as_ref()
                        .and_then(|selection| selection.2)
                    {
                        self.complete_track_selection(sequence);
                    }
                } else {
                    self.shared
                        .finish(id, CommandOutcome::Completed { position });
                }
            }
        }
        if self.last_position_update.elapsed() >= Duration::from_millis(50) {
            self.last_position_update = Instant::now();
            let position = pipeline
                .query_position::<gst::ClockTime>()
                .map(|t| MediaTime(t.nseconds()));
            let duration = pipeline
                .query_duration::<gst::ClockTime>()
                .map(|t| MediaTime(t.nseconds()));
            let mut query = gst::query::Seeking::new(gst::Format::Time);
            let range = if pipeline.query(&mut query) {
                let (seekable, start, end) = query.result();
                if seekable {
                    let start: Option<gst::ClockTime> = start.try_into().ok().flatten();
                    let end: Option<gst::ClockTime> = end.try_into().ok().flatten();
                    start.map(|s| {
                        (
                            MediaTime(s.nseconds()),
                            end.map(|t| MediaTime(t.nseconds())),
                        )
                    })
                } else {
                    None
                }
            } else {
                None
            };
            self.shared.update(|s| {
                s.position = position;
                s.duration = duration;
                s.seekable_range = range;
            });
        }
        if self.track_selection.is_some()
            && let Some(sequence) = self.confirmed_selection
        {
            self.complete_track_selection(sequence);
        }
    }
    fn track_request(&mut self, id: RequestId, tracks: Vec<String>, sequence: Option<gst::Seqnum>) {
        if let Some((previous, _, _)) = self.track_selection.replace((id, tracks, sequence)) {
            self.shared.finish(previous, CommandOutcome::Superseded);
        }
    }
    fn restart_tracks(
        &mut self,
        id: RequestId,
        tracks: Vec<String>,
        position: MediaTime,
    ) -> MediaResult<()> {
        let pipeline = self.require_pipeline()?.clone();
        self.source_cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cancel();
        pipeline
            .set_state(gst::State::Ready)
            .map_err(backend_error)?;
        // READY has joined the old streaming tasks. Reuse the cooperative input
        // with a fresh read epoch; appsrc will seek it back to byte zero.
        *self
            .source_cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CancellationToken::default();
        if let Some(bus) = pipeline.bus() {
            bus.set_flushing(true);
            bus.set_flushing(false);
        }
        *self
            .selection_preference
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(tracks.clone());
        let generation = self
            .endpoint
            .as_ref()
            .ok_or_else(|| {
                MediaError::new(MediaErrorKind::Initialization, "missing video endpoint")
            })?
            .prepare_seek(gst::Seqnum::next())?;
        self.confirmed_selection = None;
        self.selection_refresh = Some(position);
        self.track_request(id, tracks, None);
        self.shared.update(|state| {
            state.state = PlaybackState::Opening;
            state.seeking = true;
            state.frame_generation = generation;
        });
        pipeline
            .set_state(gst::State::Paused)
            .map_err(backend_error)?;
        Ok(())
    }
    fn complete_track_selection(&mut self, sequence: gst::Seqnum) {
        self.confirmed_selection = Some(sequence);
        let selected = self
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .selected_tracks
            .clone();
        if let Some((_, requested, expected)) = self.track_selection.as_mut()
            && expected.is_none()
            && requested.len() == selected.len()
            && requested.iter().all(|id| selected.contains(id))
        {
            *expected = Some(sequence);
        }
        if self
            .track_selection
            .as_ref()
            .is_some_and(|(_, requested, expected)| {
                *expected == Some(sequence)
                    && requested.len() == selected.len()
                    && requested.iter().all(|id| selected.contains(id))
            })
            && self.selection_refresh.is_none()
            && !self.seeking.as_ref().is_some_and(|seek| {
                self.track_selection
                    .as_ref()
                    .is_some_and(|selection| seek.0 == selection.0)
            })
        {
            let requested = self
                .shared
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .requested;
            if requested == PlaybackIntent::Playing
                && let Some(pipeline) = &self.pipeline
            {
                let (_, current, pending) = pipeline.state(gst::ClockTime::ZERO);
                if current != gst::State::Playing || pending != gst::State::VoidPending {
                    if pending != gst::State::Playing
                        && let Err(error) = pipeline.set_state(gst::State::Playing)
                    {
                        self.fail(backend_error(error));
                    }
                    return;
                }
                self.shared
                    .update(|state| state.state = PlaybackState::Playing);
            }
            if let Some((request, _, _)) = self.track_selection.take() {
                self.complete(request);
            }
        }
    }
}

fn audio_sink(output: AudioOutput) -> MediaResult<gst::Element> {
    if output == AudioOutput::Silent {
        return gst::ElementFactory::make("fakesink")
            .property("sync", true)
            .build()
            .map_err(backend_error);
    }
    let names: &[&str] = if cfg!(any(target_os = "macos", target_os = "ios")) {
        &["osxaudiosink", "autoaudiosink"]
    } else if cfg!(target_os = "windows") {
        &[
            "wasapi2sink",
            "wasapisink",
            "directsoundsink",
            "autoaudiosink",
        ]
    } else if cfg!(target_os = "android") {
        &["aaudiosink", "openslessink", "autoaudiosink"]
    } else {
        &["pipewiresink", "pulsesink", "alsasink", "autoaudiosink"]
    };
    for name in names {
        if gst::ElementFactory::find(name).is_some() {
            return gst::ElementFactory::make(name)
                .build()
                .map_err(backend_error);
        }
    }
    Err(MediaError::new(
        MediaErrorKind::Initialization,
        "no platform audio sink is installed",
    ))
}

fn install_input(
    pipeline: &gst::Pipeline,
    input: Box<dyn MediaInput>,
    cancellation: Arc<Mutex<CancellationToken>>,
) {
    let input = Arc::new(Mutex::new(input));
    pipeline.connect("source-setup", false, move |values| {
        let cancellation = cancellation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Ok(source) = values[1].get::<gstreamer_app::AppSrc>() else {
            return None;
        };
        let (size, seekable) = {
            let input = input
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                input
                    .length()
                    .and_then(|n| i64::try_from(n).ok())
                    .unwrap_or(-1),
                input.is_seekable(),
            )
        };
        source.set_size(size);
        source.set_format(gst::Format::Bytes);
        source.set_stream_type(if seekable {
            gstreamer_app::AppStreamType::Seekable
        } else {
            gstreamer_app::AppStreamType::Stream
        });
        source.set_max_bytes(256 * 1024);
        let reader = input.clone();
        let cancel_read = cancellation.clone();
        let seeker = input.clone();
        let cancel_seek = cancellation.clone();
        source.set_callbacks(
            gstreamer_app::AppSrcCallbacks::builder()
                .need_data(move |source, hint| {
                    if cancel_read.is_cancelled() {
                        return;
                    }
                    let size = if hint == 0 || hint == u32::MAX {
                        64 * 1024
                    } else {
                        hint.min(64 * 1024) as usize
                    };
                    let mut bytes = vec![0; size];
                    let read = reader
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .read(&mut bytes, &cancel_read);
                    match read {
                        Ok(0) => {
                            let _ = source.end_of_stream();
                        }
                        Ok(count) if count <= bytes.len() => {
                            bytes.truncate(count);
                            let _ = source.push_buffer(gst::Buffer::from_mut_slice(bytes));
                        }
                        Ok(_) => gst::element_error!(
                            source,
                            gst::ResourceError::Read,
                            ["media input returned more bytes than requested"]
                        ),
                        Err(error) if error.kind == MediaErrorKind::Cancelled => {
                            let _ = source.end_of_stream();
                        }
                        Err(error) => {
                            gst::element_error!(source, gst::ResourceError::Read, ["{error}"])
                        }
                    }
                })
                .seek_data(move |_, offset| {
                    seeker
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .seek(offset, &cancel_seek)
                        .is_ok()
                })
                .build(),
        );
        None
    });
}
