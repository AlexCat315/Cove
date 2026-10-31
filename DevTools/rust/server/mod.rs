//! WebSocket server with platform-independent protocol dispatch.
//!
//! The transport owns sockets; the host owns UI-thread scheduling. Protocol
//! state is confined to one thread by construction: the accept loop feeds a
//! channel and `pump` consumes it, so no lock guards a subscription.
pub mod config;
pub mod transport;

pub use config::{
    DevServerConfigurationError, DevToolsConfig, DevToolsHost, DevToolsHostScene, DevToolsLogFeed,
    HostMainExecutor, NodeSelectionClearHandler, NodeSelectionHandler, SceneSnapshotProvider,
};
pub use transport::{
    DevToolsConnection, DevToolsTransport, DevToolsTransportEvent, StdDevToolsTransport,
};

/// Swift's `NIODevToolsTransport`. SwiftNIO is not a workspace dependency, so
/// `StdDevToolsTransport` implements the same wire behaviour — HTTP upgrade,
/// token check, RFC 6455 framing — on standard sockets. One transport, not a
/// second parallel one.
pub type NIODevToolsTransport = StdDevToolsTransport;

use crate::server::config::hello_envelope;
use cove_devtools_protocol::{
    Envelope, InputRecording, MirrorFrame, MirrorStopped, Session, StateDifference,
    StateObservation, StateWatch, Subscription, TimelineSnapshot, TimingFrame, TreeSnapshot,
    WindowResize, decode_payload, state as decode_state_map,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Work a host callback wants broadcast but cannot deliver itself, because the
/// server owns the client list. Sinks push here; `pump` drains on the UI thread.
#[derive(Clone, Debug)]
pub enum Outbound {
    Timing(TimingFrame),
    MirrorFrame(MirrorFrame),
    MirrorStopped(String),
    TreeChanged,
}

struct Client {
    connection: Arc<dyn DevToolsConnection>,
    session: Session,
    last_state: Option<StateObservation>,
    /// Bytes queued for a mirrored log stream but not yet written.
    log_sequence: u64,
}

/// Generous cap for a single client's log backlog before it is dropped.
const MAX_CLIENT_LOG_BACKLOG: u64 = 4096;

/// WebSocket server with opt-in lifecycle.
pub struct DevServer {
    config: DevToolsConfig,
    transport: Arc<dyn DevToolsTransport>,
    pub host: DevToolsHost,
    pub advertised_capabilities: Vec<String>,
    clients: BTreeMap<usize, Client>,
    recording_owner: Option<usize>,
    selection_owner: Option<usize>,
    inspection_owner: Option<usize>,
    inspection_lease: u64,
    active_inspection_lease: Option<u64>,
    generation: u64,
    started: bool,
    logs: Arc<DevToolsLogFeed>,
    tree_broadcast_pending: bool,
    /// Full-tree traffic is bounded to 10 Hz: a broadcast stays in flight for
    /// 100 ms and changes meanwhile coalesce into one final delta.
    tree_broadcast_in_flight_until: Option<std::time::Instant>,
    next_lease: u64,
    /// Transport events awaiting dispatch on the host's UI thread.
    inbox: Option<crate::server::transport::EventReceiver>,
    /// Broadcasts queued by host sinks. Shared so a sink closure can hold it
    /// without borrowing the server.
    outbound: crate::devtools::mirror::OutboundQueue,
}

impl DevServer {
    /// Actual bound port, including when configured with port zero.
    pub fn bound_port(&self) -> Option<u16> {
        self.transport.bound_port()
    }

    pub fn config(&self) -> &DevToolsConfig {
        &self.config
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn new(config: DevToolsConfig) -> Self {
        let transport: Arc<dyn DevToolsTransport> = Arc::new(StdDevToolsTransport::new(
            config.launch.as_ref().map(|launch| launch.token.clone()),
        ));
        Self::with_transport(config, transport)
    }

    pub fn with_transport(config: DevToolsConfig, transport: Arc<dyn DevToolsTransport>) -> Self {
        Self {
            config,
            transport,
            host: DevToolsHost::default(),
            advertised_capabilities: vec![
                "tree".into(),
                "select".into(),
                "log".into(),
                "timing".into(),
            ],
            clients: BTreeMap::new(),
            recording_owner: None,
            selection_owner: None,
            inspection_owner: None,
            inspection_lease: 0,
            active_inspection_lease: None,
            generation: 0,
            started: false,
            logs: Arc::new(DevToolsLogFeed::default()),
            tree_broadcast_pending: false,
            tree_broadcast_in_flight_until: None,
            next_lease: 1,
            inbox: None,
            outbound: crate::devtools::mirror::OutboundQueue::default(),
        }
    }

    /// Start listening. Events are collected into `inbox`; the host calls
    /// `pump` on its UI thread to dispatch them.
    pub fn start(&mut self) -> std::io::Result<()> {
        if self.started {
            return Ok(());
        }
        self.generation = self.generation.wrapping_add(1);
        let (sender, receiver) = crate::server::transport::event_channel();
        self.inbox = Some(receiver);
        let logs = self.logs.clone();
        let sink: Arc<dyn Fn(cove_logging::LogRecord) + Send + Sync> =
            Arc::new(move |record| logs.enqueue(record));
        self.logs.start(sink);
        let capabilities = self.advertised_capabilities.clone();
        let config = self.config.clone();
        let sender_handle = sender;
        self.transport.start(
            &self.config.host,
            self.config.port,
            Arc::new(move |event| {
                let _ = sender_handle.send(event);
            }),
        )?;
        let _ = (capabilities, config);
        self.started = true;
        Ok(())
    }

    pub fn stop(&mut self) {
        if !self.started {
            return;
        }
        self.started = false;
        self.generation = self.generation.wrapping_add(1);
        self.logs.stop();
        self.transport.stop();
        self.inbox = None;
        let selection = self.selection_owner.is_some();
        let mirror = self.has_subscribers(Subscription::Mirror);
        let recording = self.recording_owner.is_some();
        let lease = self.inspection_lease;
        self.clients.clear();
        self.selection_owner = None;
        self.recording_owner = None;
        self.inspection_owner = None;
        self.inspection_lease = 0;
        self.tree_broadcast_pending = false;
        self.tree_broadcast_in_flight_until = None;
        if let Some(capture) = &self.host.observation.capture {
            capture(false);
        }
        if selection && let Some(clear) = &self.host.scene.clear_selection {
            clear();
        }
        if mirror && let Some(stop) = &self.host.mirror.stop {
            stop();
        }
        if recording && let Some(stop) = &self.host.recording.stop {
            let _ = stop();
        }
        if lease != 0 {
            self.reset_inspection(lease);
        }
    }

    /// Shared queue that frame-tap and timing sinks push into.
    pub fn outbound(&self) -> crate::devtools::mirror::OutboundQueue {
        self.outbound.clone()
    }

    /// Read pending transport events and queued broadcasts without blocking. The
    /// host calls this once per frame; several events per call are fine.
    pub fn pump(&mut self) {
        self.flush_tree_broadcast();
        // Drain broadcasts first so a frame produced this tick is delivered on
        // this tick.
        let queued: Vec<Outbound> = {
            let mut queue = self.outbound.borrow_mut();
            std::mem::take(&mut *queue)
        };
        for item in queued {
            match item {
                Outbound::Timing(frame) => self.broadcast_timing(frame),
                Outbound::MirrorFrame(frame) => self.broadcast_mirror_frame(frame),
                Outbound::MirrorStopped(reason) => self.broadcast_mirror_stopped(&reason),
                Outbound::TreeChanged => self.broadcast_tree_delta(),
            }
        }
        // `try_recv` needs `&Receiver`; the server keeps ownership so the
        // transport can be restarted without rebuilding it.
        let mut events = Vec::new();
        if let Some(inbox) = &self.inbox {
            while let Ok(event) = inbox.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.handle_event(event);
        }
        self.flush_logs();
    }

    fn handle_event(&mut self, event: DevToolsTransportEvent) {
        match event {
            DevToolsTransportEvent::Connected(connection) => {
                let key = Arc::as_ptr(&connection) as *const () as usize;
                self.clients.insert(
                    key,
                    Client {
                        connection: connection.clone(),
                        session: Session::default(),
                        last_state: None,
                        log_sequence: 0,
                    },
                );
                let hello = hello_envelope(&self.config, &self.advertised_capabilities);
                self.send(hello, &connection);
            }
            DevToolsTransportEvent::Text(connection, data) => {
                let key = Arc::as_ptr(&connection) as *const () as usize;
                if !self.clients.contains_key(&key) {
                    return;
                }
                self.dispatch(&data, &connection, key);
            }
            DevToolsTransportEvent::Disconnected(connection) => {
                let key = Arc::as_ptr(&connection) as *const () as usize;
                self.remove_client(key);
            }
        }
    }

    fn dispatch(&mut self, data: &[u8], connection: &Arc<dyn DevToolsConnection>, key: usize) {
        let Ok(envelope) = serde_json::from_slice::<Envelope>(data) else {
            log_debug("DevServer JSON decode failed");
            return;
        };
        // Validation runs against the client's own session before any handler
        // can mutate shared state.
        if let Some(error) = self
            .clients
            .get(&key)
            .and_then(|client| client.session.validate(&envelope))
        {
            self.send(error, connection);
            return;
        }
        if envelope.r#type.starts_with("inspect.") {
            self.dispatch_inspect(envelope, connection, key);
            return;
        }
        match envelope.r#type.as_str() {
            "diagnostics.tools" | "diagnostics.call" => {
                self.dispatch_diagnostics(envelope, connection)
            }
            "state.list"
            | "state.subscribe"
            | "state.unsubscribe"
            | "timeline.subscribe"
            | "timeline.unsubscribe" => self.dispatch_observation(envelope, connection, key),
            "mirror.start" | "mirror.stop" | "mirror.input" | "mirror.activate" => {
                self.dispatch_mirror(envelope, connection, key)
            }
            "window.resize" => self.dispatch_window(envelope, connection),
            "state.checkpoint" | "state.restore" | "state.diff" => {
                self.dispatch_checkpoint(envelope, connection)
            }
            "input.record.start" | "input.record.stop" | "input.replay" => {
                self.dispatch_recording(envelope, connection, key)
            }
            "hello.ack" | "tree.subscribe" | "tree.unsubscribe" | "select.node"
            | "select.clear" | "bye" | "log.subscribe" | "log.unsubscribe" | "timing.subscribe"
            | "timing.unsubscribe" => self.dispatch_scene(envelope, connection, key),
            other => self.send_error(
                connection,
                &envelope,
                "not_implemented",
                &format!("unknown message type: {other}"),
            ),
        }
    }

    // MARK: - Scene

    fn dispatch_scene(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
        key: usize,
    ) {
        match envelope.r#type.as_str() {
            "hello.ack" => {}
            "tree.subscribe" => {
                self.set_subscription(Subscription::Tree, true, key);
                if let Some(snapshot) = &self.host.scene.snapshot {
                    let snapshot = snapshot();
                    let mut response = envelope
                        .ok()
                        .unwrap_or_else(|| Envelope::new("tree.snapshot", envelope.id, None));
                    response.r#type = "tree.snapshot".into();
                    response.payload = serde_json::to_value(&snapshot).ok();
                    self.send(response, connection);
                }
            }
            "tree.unsubscribe" => {
                self.set_subscription(Subscription::Tree, false, key);
                self.send_ok(connection, &envelope);
            }
            "select.node" => {
                if self.inspection_owner.is_some_and(|owner| owner != key) {
                    self.send_error(
                        connection,
                        &envelope,
                        "busy",
                        "Another inspector owns the scene editor",
                    );
                    return;
                }
                let node_id = envelope
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                match node_id {
                    Some(node_id) => {
                        self.selection_owner = Some(key);
                        if let Some(select) = &self.host.scene.select {
                            select(&node_id);
                        }
                        self.send_ok(connection, &envelope);
                    }
                    None => self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "select.node requires payload.id",
                    ),
                }
            }
            "select.clear" => {
                if self.selection_owner == Some(key) {
                    self.selection_owner = None;
                    if let Some(clear) = &self.host.scene.clear_selection {
                        clear();
                    }
                }
                self.send_ok(connection, &envelope);
            }
            "bye" => connection.close(),
            "log.subscribe" => {
                let query =
                    decode_payload::<cove_logging::LogQuery>(&envelope.payload).unwrap_or_default();
                let snapshot = DevToolsLogFeed::snapshot(&query);
                let last = snapshot.records.last().map(|record| record.sequence);
                if let Some(client) = self.clients.get_mut(&key) {
                    client.session.log_query = query.clone();
                    client.session.log_sequence = last.or(query.after_sequence).unwrap_or(0);
                }
                self.set_subscription(Subscription::Log, true, key);
                let mut response = envelope
                    .ok()
                    .unwrap_or_else(|| Envelope::new("log.snapshot", envelope.id, None));
                response.r#type = "log.snapshot".into();
                response.payload = serde_json::to_value(&snapshot).ok();
                self.send(response, connection);
            }
            "log.unsubscribe" => {
                self.set_subscription(Subscription::Log, false, key);
                self.send_ok(connection, &envelope);
            }
            "timing.subscribe" => {
                self.set_subscription(Subscription::Timing, true, key);
                self.send_ok(connection, &envelope);
            }
            "timing.unsubscribe" => {
                self.set_subscription(Subscription::Timing, false, key);
                self.send_ok(connection, &envelope);
            }
            _ => {}
        }
    }

    fn dispatch_diagnostics(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
    ) {
        let Some(diagnostics) = &self.host.scene.diagnostics else {
            self.send_error(
                connection,
                &envelope,
                "unsupported",
                "Host has no diagnostic query adapter",
            );
            return;
        };
        let response = diagnostics(&envelope);
        self.send(response, connection);
    }

    fn dispatch_inspect(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
        key: usize,
    ) {
        if self.host.scene.inspect.is_none() {
            self.send_error(
                connection,
                &envelope,
                "unsupported",
                "Host has no scene editor",
            );
            return;
        }
        if self.inspection_owner.is_some_and(|owner| owner != key) {
            self.send_error(
                connection,
                &envelope,
                "busy",
                "Another inspector owns the temporary styles; disconnect it first",
            );
            return;
        }
        if self.inspection_owner.is_none() {
            self.inspection_owner = Some(key);
            self.inspection_lease = self.next_lease;
            self.next_lease = self.next_lease.wrapping_add(1);
        }
        self.selection_owner = Some(key);
        let lease = self.inspection_lease;
        if self.active_inspection_lease != Some(lease) {
            if self.active_inspection_lease.is_some()
                && let Some(reset) = &self.host.scene.reset
            {
                reset();
            }
            self.active_inspection_lease = Some(lease);
        }
        if let Some(inspect) = &self.host.scene.inspect {
            let response = inspect(&envelope);
            self.send(response, connection);
        }
    }

    // MARK: - Observation

    fn dispatch_observation(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
        key: usize,
    ) {
        match envelope.r#type.as_str() {
            "state.list" | "state.subscribe" => {
                let Some(state) = self.host.observation.state.clone() else {
                    self.send_error(
                        connection,
                        &envelope,
                        "unsupported",
                        "Host has no state registry",
                    );
                    return;
                };
                let ids = if envelope.r#type == "state.subscribe" {
                    decode_payload::<StateWatch>(&envelope.payload)
                        .map(|watch| watch.ids)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                if envelope.r#type == "state.subscribe" {
                    if let Some(client) = self.clients.get_mut(&key) {
                        client.session.watched_state_ids = ids.clone();
                    }
                    self.set_subscription(Subscription::State, true, key);
                }
                let observation = state(&ids);
                let mut response = envelope.ok().unwrap_or_else(|| {
                    Envelope::new(format!("{}.ok", envelope.r#type), envelope.id, None)
                });
                response.payload = serde_json::to_value(&observation).ok();
                self.send(response, connection);
            }
            "state.unsubscribe" => {
                if let Some(client) = self.clients.get_mut(&key) {
                    client.session.watched_state_ids.clear();
                }
                self.set_subscription(Subscription::State, false, key);
                self.send_ok(connection, &envelope);
            }
            "timeline.subscribe" => {
                if self.host.observation.timeline.is_none() {
                    self.send_error(
                        connection,
                        &envelope,
                        "unsupported",
                        "Host has no timeline recorder",
                    );
                    return;
                }
                if let Some(client) = self.clients.get_mut(&key) {
                    client.session.timeline_sequence = 0;
                }
                self.set_subscription(Subscription::Timeline, true, key);
                self.sync_timeline_capture();
                if let Some(timeline) = &self.host.observation.timeline {
                    let snapshot = timeline(0);
                    self.send_timeline(&snapshot, connection, envelope.id);
                }
            }
            "timeline.unsubscribe" => {
                self.set_subscription(Subscription::Timeline, false, key);
                self.sync_timeline_capture();
                self.send_ok(connection, &envelope);
            }
            _ => {}
        }
    }

    fn sync_timeline_capture(&self) {
        if let Some(capture) = &self.host.observation.capture {
            capture(self.has_subscribers(Subscription::Timeline));
        }
    }

    fn send_timeline(
        &mut self,
        snapshot: &TimelineSnapshot,
        connection: &Arc<dyn DevToolsConnection>,
        id: Option<i64>,
    ) {
        let key = Arc::as_ptr(connection) as *const () as usize;
        let Some(client) = self.clients.get_mut(&key) else {
            return;
        };
        if !client
            .session
            .subscriptions
            .contains(&Subscription::Timeline)
        {
            return;
        }
        let cursor = client.session.timeline_sequence;
        let events: Vec<_> = snapshot
            .events
            .iter()
            .filter(|event| event.sequence > cursor)
            .cloned()
            .collect();
        if let Some(last) = events.last() {
            client.session.timeline_sequence = last.sequence;
        }
        // A subscription request is always answered, even when empty.
        if id.is_none() && events.is_empty() {
            return;
        }
        let payload = TimelineSnapshot {
            events,
            dropped: snapshot.dropped,
        };
        let mut envelope = Envelope::new("timeline.events", id, serde_json::to_value(payload).ok());
        envelope.r#type = "timeline.events".into();
        self.send(envelope, connection);
    }

    // MARK: - Mirror

    fn dispatch_mirror(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
        key: usize,
    ) {
        match envelope.r#type.as_str() {
            "mirror.start" => {
                let payload = if envelope.payload.is_some() {
                    match decode_payload::<cove_devtools_protocol::MirrorStart>(&envelope.payload) {
                        Some(payload) => payload,
                        None => {
                            self.send_error(
                                connection,
                                &envelope,
                                "bad_request",
                                "mirror.start payload is malformed",
                            );
                            return;
                        }
                    }
                } else {
                    cove_devtools_protocol::MirrorStart {
                        fps: None,
                        quality: None,
                        scale: None,
                    }
                };
                self.set_subscription(Subscription::Mirror, true, key);
                if let Some(start) = &self.host.mirror.start {
                    start(&payload);
                }
                self.send_ok(connection, &envelope);
            }
            "mirror.stop" => {
                self.set_subscription(Subscription::Mirror, false, key);
                let should_stop = !self.has_subscribers(Subscription::Mirror);
                if should_stop && let Some(stop) = &self.host.mirror.stop {
                    stop();
                }
                self.send_ok(connection, &envelope);
            }
            "mirror.input" => {
                if !self.is_subscribed(Subscription::Mirror, key) {
                    self.send_error(
                        connection,
                        &envelope,
                        "invalid_state",
                        "mirror.input requires an active mirror subscription",
                    );
                    return;
                }
                match decode_payload::<cove_devtools_protocol::MirrorInput>(&envelope.payload) {
                    Some(input) => {
                        if let Some(deliver) = &self.host.mirror.input {
                            deliver(&input);
                        }
                        self.send_ok(connection, &envelope);
                    }
                    None => self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "mirror.input requires payload",
                    ),
                }
            }
            "mirror.activate" => {
                if !self.is_subscribed(Subscription::Mirror, key) {
                    self.send_error(
                        connection,
                        &envelope,
                        "invalid_state",
                        "mirror.activate requires an active mirror subscription",
                    );
                    return;
                }
                let Some(request) =
                    decode_payload::<cove_devtools_protocol::MirrorActivate>(&envelope.payload)
                else {
                    self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "mirror.activate requires payload.node_id",
                    );
                    return;
                };
                let failure = self
                    .host
                    .mirror
                    .activate
                    .as_ref()
                    .map(|activate| activate(&request))
                    .unwrap_or_else(|| Some("Host cannot perform accessibility actions".into()));
                match failure {
                    Some(message) => self.send_error(connection, &envelope, "not_found", &message),
                    None => self.send_ok(connection, &envelope),
                }
            }
            _ => {}
        }
    }

    // MARK: - Window / checkpoint / recording

    fn dispatch_window(&mut self, envelope: Envelope, connection: &Arc<dyn DevToolsConnection>) {
        let Some(request) =
            decode_payload::<WindowResize>(&envelope.payload).filter(WindowResize::is_valid)
        else {
            self.send_error(
                connection,
                &envelope,
                "bad_request",
                "window.resize requires payload.width and payload.height in 1...16384",
            );
            return;
        };
        let failure = self
            .host
            .window
            .resize
            .as_ref()
            .map(|resize| resize(&request))
            .unwrap_or_else(|| Some("Host cannot resize its window".into()));
        match failure {
            Some(message) => self.send_error(connection, &envelope, "not_supported", &message),
            None => self.send_ok(connection, &envelope),
        }
    }

    fn dispatch_checkpoint(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
    ) {
        match envelope.r#type.as_str() {
            "state.checkpoint" => {
                let snapshot = self
                    .host
                    .checkpoint
                    .capture
                    .as_ref()
                    .map(|capture| capture())
                    .unwrap_or_default();
                let mut response = envelope
                    .ok()
                    .unwrap_or_else(|| Envelope::new("state.checkpoint.ok", envelope.id, None));
                response.payload = serde_json::to_value(&snapshot).ok();
                self.send(response, connection);
            }
            "state.restore" => {
                let Some(snapshot) = decode_state_map(&envelope.payload) else {
                    self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "state.restore requires an object with string values",
                    );
                    return;
                };
                match &self.host.checkpoint.restore {
                    Some(restore) if restore(&snapshot) => self.send_ok(connection, &envelope),
                    Some(_) => self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "Host rejected checkpoint state",
                    ),
                    None => self.send_error(
                        connection,
                        &envelope,
                        "unsupported",
                        "Host has no checkpoint restore provider",
                    ),
                }
            }
            "state.diff" => {
                let Some(before) = decode_state_map(&envelope.payload) else {
                    self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "state.restore requires an object with string values",
                    );
                    return;
                };
                let after = self
                    .host
                    .checkpoint
                    .capture
                    .as_ref()
                    .map(|capture| capture())
                    .unwrap_or_default();
                let diff = StateDifference::new(&before, &after);
                let mut response = envelope
                    .ok()
                    .unwrap_or_else(|| Envelope::new("state.diff.ok", envelope.id, None));
                response.payload = serde_json::to_value(&diff).ok();
                self.send(response, connection);
            }
            _ => {}
        }
    }

    fn dispatch_recording(
        &mut self,
        envelope: Envelope,
        connection: &Arc<dyn DevToolsConnection>,
        key: usize,
    ) {
        match envelope.r#type.as_str() {
            "input.record.start" => {
                if self.recording_owner.is_some() || self.host.recording.start.is_none() {
                    self.send_error(
                        connection,
                        &envelope,
                        "invalid_state",
                        "Recording unavailable or already active",
                    );
                    return;
                }
                self.recording_owner = Some(key);
                let started = self
                    .host
                    .recording
                    .start
                    .as_ref()
                    .is_some_and(|start| start());
                if started {
                    self.send_ok(connection, &envelope);
                } else {
                    self.recording_owner = None;
                    self.send_error(
                        connection,
                        &envelope,
                        "invalid_state",
                        "Host could not start recording",
                    );
                }
            }
            "input.record.stop" => {
                if self.recording_owner != Some(key) {
                    self.send_error(
                        connection,
                        &envelope,
                        "invalid_state",
                        "This connection does not own a recording",
                    );
                    return;
                }
                self.recording_owner = None;
                match self.host.recording.stop.as_ref().and_then(|stop| stop()) {
                    Some(recording) => {
                        let mut response = envelope.ok().unwrap_or_else(|| {
                            Envelope::new("input.record.stop.ok", envelope.id, None)
                        });
                        response.payload = serde_json::to_value(&recording).ok();
                        self.send(response, connection);
                    }
                    None => self.send_error(connection, &envelope, "invalid_state", "No recording"),
                }
            }
            "input.replay" => {
                if self.recording_owner.is_some() || self.host.recording.replay.is_none() {
                    self.send_error(
                        connection,
                        &envelope,
                        "invalid_state",
                        "Stop recording before replaying",
                    );
                    return;
                }
                let Some(recording) = decode_payload::<InputRecording>(&envelope.payload) else {
                    self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "input.replay requires a recording payload",
                    );
                    return;
                };
                let accepted = self
                    .host
                    .recording
                    .replay
                    .as_ref()
                    .is_some_and(|replay| replay(&recording));
                if accepted {
                    self.send_ok(connection, &envelope);
                } else {
                    self.send_error(
                        connection,
                        &envelope,
                        "bad_request",
                        "Host rejected recording state",
                    );
                }
            }
            _ => {}
        }
    }

    // MARK: - Broadcasts

    /// Compact snapshots coalesce: at most one is pending at a time.
    pub fn broadcast_tree_delta(&mut self) {
        if !self.has_subscribers(Subscription::Tree) {
            return;
        }
        if self.tree_broadcast_in_flight_until.is_some() {
            self.tree_broadcast_pending = true;
            return;
        }
        let Some(snapshot) = self.host.scene.snapshot.clone() else {
            return;
        };
        let snapshot = snapshot();
        self.broadcast_tree(&snapshot, "tree.delta", None);
        self.tree_broadcast_in_flight_until =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(100));
    }

    /// Ends an expired 10 Hz window and sends the change that arrived during it.
    fn flush_tree_broadcast(&mut self) {
        let Some(until) = self.tree_broadcast_in_flight_until else {
            return;
        };
        if std::time::Instant::now() < until {
            return;
        }
        self.tree_broadcast_in_flight_until = None;
        if std::mem::take(&mut self.tree_broadcast_pending) {
            self.broadcast_tree_delta();
        }
    }

    fn broadcast_tree(&mut self, snapshot: &TreeSnapshot, kind: &str, id: Option<i64>) {
        let payload = serde_json::to_value(snapshot).ok();
        let envelope = Envelope::new(kind, id, payload);
        self.send_to_subscribers(&envelope, Subscription::Tree);
    }

    pub fn broadcast_log(&mut self, record: cove_logging::LogRecord) {
        self.logs.enqueue(record);
    }

    /// Deliver queued log records to subscribers, honouring each client's query
    /// and cursor. Called from `pump` so delivery never re-enters a callback.
    fn flush_logs(&mut self) {
        if !self.logs.has_pending() {
            return;
        }
        let (records, dropped) = self.logs.drain();
        if records.is_empty() && dropped == 0 {
            return;
        }
        let keys: Vec<usize> = self
            .clients
            .iter()
            .filter(|(_, client)| client.session.subscriptions.contains(&Subscription::Log))
            .map(|(key, _)| *key)
            .collect();
        for key in keys {
            let mut loss = dropped;
            let mut entries = Vec::new();
            {
                let Some(client) = self.clients.get_mut(&key) else {
                    continue;
                };
                for record in &records {
                    if record.sequence > client.session.log_sequence
                        && client.session.log_query.matches(record)
                    {
                        client.session.log_sequence = record.sequence;
                        if !client.connection.is_writable() {
                            loss = loss.wrapping_add(1);
                            continue;
                        }
                        entries.push(record.clone());
                    }
                }
                client.session.log_dropped = client.session.log_dropped.wrapping_add(loss);
                client.log_sequence = client.session.log_sequence;
            }
            let (connection, gap, sequence) = {
                let Some(client) = self.clients.get(&key) else {
                    continue;
                };
                (
                    client.connection.clone(),
                    client.session.log_dropped,
                    client.log_sequence,
                )
            };
            for record in entries {
                let envelope = Envelope::new("log.entry", None, serde_json::to_value(&record).ok());
                self.send(envelope, &connection);
            }
            if gap > 0 && connection.is_writable() {
                let envelope =
                    Envelope::new("log.gap", None, Some(serde_json::json!({ "dropped": gap })));
                self.send(envelope, &connection);
                if let Some(client) = self.clients.get_mut(&key) {
                    client.session.log_dropped = 0;
                }
            }
            // Bound the backlog a single slow client can accumulate.
            if sequence.saturating_add(MAX_CLIENT_LOG_BACKLOG) < self.latest_log_sequence() {
                if let Some(client) = self.clients.get_mut(&key) {
                    client.session.log_dropped = client.session.log_dropped.wrapping_add(1);
                }
            }
        }
    }

    fn latest_log_sequence(&self) -> u64 {
        self.clients
            .values()
            .map(|client| client.log_sequence)
            .max()
            .unwrap_or(0)
    }

    pub fn broadcast_timing(&mut self, frame: TimingFrame) {
        if !self.has_subscribers(Subscription::Timing) {
            return;
        }
        let envelope = Envelope::new("timing.frame", None, serde_json::to_value(&frame).ok());
        self.send_to_subscribers(&envelope, Subscription::Timing);
    }

    /// Read only requested summaries. Unchanged snapshots are suppressed per
    /// client.
    pub fn broadcast_observations(&mut self) {
        let watchers: Vec<(usize, Vec<String>)> = self
            .clients
            .iter()
            .filter(|(_, client)| client.session.subscriptions.contains(&Subscription::State))
            .map(|(key, client)| (*key, client.session.watched_state_ids.clone()))
            .collect();
        for (key, ids) in watchers {
            let Some(state) = self.host.observation.state.clone() else {
                continue;
            };
            let observation = state(&ids);
            let should_send = self.clients.get(&key).is_some_and(|client| {
                client.session.subscriptions.contains(&Subscription::State)
                    && client.session.watched_state_ids == ids
                    && client.last_state.as_ref() != Some(&observation)
            });
            if !should_send {
                continue;
            }
            let connection = {
                let Some(client) = self.clients.get_mut(&key) else {
                    continue;
                };
                client.last_state = Some(observation.clone());
                client.connection.clone()
            };
            let envelope = Envelope::new(
                "state.observation",
                None,
                serde_json::to_value(&observation).ok(),
            );
            self.send(envelope, &connection);
        }
        let cursor = self
            .clients
            .values()
            .filter(|client| {
                client
                    .session
                    .subscriptions
                    .contains(&Subscription::Timeline)
            })
            .map(|client| client.session.timeline_sequence)
            .min();
        let (Some(cursor), Some(timeline)) = (cursor, &self.host.observation.timeline) else {
            return;
        };
        let snapshot = timeline(cursor);
        let connections: Vec<Arc<dyn DevToolsConnection>> = self
            .clients
            .values()
            .filter(|client| {
                client
                    .session
                    .subscriptions
                    .contains(&Subscription::Timeline)
            })
            .map(|client| client.connection.clone())
            .collect();
        for connection in connections {
            self.send_timeline(&snapshot, &connection, None);
        }
    }

    pub fn broadcast_mirror_frame(&mut self, frame: MirrorFrame) {
        if !self.has_subscribers(Subscription::Mirror) {
            return;
        }
        let envelope = Envelope::new("mirror.frame", None, serde_json::to_value(&frame).ok());
        self.send_to_subscribers(&envelope, Subscription::Mirror);
    }

    pub fn broadcast_mirror_stopped(&mut self, reason: &str) {
        let envelope = Envelope::new(
            "mirror.stopped",
            None,
            serde_json::to_value(MirrorStopped {
                reason: reason.into(),
            })
            .ok(),
        );
        self.send_to_subscribers(&envelope, Subscription::Mirror);
    }

    // MARK: - Plumbing

    fn send(&mut self, envelope: Envelope, connection: &Arc<dyn DevToolsConnection>) {
        let Ok(data) = serde_json::to_vec(&envelope) else {
            log_debug("DevServer encode failed");
            return;
        };
        connection.send_text(&data);
    }

    fn send_ok(&mut self, connection: &Arc<dyn DevToolsConnection>, request: &Envelope) {
        if let Some(response) = request.ok() {
            self.send(response, connection);
        }
    }

    fn send_error(
        &mut self,
        connection: &Arc<dyn DevToolsConnection>,
        request: &Envelope,
        code: &str,
        message: &str,
    ) {
        let mut envelope = request.error(code, message);
        envelope.payload = Some(serde_json::json!({ "code": code, "message": message }));
        self.send(envelope, connection);
    }

    fn send_to_subscribers(&mut self, envelope: &Envelope, subscription: Subscription) {
        let connections: Vec<Arc<dyn DevToolsConnection>> = self
            .clients
            .values()
            .filter(|client| client.session.subscriptions.contains(&subscription))
            .map(|client| client.connection.clone())
            .collect();
        for connection in connections {
            self.send(envelope.clone(), &connection);
        }
    }

    fn set_subscription(&mut self, subscription: Subscription, enabled: bool, key: usize) -> bool {
        self.clients
            .get_mut(&key)
            .is_some_and(|client| client.session.set(subscription, enabled))
    }

    fn has_subscribers(&self, subscription: Subscription) -> bool {
        self.clients
            .values()
            .any(|client| client.session.subscriptions.contains(&subscription))
    }

    fn is_subscribed(&self, subscription: Subscription, key: usize) -> bool {
        self.clients
            .get(&key)
            .is_some_and(|client| client.session.subscriptions.contains(&subscription))
    }

    fn remove_client(&mut self, key: usize) {
        let Some(removed) = self.clients.remove(&key) else {
            return;
        };
        let was_mirroring = removed
            .session
            .subscriptions
            .contains(&Subscription::Mirror);
        if removed
            .session
            .subscriptions
            .contains(&Subscription::Timeline)
        {
            self.sync_timeline_capture();
        }
        if self.inspection_owner == Some(key) {
            let lease = self.inspection_lease;
            self.inspection_owner = None;
            self.inspection_lease = 0;
            self.reset_inspection(lease);
        }
        if self.recording_owner == Some(key) {
            self.recording_owner = None;
            if let Some(stop) = &self.host.recording.stop {
                let _ = stop();
            }
        }
        if self.selection_owner == Some(key) {
            self.selection_owner = None;
            if let Some(clear) = &self.host.scene.clear_selection {
                clear();
            }
        }
        if was_mirroring
            && !self.has_subscribers(Subscription::Mirror)
            && let Some(stop) = &self.host.mirror.stop
        {
            stop();
        }
    }

    fn reset_inspection(&mut self, lease: u64) {
        if self.active_inspection_lease != Some(lease) {
            return;
        }
        self.active_inspection_lease = None;
        if let Some(reset) = &self.host.scene.reset {
            reset();
        }
    }
}

fn log_debug(message: &str) {
    cove_logging::LogSystem::logger("com.cove.devtools").log(&cove_logging::LogEvent::new(
        cove_logging::Level::Debug,
        message,
    ));
}
