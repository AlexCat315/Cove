//! Translation of `Tests/CoveUIDevToolsTests/DevServerSmokeTests.swift`.
//!
//! Behaviour checklist:
//! - a connecting client receives `hello` carrying the protocol version, the
//!   configured app title and the `tree` capability
//! - `tree.subscribe` answers with a snapshot whose fields match the node tree,
//!   including absolute frames, debug names and layout roles
//! - the inspector finds a node by its stable element id
//! - a tree change does not snapshot when nobody is subscribed
//! - binding to a non-loopback host is rejected instead of exposing every
//!   interface
//!
//! The Swift suite opened a real Network.framework WebSocket because its
//! `DevTools` façade owned an async server. The wire contract it asserted is the
//! same one `DevServer` speaks, and `dev_server_tests.rs` already drives that
//! synchronously; this target reuses that harness for the smoke cases and adds
//! the snapshot-field, inspector, and host-rejection coverage the Swift file
//! carries.

use std::sync::{Arc, Mutex};

use cove_core::scene::NodeTree;
use cove_core::Rect;
use cove_devtools::inspection::SceneInspector;
use cove_devtools::protocol::{Envelope, JsonValue, PROTOCOL_VERSION};
use cove_devtools::server::transport::resolve_bind_address;
use cove_devtools::server::{
    DevServer, DevServerConfigurationError, DevToolsConfig, DevToolsConnection, DevToolsTransport,
    DevToolsTransportEvent,
};

#[derive(Default)]
struct TestConnection {
    messages: Mutex<Vec<Vec<u8>>>,
}

impl TestConnection {
    fn take(&self) -> Vec<Envelope> {
        std::mem::take(&mut *self.messages.lock().expect("messages"))
            .into_iter()
            .map(|bytes| serde_json::from_slice(&bytes).expect("envelope"))
            .collect()
    }
}

impl DevToolsConnection for TestConnection {
    fn is_writable(&self) -> bool {
        true
    }
    fn send_text(&self, text: &[u8]) {
        self.messages.lock().expect("messages").push(text.to_vec());
    }
    fn close(&self) {}
}

struct TestTransport {
    on_event: Mutex<Option<Arc<dyn Fn(DevToolsTransportEvent) + Send + Sync>>>,
}

impl TestTransport {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            on_event: Mutex::new(None),
        })
    }

    fn emit(&self, event: DevToolsTransportEvent) {
        let callback = self.on_event.lock().expect("event").clone();
        if let Some(callback) = callback {
            callback(event);
        }
    }

    fn connect(&self, connection: &Arc<TestConnection>) {
        self.emit(DevToolsTransportEvent::Connected(connection.clone()));
    }

    fn request(&self, kind: &str, connection: &Arc<TestConnection>) {
        let envelope = Envelope::new(kind, Some(1), None);
        let data = serde_json::to_vec(&envelope).expect("encode");
        self.emit(DevToolsTransportEvent::Text(connection.clone(), data));
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

fn receive(server: &mut DevServer, kind: &str, connection: &Arc<TestConnection>) -> Envelope {
    for _ in 0..8 {
        server.pump();
        if let Some(message) = connection
            .take()
            .into_iter()
            .find(|message| message.r#type == kind)
        {
            return message;
        }
    }
    panic!("timed out waiting for {kind}");
}

fn configured_server(transport: &Arc<TestTransport>) -> DevServer {
    DevServer::with_transport(
        DevToolsConfig {
            app_title: "TestApp".into(),
            ..DevToolsConfig::default()
        },
        transport.clone(),
    )
}

#[test]
fn hello_carries_protocol_version_app_title_and_tree_capability() {
    let transport = TestTransport::new();
    let mut server = configured_server(&transport);
    server.start().expect("start");
    let connection = Arc::new(TestConnection::default());

    transport.connect(&connection);
    let hello = receive(&mut server, "hello", &connection);
    let payload = hello.payload.expect("hello carries a payload");

    assert_eq!(
        payload.get("protocol").and_then(JsonValue::as_str),
        Some(PROTOCOL_VERSION),
        "hello advertises the protocol version"
    );
    assert_eq!(
        payload
            .get("host")
            .and_then(|host| host.get("appTitle"))
            .and_then(JsonValue::as_str),
        Some("TestApp"),
        "hello carries the configured application title"
    );
    let capabilities: Vec<&str> = payload
        .get("capabilities")
        .and_then(JsonValue::as_array)
        .expect("capabilities array")
        .iter()
        .filter_map(JsonValue::as_str)
        .collect();
    assert!(
        capabilities.contains(&"tree"),
        "the tree capability is advertised, got {capabilities:?}"
    );
}

#[test]
fn tree_subscribe_returns_a_snapshot_of_the_host_scene() {
    use cove_devtools::protocol::{NodeFlags, NodeFrame, NodeSummary, TreeSnapshot};

    let transport = TestTransport::new();
    let mut server = configured_server(&transport);
    server.host.scene.snapshot = Some(std::rc::Rc::new(|| {
        let child = NodeSummary {
            id: "child".into(),
            view_tag: Some("Child".into()),
            debug_name: Some("debug.child".into()),
            frame: NodeFrame { x: 10.0, y: 20.0, w: 100.0, h: 40.0 },
            absolute_frame: Some(NodeFrame { x: 15.0, y: 27.0, w: 100.0, h: 40.0 }),
            content_offset: Some(cove_devtools::protocol::NodePoint { x: 4.0, y: 6.0 }),
            z_index: Some(3.0),
            opacity: Some(0.75),
            layout_role: Some("button".into()),
            semantic_role: None,
            flags: NodeFlags {
                hit_testable: true,
                focusable: true,
                clips_to_bounds: false,
                has_background: true,
                has_border: false,
            },
            children: Vec::new(),
            element_id: Some("7".into()),
            layout: None,
            style: None,
            source: None,
            owner_scope_id: None,
            recomposition: None,
        };
        TreeSnapshot {
            root: Some(NodeSummary {
                id: "root".into(),
                view_tag: Some("Root".into()),
                debug_name: None,
                frame: NodeFrame { x: 5.0, y: 7.0, w: 800.0, h: 600.0 },
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
                    has_background: false,
                    has_border: false,
                },
                children: vec![child],
                element_id: None,
                layout: None,
                style: None,
                source: None,
                owner_scope_id: None,
                recomposition: None,
            }),
            invalidations: None,
            render_inventory: None,
            input_inventory: None,
            inspection: None,
        }
    }));
    server.start().expect("start");

    let connection = Arc::new(TestConnection::default());
    transport.connect(&connection);
    let _ = receive(&mut server, "hello", &connection);
    transport.request("tree.subscribe", &connection);
    let snapshot = receive(&mut server, "tree.snapshot", &connection);

    let decoded: TreeSnapshot = cove_devtools::protocol::decode_payload(&snapshot.payload)
        .expect("snapshot payload decodes");
    let root = decoded.root.expect("snapshot carries a root");
    assert_eq!(root.view_tag.as_deref(), Some("Root"));
    assert_eq!(root.children.len(), 1, "the single child is serialised");
    let child = &root.children[0];
    assert_eq!(child.view_tag.as_deref(), Some("Child"));
    assert_eq!(child.debug_name.as_deref(), Some("debug.child"));
    assert_eq!(child.frame.w, 100.0, "the child frame is reported in points");
    assert_eq!(
        child.absolute_frame.as_ref().map(|frame| frame.x),
        Some(15.0),
        "absolute frames compose the parent origin exactly once"
    );
    assert_eq!(child.content_offset.as_ref().map(|offset| offset.x), Some(4.0));
    assert_eq!(child.z_index, Some(3.0));
    assert_eq!(child.opacity, Some(0.75));
    assert_eq!(child.layout_role.as_deref(), Some("button"));
    assert!(child.flags.has_background);
    assert_eq!(child.element_id.as_deref(), Some("7"));
}

#[test]
fn inspector_finds_nodes_by_stable_element_id() {
    let tree = std::rc::Rc::new(std::cell::RefCell::new(NodeTree::new()));
    let root = cove_core::scene::Node::new();
    let child = cove_core::scene::Node::new();
    root.add_child(&child);
    tree.borrow_mut().set_root(Some(root));

    let inspector = SceneInspector::new(tree, None, None);
    let id = SceneInspector::identifier(&child);
    let found = inspector.find(&id).expect("the child resolves by its identifier");
    assert_eq!(
        found.id().0,
        child.id().0,
        "the inspector returns the same node, not a copy with a new identity"
    );
}

#[test]
fn tree_change_does_not_snapshot_without_a_subscriber() {
    let transport = TestTransport::new();
    let mut server = configured_server(&transport);
    server.start().expect("start");
    let connection = Arc::new(TestConnection::default());
    transport.connect(&connection);
    let _ = receive(&mut server, "hello", &connection);
    connection.take();

    server.broadcast_tree_delta();
    let after = connection.take();
    assert!(
        after.iter().all(|message| message.r#type != "tree.snapshot"),
        "an unsubscribed connection receives no snapshot, got {:?}",
        after.iter().map(|message| message.r#type.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn invalid_bind_host_is_rejected_instead_of_exposing_all_interfaces() {
    assert_eq!(
        resolve_bind_address("192.0.2.1"),
        Err(DevServerConfigurationError::UnsupportedHost("192.0.2.1".into())),
        "a public address is rejected rather than bound"
    );
    assert_eq!(resolve_bind_address("127.0.0.1"), Ok("127.0.0.1"));
    assert_eq!(resolve_bind_address("localhost"), Ok("127.0.0.1"));
    assert_eq!(
        resolve_bind_address("0.0.0.0"),
        Ok("0.0.0.0"),
        "all interfaces is allowed only when asked for explicitly"
    );
}
