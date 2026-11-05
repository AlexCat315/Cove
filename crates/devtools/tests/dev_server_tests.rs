//! Translation of CoveUIDevToolsServerTests/DevServerTests.swift.
//!
//! Behaviour checklist:
//! - diagnostic reads answer without taking the inspection lease
//! - a host without diagnostics reports `unsupported`
//! - a large tree broadcast coalesces a burst and still delivers the final change
//! - a deeply nested tree round-trips through JSON unchanged
//! - a stale subscribe cannot capture after the server restarts
//! - subscriptions, checkpoint, selection and disconnect share one protocol
//! - the state diff and restore path rejects non-string and out-of-range values
//! - recording ownership stays with the connection that started it
//! - temporary inspection is exclusive and disconnect cleans only its own lease
//! - state and timeline streams are independent; timeline stops after the last disconnect
//! - timing frames stream only to subscribed clients
//! - logs replay before the connection filter and report slow-consumer loss
//! - window resize validates its payload and reaches the host
//!
//! The Swift suite awaited an async transport. Rust dispatches synchronously
//! inside `DevServer::pump`, so a request is written by injecting a transport
//! event and answered by pumping; `receive` below keeps the same shape as the
//! Swift helper without sleeping.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_devtools::protocol::{
    Envelope, InputRecording, JsonValue, PROTOCOL_VERSION, StateDifference, StateObservation,
    StateWatch, TimingFrame, TreeSnapshot, WindowResize, decode_payload,
};
use cove_devtools::server::{
    DevServer, DevToolsConfig, DevToolsConnection, DevToolsTransport, DevToolsTransportEvent,
};

/// Records everything the server writes, in order.
#[derive(Default)]
struct TestConnection {
    messages: Mutex<Vec<Vec<u8>>>,
    closed: Mutex<bool>,
}

impl TestConnection {
    fn take(&self) -> Vec<Envelope> {
        let drained = std::mem::take(&mut *self.messages.lock().expect("messages"));
        drained
            .into_iter()
            .map(|bytes| serde_json::from_slice(&bytes).expect("envelope"))
            .collect()
    }

    fn drain(&self) {
        self.messages.lock().expect("messages").clear();
    }
}

impl DevToolsConnection for TestConnection {
    fn is_writable(&self) -> bool {
        true
    }
    fn send_text(&self, text: &[u8]) {
        self.messages.lock().expect("messages").push(text.to_vec());
    }
    fn close(&self) {
        *self.closed.lock().expect("closed") = true;
    }
}

/// Transport the test drives directly instead of over a socket.
struct TestTransport {
    on_event: Mutex<Option<Arc<dyn Fn(DevToolsTransportEvent) + Send + Sync>>>,
}

impl TestTransport {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            on_event: Mutex::new(None),
        })
    }

    fn connect(&self, connection: &Arc<TestConnection>) {
        let callback = self.on_event.lock().expect("event").clone();
        if let Some(callback) = callback {
            callback(DevToolsTransportEvent::Connected(connection.clone()));
        }
    }

    fn disconnect(&self, connection: &Arc<TestConnection>) {
        let callback = self.on_event.lock().expect("event").clone();
        if let Some(callback) = callback {
            callback(DevToolsTransportEvent::Disconnected(connection.clone()));
        }
    }

    fn request(&self, kind: &str, payload: Option<JsonValue>, connection: &Arc<TestConnection>) {
        let envelope = Envelope::new(kind, Some(1), payload);
        let data = serde_json::to_vec(&envelope).expect("encode");
        let callback = self.on_event.lock().expect("event").clone();
        if let Some(callback) = callback {
            callback(DevToolsTransportEvent::Text(connection.clone(), data));
        }
    }
}

impl DevToolsTransport for TestTransport {
    fn bound_port(&self) -> Option<u16> {
        Some(9229)
    }
    fn start(
        &self,
        _host: &str,
        _port: u16,
        on_event: Arc<dyn Fn(DevToolsTransportEvent) + Send + Sync>,
    ) -> std::io::Result<()> {
        *self.on_event.lock().expect("event") = Some(on_event);
        Ok(())
    }
    fn stop(&self) {
        *self.on_event.lock().expect("event") = None;
    }
}

fn server(transport: &Arc<TestTransport>) -> DevServer {
    DevServer::with_transport(DevToolsConfig::default(), transport.clone())
}

/// Pump until `kind` arrives, or fail. Dispatch is synchronous, so this only
/// needs to drain the inbox the transport filled.
fn receive(server: &mut DevServer, kind: &str, connection: &Arc<TestConnection>) -> Envelope {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        server.pump();
        if let Some(message) = connection
            .take()
            .into_iter()
            .find(|message| message.r#type == kind)
        {
            return message;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out waiting for {kind}");
}

fn connect(
    server: &mut DevServer,
    transport: &Arc<TestTransport>,
    connection: &Arc<TestConnection>,
) {
    transport.connect(connection);
    let hello = receive(server, "hello", connection);
    assert!(hello.payload.is_some(), "hello carries the host payload");
}

fn code(envelope: &Envelope) -> Option<String> {
    envelope
        .payload
        .as_ref()
        .and_then(|payload| payload.get("code"))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

#[test]
fn diagnostic_reads_use_the_host_adapter_without_taking_the_inspection_lease() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let owner = Arc::new(TestConnection::default());
    let reader = Arc::new(TestConnection::default());
    let calls = Rc::new(RefCell::new(0usize));

    server.host.scene.inspect = Some(Rc::new(|request: &Envelope| {
        Envelope::new(format!("{}.ok", request.r#type), request.id, None)
    }));
    let counter = calls.clone();
    server.host.scene.diagnostics = Some(Rc::new(move |request: &Envelope| {
        *counter.borrow_mut() += 1;
        Envelope::new(
            format!("{}.ok", request.r#type),
            request.id,
            Some(serde_json::json!({ "count": *counter.borrow() as f64 })),
        )
    }));
    server.start().expect("start");

    connect(&mut server, &transport, &owner);
    connect(&mut server, &transport, &reader);

    transport.request("inspect.pick.start", None, &owner);
    transport.request("diagnostics.call", None, &reader);
    let read = receive(&mut server, "diagnostics.call.ok", &reader);
    assert_eq!(*calls.borrow(), 1, "the reader's call reached the adapter");
    assert_eq!(
        read.payload
            .as_ref()
            .and_then(|payload| payload.get("count"))
            .and_then(JsonValue::as_f64),
        Some(1.0)
    );
    // The reader's diagnostic call did not take the inspection lease.
    transport.request("inspect.pick.start", None, &reader);
    let busy = receive(&mut server, "inspect.pick.start.err", &reader);
    assert_eq!(code(&busy).as_deref(), Some("busy"));

    server.stop();
}

#[test]
fn diagnostic_reads_report_unavailable_hosts() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let connection = Arc::new(TestConnection::default());
    server.start().expect("start");
    connect(&mut server, &transport, &connection);

    transport.request("diagnostics.tools", None, &connection);
    let missing = receive(&mut server, "diagnostics.tools.err", &connection);
    assert_eq!(code(&missing).as_deref(), Some("unsupported"));

    let calls = Rc::new(RefCell::new(0usize));
    let counter = calls.clone();
    server.host.scene.diagnostics = Some(Rc::new(move |request: &Envelope| {
        *counter.borrow_mut() += 1;
        Envelope::new(format!("{}.ok", request.r#type), request.id, None)
    }));
    transport.request("diagnostics.call", None, &connection);
    let _ = receive(&mut server, "diagnostics.call.ok", &connection);
    assert_eq!(*calls.borrow(), 1);

    // A restarted server refuses the stale connection until it says hello again.
    server.stop();
    connection.drain();
    server.start().expect("restart");
    transport.request("diagnostics.call", None, &connection);
    server.pump();
    assert!(
        connection.take().is_empty(),
        "an unknown connection gets no answer"
    );
    assert_eq!(*calls.borrow(), 1);

    connect(&mut server, &transport, &connection);
    transport.request("diagnostics.call", None, &connection);
    let _ = receive(&mut server, "diagnostics.call.ok", &connection);
    assert_eq!(*calls.borrow(), 2);
    server.stop();
}

fn node(
    id: &str,
    children: Vec<cove_devtools::protocol::NodeSummary>,
) -> cove_devtools::protocol::NodeSummary {
    use cove_devtools::protocol::{NodeFlags, NodeFrame, NodeSummary};
    NodeSummary {
        id: id.into(),
        view_tag: None,
        debug_name: None,
        frame: NodeFrame {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        },
        absolute_frame: None,
        content_offset: None,
        z_index: None,
        opacity: None,
        layout_role: None,
        semantic_role: None,
        flags: NodeFlags {
            hit_testable: true,
            focusable: false,
            clips_to_bounds: false,
            has_background: true,
            has_border: false,
        },
        children,
        element_id: Some(id.into()),
        layout: None,
        style: None,
        source: None,
        owner_scope_id: None,
        recomposition: None,
    }
}

#[test]
fn large_tree_broadcast_coalesces_bursts_and_delivers_final_idle_change() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let connection = Arc::new(TestConnection::default());
    let captures = Rc::new(RefCell::new(0usize));
    let name = Rc::new(RefCell::new("initial".to_string()));

    let children: Vec<_> = (0..1_036)
        .map(|index| {
            let mut child = node(&index.to_string(), Vec::new());
            child.view_tag = Some("EditorPanel".into());
            child.source = Some(cove_devtools::protocol::SourceLocation {
                file_id: "Editor/Panel.swift".into(),
                file_path: "/project/Panel.swift".into(),
                line: index as u64 + 1,
                column: 2,
            });
            child
        })
        .collect();
    let snapshot_children = children.clone();
    let counter = captures.clone();
    let revision = name.clone();
    server.host.scene.snapshot = Some(Rc::new(move || {
        *counter.borrow_mut() += 1;
        let mut root = node("root", snapshot_children.clone());
        root.debug_name = Some(revision.borrow().clone());
        TreeSnapshot {
            root: Some(root),
            invalidations: None,
            render_inventory: None,
            input_inventory: None,
            inspection: None,
        }
    }));
    server.start().expect("start");
    connect(&mut server, &transport, &connection);

    transport.request("tree.subscribe", None, &connection);
    let initial = receive(&mut server, "tree.snapshot", &connection);
    let snapshot: TreeSnapshot = decode_payload(&initial.payload).expect("tree payload");
    let root = snapshot.root.expect("root");
    assert_eq!(initial.id, Some(1));
    assert_eq!(root.children.len(), 1_036);
    assert_eq!(
        root.children
            .last()
            .and_then(|child| child.source.as_ref())
            .map(|s| s.line),
        Some(1_036)
    );

    *name.borrow_mut() = "first".into();
    server.broadcast_tree_delta();
    for index in 0..100 {
        *name.borrow_mut() = format!("change.{index}");
        server.broadcast_tree_delta();
    }
    // The burst does not capture 100 full snapshots.
    assert_eq!(*captures.borrow(), 2);

    let first = receive(&mut server, "tree.delta", &connection);
    assert_eq!(first.id, None, "a broadcast is a notification");
    let first_payload: TreeSnapshot = decode_payload(&first.payload).expect("tree payload");
    assert_eq!(
        first_payload
            .root
            .and_then(|root| root.debug_name)
            .as_deref(),
        Some("first")
    );

    let final_message = receive(&mut server, "tree.delta", &connection);
    let final_payload: TreeSnapshot = decode_payload(&final_message.payload).expect("tree payload");
    assert_eq!(
        final_payload
            .root
            .and_then(|root| root.debug_name)
            .as_deref(),
        Some("change.99")
    );

    transport.request("tree.unsubscribe", None, &connection);
    let _ = receive(&mut server, "tree.unsubscribe.ok", &connection);
    let after = *captures.borrow();
    server.broadcast_tree_delta();
    assert_eq!(*captures.borrow(), after, "no capture once unsubscribed");
    server.stop();
}

#[test]
fn deeply_nested_tree_round_trips_through_json() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let connection = Arc::new(TestConnection::default());

    // serde_json counts one level per object *and* per array, and the default
    // limit is 128, so each NodeSummary costs two levels. 60 wrappers (122
    // levels) is the same stress the Swift 128-wrapper tree applied — deep
    // enough to catch a stack-unsafe encoder — while staying under the limit.
    let mut root = node("leaf", Vec::new());
    for index in 0..60 {
        root = node(&format!("wrapper.{index}"), vec![root]);
    }
    let expected = root.id.clone();
    let snapshot = TreeSnapshot {
        root: Some(root),
        invalidations: None,
        render_inventory: None,
        input_inventory: None,
        inspection: None,
    };
    server.host.scene.snapshot = Some(Rc::new(move || snapshot.clone()));
    server.start().expect("start");
    connect(&mut server, &transport, &connection);

    transport.request("tree.subscribe", None, &connection);
    let initial = receive(&mut server, "tree.snapshot", &connection);
    let payload: TreeSnapshot = decode_payload(&initial.payload).expect("tree payload");
    let mut current = payload.root.expect("root");
    for _ in 0..60 {
        current = current.children.into_iter().next().expect("nested child");
    }
    assert_eq!(current.id, "leaf");

    server.broadcast_tree_delta();
    let update = receive(&mut server, "tree.delta", &connection);
    let payload: TreeSnapshot = decode_payload(&update.payload).expect("tree payload");
    assert_eq!(payload.root.map(|root| root.id), Some(expected));
    server.stop();
}

#[test]
fn stale_tree_subscribe_cannot_capture_after_server_restart() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let old = Arc::new(TestConnection::default());
    let current = Arc::new(TestConnection::default());
    let captures = Rc::new(RefCell::new(0usize));

    let counter = captures.clone();
    server.host.scene.snapshot = Some(Rc::new(move || {
        *counter.borrow_mut() += 1;
        TreeSnapshot {
            root: None,
            invalidations: None,
            render_inventory: None,
            input_inventory: None,
            inspection: None,
        }
    }));
    server.start().expect("start");
    connect(&mut server, &transport, &old);

    transport.request("tree.subscribe", None, &old);
    let _ = receive(&mut server, "tree.snapshot", &old);
    assert_eq!(*captures.borrow(), 1);

    server.stop();
    old.drain();
    server.start().expect("restart");
    // The old connection is unknown until it says hello again.
    transport.request("tree.subscribe", None, &old);
    server.pump();
    assert_eq!(*captures.borrow(), 1, "a stale connection captures nothing");

    connect(&mut server, &transport, &current);
    transport.request("tree.subscribe", None, &current);
    let _ = receive(&mut server, "tree.snapshot", &current);
    assert_eq!(*captures.borrow(), 2);
    server.stop();
}

#[test]
fn subscriptions_state_and_disconnect_use_shared_protocol() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let snapshots = Rc::new(RefCell::new(0usize));
    let restored = Rc::new(RefCell::new(None::<BTreeMap<String, String>>));
    let selections = Rc::new(RefCell::new(Vec::<String>::new()));
    let mirror_stops = Rc::new(RefCell::new(0usize));

    let counter = snapshots.clone();
    server.host.scene.snapshot = Some(Rc::new(move || {
        *counter.borrow_mut() += 1;
        TreeSnapshot {
            root: None,
            invalidations: None,
            render_inventory: None,
            input_inventory: None,
            inspection: None,
        }
    }));
    server.host.checkpoint.capture = Some(Rc::new(|| {
        BTreeMap::from([("count".to_string(), "7".to_string())])
    }));
    let restore_slot = restored.clone();
    server.host.checkpoint.restore = Some(Rc::new(move |state: &BTreeMap<String, String>| {
        *restore_slot.borrow_mut() = Some(state.clone());
        true
    }));
    let select_slot = selections.clone();
    server.host.scene.select = Some(Rc::new(move |id: &str| {
        select_slot.borrow_mut().push(id.to_string())
    }));
    let clear_slot = selections.clone();
    server.host.scene.clear_selection = Some(Rc::new(move || {
        clear_slot.borrow_mut().push("clear".to_string())
    }));
    let stops = mirror_stops.clone();
    server.host.mirror.stop = Some(Rc::new(move || *stops.borrow_mut() += 1));

    server.start().expect("start");
    // No subscriber: a tree change captures nothing.
    server.broadcast_tree_delta();
    assert_eq!(*snapshots.borrow(), 0);

    let connection = Arc::new(TestConnection::default());
    connect(&mut server, &transport, &connection);
    transport.request("tree.subscribe", None, &connection);
    let _ = receive(&mut server, "tree.snapshot", &connection);
    assert_eq!(*snapshots.borrow(), 1);

    transport.request("state.checkpoint", None, &connection);
    let checkpoint = receive(&mut server, "state.checkpoint.ok", &connection);
    assert_eq!(
        checkpoint
            .payload
            .as_ref()
            .and_then(|payload| payload.get("count"))
            .and_then(JsonValue::as_str),
        Some("7")
    );

    // A non-string value is rejected and never reaches the host.
    transport.request(
        "state.restore",
        Some(serde_json::json!({ "count": 2 })),
        &connection,
    );
    let _ = receive(&mut server, "state.restore.err", &connection);
    assert!(restored.borrow().is_none());

    transport.request(
        "state.restore",
        Some(serde_json::json!({ "count": "9" })),
        &connection,
    );
    let _ = receive(&mut server, "state.restore.ok", &connection);
    assert_eq!(
        restored.borrow().as_ref(),
        Some(&BTreeMap::from([("count".to_string(), "9".to_string())]))
    );

    // mirror.input needs a mirror subscription.
    transport.request("mirror.input", None, &connection);
    let _ = receive(&mut server, "mirror.input.err", &connection);
    transport.request("mirror.start", None, &connection);
    let _ = receive(&mut server, "mirror.start.ok", &connection);

    transport.request(
        "select.node",
        Some(serde_json::json!({ "id": "42" })),
        &connection,
    );
    let _ = receive(&mut server, "select.node.ok", &connection);

    transport.disconnect(&connection);
    server.pump();
    assert_eq!(
        *selections.borrow(),
        vec!["42".to_string(), "clear".to_string()]
    );
    assert_eq!(*mirror_stops.borrow(), 1, "the last mirror subscriber left");
    server.stop();
}

#[test]
fn state_diff_restore_result_and_recording_ownership() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let value = Rc::new(RefCell::new("2".to_string()));
    let stops = Rc::new(RefCell::new(0usize));
    let replays = Rc::new(RefCell::new(0usize));

    let capture_value = value.clone();
    server.host.checkpoint.capture = Some(Rc::new(move || {
        BTreeMap::from([("count".to_string(), capture_value.borrow().clone())])
    }));
    let restore_value = value.clone();
    server.host.checkpoint.restore = Some(Rc::new(move |state: &BTreeMap<String, String>| {
        let Some(next) = state.get("count") else {
            return false;
        };
        let Ok(integer) = next.parse::<i64>() else {
            return false;
        };
        if integer < 0 {
            return false;
        }
        *restore_value.borrow_mut() = next.clone();
        true
    }));
    server.host.recording.start = Some(Rc::new(|| true));
    let stop_counter = stops.clone();
    let stop_value = value.clone();
    server.host.recording.stop = Some(Rc::new(move || {
        *stop_counter.borrow_mut() += 1;
        Some(InputRecording::new(
            BTreeMap::from([("count".to_string(), stop_value.borrow().clone())]),
            None,
        ))
    }));
    let replay_counter = replays.clone();
    let replay_value = value.clone();
    server.host.recording.replay = Some(Rc::new(move |recording: &InputRecording| {
        *replay_counter.borrow_mut() += 1;
        if let Some(next) = recording.initial_state.get("count") {
            *replay_value.borrow_mut() = next.clone();
        }
        true
    }));

    server.start().expect("start");
    let owner = Arc::new(TestConnection::default());
    let other = Arc::new(TestConnection::default());
    connect(&mut server, &transport, &owner);
    connect(&mut server, &transport, &other);

    transport.request(
        "state.diff",
        Some(serde_json::json!({ "count": "1" })),
        &owner,
    );
    let difference = receive(&mut server, "state.diff.ok", &owner);
    let difference: StateDifference = decode_payload(&difference.payload).expect("diff payload");
    assert_eq!(
        difference
            .changed
            .get("count")
            .map(|change| change.after.as_str()),
        Some("2")
    );

    transport.request(
        "state.restore",
        Some(serde_json::json!({ "count": "-1" })),
        &owner,
    );
    let _ = receive(&mut server, "state.restore.err", &owner);
    assert_eq!(
        *value.borrow(),
        "2",
        "rejected restores leave the value alone"
    );

    transport.request("input.record.start", None, &owner);
    let _ = receive(&mut server, "input.record.start.ok", &owner);
    // Another connection cannot stop this recording.
    transport.request("input.record.stop", None, &other);
    let _ = receive(&mut server, "input.record.stop.err", &other);
    assert_eq!(*stops.borrow(), 0);

    transport.request("select.clear", None, &owner);
    let _ = receive(&mut server, "select.clear.ok", &owner);
    assert_eq!(
        *stops.borrow(),
        0,
        "an unrelated command does not stop recording"
    );

    transport.request("input.record.stop", None, &owner);
    let recording = receive(&mut server, "input.record.stop.ok", &owner);
    assert_eq!(*stops.borrow(), 1);

    transport.request("input.replay", recording.payload.clone(), &owner);
    let _ = receive(&mut server, "input.replay.ok", &owner);
    assert_eq!(*replays.borrow(), 1);

    // Disconnecting mid-recording stops it.
    transport.request("input.record.start", None, &owner);
    let _ = receive(&mut server, "input.record.start.ok", &owner);
    transport.disconnect(&owner);
    server.pump();
    assert_eq!(*stops.borrow(), 2);
    server.stop();
}

#[test]
fn temporary_inspection_is_exclusive_and_disconnect_cleans_only_its_lease() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let applied = Rc::new(RefCell::new(0usize));
    let resets = Rc::new(RefCell::new(0usize));

    let applied_counter = applied.clone();
    server.host.scene.inspect = Some(Rc::new(move |request: &Envelope| {
        *applied_counter.borrow_mut() += 1;
        Envelope::new(format!("{}.ok", request.r#type), request.id, None)
    }));
    let reset_counter = resets.clone();
    server.host.scene.reset = Some(Rc::new(move || *reset_counter.borrow_mut() += 1));
    server.start().expect("start");

    let owner = Arc::new(TestConnection::default());
    let other = Arc::new(TestConnection::default());
    connect(&mut server, &transport, &owner);
    connect(&mut server, &transport, &other);

    transport.request("inspect.pick.start", None, &owner);
    transport.request("inspect.pick.start", None, &other);
    let busy = receive(&mut server, "inspect.pick.start.err", &other);
    assert_eq!(code(&busy).as_deref(), Some("busy"));
    let _ = receive(&mut server, "inspect.pick.start.ok", &owner);
    assert_eq!(*applied.borrow(), 1);
    assert_eq!(*resets.borrow(), 0);

    // Disconnecting the lease holder resets the styles exactly once.
    transport.disconnect(&owner);
    server.pump();
    assert_eq!(*resets.borrow(), 1);

    // The other connection can now take the lease.
    transport.request("inspect.pick.start", None, &other);
    let _ = receive(&mut server, "inspect.pick.start.ok", &other);
    assert_eq!(*applied.borrow(), 2);
    server.stop();
    assert_eq!(*resets.borrow(), 2, "stopping resets the active lease");
}

#[test]
fn observation_streams_are_independent_and_timeline_stops_after_last_disconnect() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let one = Arc::new(TestConnection::default());
    let two = Arc::new(TestConnection::default());
    let reads = Rc::new(RefCell::new(Vec::<Vec<String>>::new()));
    let capture = Rc::new(RefCell::new(false));
    let value = Rc::new(RefCell::new("1".to_string()));

    let read_log = reads.clone();
    let number = value.clone();
    server.host.observation.state = Some(Rc::new(move |ids: &[String]| {
        read_log.borrow_mut().push(ids.to_vec());
        StateObservation {
            registered: vec![cove_devtools::protocol::RegisteredState {
                id: "count".into(),
                name: "count".into(),
                value_type: "Int".into(),
                scope_id: None,
            }],
            values: if ids.iter().any(|id| id == "count") {
                vec![cove_devtools::protocol::ObservedState {
                    id: "count".into(),
                    summary: number.borrow().clone(),
                    truncated: false,
                }]
            } else {
                Vec::new()
            },
        }
    }));
    let capture_flag = capture.clone();
    server.host.observation.capture = Some(Rc::new(move |enabled: bool| {
        *capture_flag.borrow_mut() = enabled
    }));
    server.host.observation.timeline = Some(Rc::new(|_after: u64| {
        cove_devtools::protocol::TimelineSnapshot {
            events: vec![cove_devtools::protocol::TimelineEvent {
                sequence: 1,
                phase: "draw".into(),
                name: "Draw".into(),
                scope_id: None,
                start_ms: 1.0,
                duration_ms: 2.0,
                reasons: Vec::new(),
            }],
            dropped: 0,
        }
    }));

    server.start().expect("start");
    server.broadcast_observations();
    assert!(reads.borrow().is_empty(), "no subscribers, no reads");
    assert!(!*capture.borrow());

    connect(&mut server, &transport, &one);
    connect(&mut server, &transport, &two);

    transport.request("state.list", None, &one);
    let metadata = receive(&mut server, "state.list.ok", &one);
    let observation: StateObservation = decode_payload(&metadata.payload).expect("state payload");
    assert!(
        observation.values.is_empty(),
        "state.list reads metadata only"
    );
    assert_eq!(reads.borrow().as_slice(), &[Vec::<String>::new()]);

    transport.request(
        "state.subscribe",
        Some(
            serde_json::to_value(StateWatch {
                ids: vec!["count".into()],
            })
            .expect("watch"),
        ),
        &one,
    );
    let _ = receive(&mut server, "state.subscribe.ok", &one);
    *value.borrow_mut() = "2".into();
    server.broadcast_observations();
    let updated = receive(&mut server, "state.observation", &one);
    let observation: StateObservation = decode_payload(&updated.payload).expect("state payload");
    assert_eq!(
        observation
            .values
            .first()
            .map(|value| value.summary.as_str()),
        Some("2")
    );
    // The other connection never subscribed, so it saw nothing.
    assert!(two.take().is_empty());

    transport.request("state.unsubscribe", None, &one);
    let _ = receive(&mut server, "state.unsubscribe.ok", &one);
    let before = reads.borrow().len();
    server.broadcast_observations();
    assert_eq!(
        reads.borrow().len(),
        before,
        "unsubscribing stops the reads"
    );

    transport.request("timeline.subscribe", None, &one);
    let _ = receive(&mut server, "timeline.events", &one);
    transport.request("timeline.subscribe", None, &two);
    let _ = receive(&mut server, "timeline.events", &two);
    assert!(*capture.borrow());

    transport.request("timeline.unsubscribe", None, &one);
    let _ = receive(&mut server, "timeline.unsubscribe.ok", &one);
    assert!(
        *capture.borrow(),
        "one subscriber remains, so capture continues"
    );

    transport.disconnect(&two);
    server.pump();
    assert!(!*capture.borrow(), "the last timeline subscriber left");
    server.stop();
}

#[test]
fn timing_frames_stream_only_to_subscribed_clients() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let subscriber = Arc::new(TestConnection::default());
    let observer = Arc::new(TestConnection::default());
    server.start().expect("start");
    connect(&mut server, &transport, &subscriber);
    connect(&mut server, &transport, &observer);

    fn frame(number: u64) -> TimingFrame {
        TimingFrame {
            frame: number,
            recomposition_ms: None,
            layout_ms: 1.0,
            draw_ms: 2.0,
            present_ms: 3.0,
            presented: Some(true),
            total_ms: 6.0,
            node_count: 4,
            batch_count: 1,
        }
    }

    // Silence before anyone subscribes: the publisher queues no work.
    server.broadcast_timing(frame(1));
    server.pump();
    assert!(
        !subscriber
            .take()
            .iter()
            .any(|message| message.r#type == "timing.frame")
    );

    transport.request("timing.subscribe", None, &subscriber);
    let _ = receive(&mut server, "timing.subscribe.ok", &subscriber);
    server.broadcast_timing(frame(2));
    let delivered = receive(&mut server, "timing.frame", &subscriber);
    let payload: TimingFrame = decode_payload(&delivered.payload).expect("timing payload");
    assert_eq!(payload.frame, 2);
    assert_eq!(payload.total_ms, 6.0);
    assert_eq!(payload.presented, Some(true));
    // The observer only receives frames after its own subscribe.
    assert!(
        !observer
            .take()
            .iter()
            .any(|message| message.r#type == "timing.frame")
    );
    server.stop();
}

#[test]
fn logs_replay_before_connection_filter_and_report_slow_consumer_loss() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    let label = format!("test.server.{}", std::process::id());
    let logger = cove_logging::LogSystem::logger(&label);

    let before = cove_logging::LogEvent::new(cove_logging::Level::Info, "below threshold");
    logger.log(&before);
    let structured = cove_logging::LogEvent::new(cove_logging::Level::Warning, "before connection")
        .with_metadata(BTreeMap::from([(
            "structured".to_string(),
            cove_logging::LogValue::Object(BTreeMap::from([(
                "nested".to_string(),
                cove_logging::LogValue::String("value".into()),
            )])),
        )]));
    logger.log(&structured);
    cove_logging::LogSystem::flush();

    server.start().expect("start");
    let connection = Arc::new(TestConnection::default());
    connect(&mut server, &transport, &connection);

    // `text` must be omitted rather than sent as null: the protocol's
    // `log.subscribe` validator requires `text` to be a string when the key is
    // present, and `LogQuery` serialises `None` as null. Swift's validator only
    // reached the text check when `query.text` was non-nil, so this is a
    // translation hazard worth pinning.
    let query = cove_logging::LogQuery {
        minimum: cove_logging::Level::Warning,
        labels: vec![label.clone()],
        text: None,
        after_sequence: None,
    };
    let mut query_payload = serde_json::to_value(&query).expect("query");
    query_payload
        .as_object_mut()
        .expect("object")
        .remove("text");
    transport.request("log.subscribe", Some(query_payload), &connection);
    let response = receive(&mut server, "log.snapshot", &connection);
    let snapshot: cove_logging::LogSnapshot =
        decode_payload(&response.payload).expect("log snapshot");
    assert_eq!(
        snapshot.records.len(),
        1,
        "the info record is below the threshold"
    );
    assert_eq!(snapshot.records[0].message, "before connection");
    assert_eq!(
        snapshot.records[0].metadata.get("structured"),
        Some(&cove_logging::LogValue::Object(BTreeMap::from([(
            "nested".to_string(),
            cove_logging::LogValue::String("value".into()),
        )])))
    );
    let last_sequence = snapshot.records.last().expect("record").sequence;

    logger.log(&cove_logging::LogEvent::new(
        cove_logging::Level::Warning,
        "live",
    ));
    cove_logging::LogSystem::flush();
    let live = receive(&mut server, "log.entry", &connection);
    let record: cove_logging::LogRecord = decode_payload(&live.payload).expect("log record");
    assert_eq!(record.message, "live");
    assert!(record.sequence > last_sequence);

    transport.request("log.unsubscribe", None, &connection);
    let _ = receive(&mut server, "log.unsubscribe.ok", &connection);
    logger.log(&cove_logging::LogEvent::new(
        cove_logging::Level::Warning,
        "after unsubscribe",
    ));
    cove_logging::LogSystem::flush();
    server.pump();
    assert!(
        !connection
            .take()
            .iter()
            .any(|message| message.r#type == "log.entry"),
        "an unsubscribed client receives no entries"
    );
    server.stop();
}

#[test]
fn window_resize_validates_and_reaches_the_host() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    server.start().expect("start");
    let connection = Arc::new(TestConnection::default());
    connect(&mut server, &transport, &connection);

    // Without a host hook the request is refused rather than silently dropped.
    transport.request(
        "window.resize",
        Some(serde_json::json!({ "width": 640, "height": 400 })),
        &connection,
    );
    let _ = receive(&mut server, "window.resize.err", &connection);

    let sizes = Rc::new(RefCell::new(Vec::<WindowResize>::new()));
    let recorded = sizes.clone();
    server.host.window.resize = Some(Rc::new(move |request: &WindowResize| {
        recorded.borrow_mut().push(request.clone());
        None
    }));

    transport.request(
        "window.resize",
        Some(serde_json::json!({ "width": 0, "height": 400 })),
        &connection,
    );
    let _ = receive(&mut server, "window.resize.err", &connection);
    assert!(
        sizes.borrow().is_empty(),
        "an out-of-range size never reaches the host"
    );

    transport.request(
        "window.resize",
        Some(serde_json::json!({ "width": 640, "height": 400 })),
        &connection,
    );
    let _ = receive(&mut server, "window.resize.ok", &connection);
    assert_eq!(sizes.borrow().len(), 1);
    assert_eq!(sizes.borrow()[0].width, 640.0);
    assert_eq!(sizes.borrow()[0].height, 400.0);
    server.stop();
}

#[test]
fn hello_advertises_the_protocol_version() {
    let transport = TestTransport::new();
    let mut server = server(&transport);
    server.start().expect("start");
    let connection = Arc::new(TestConnection::default());
    transport.connect(&connection);
    let hello = receive(&mut server, "hello", &connection);
    let payload: cove_devtools::protocol::HelloPayload =
        decode_payload(&hello.payload).expect("hello payload");
    assert_eq!(payload.protocol, PROTOCOL_VERSION);
    assert!(!payload.capabilities.is_empty());
    server.stop();
}
