//! Persistent, bounded WebSocket client. Never replay a request after failure.
use cove_agent::AgentError;
use cove_devtools_protocol::{Envelope, HelloPayload, PROTOCOL_VERSION};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket, client::client_with_config, protocol::WebSocketConfig};
const EVENT_BUFFER_BYTES: usize = 2 * 1024 * 1024;

pub(crate) fn failure(message: impl Into<String>) -> AgentError {
    AgentError::InvalidConfiguration(message.into())
}

/// Resolve on each connection so a restarted app can replace its descriptor.
pub fn endpoint(target: &str) -> Result<String, AgentError> {
    if let Some(path) = target.strip_prefix('@') {
        let file = std::fs::File::open(Path::new(path))
            .map_err(|_| failure("Cannot read DevTools session file."))?;
        let mut bytes = Vec::new();
        file.take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| failure("Cannot read DevTools session file."))?;
        if bytes.len() > 65_536 {
            return Err(failure("DevTools session file exceeds 64 KiB."));
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| failure("Invalid DevTools session file JSON."))?;
        return value
            .get("endpoint")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| failure("DevTools session file requires endpoint."));
    }
    if let Ok(port) = target.parse::<u16>() {
        if port == 0 {
            return Err(failure("DevTools port must be nonzero."));
        }
        return Ok(format!("ws://127.0.0.1:{port}/"));
    }
    Ok(target.to_owned())
}

pub struct Connection {
    socket: WebSocket<TcpStream>,
    pub hello: HelloPayload,
    id: i64,
    events: VecDeque<Envelope>,
    event_bytes: usize,
    pub dropped: usize,
    pub healthy: bool,
    pub mirroring: bool,
}

impl Connection {
    pub fn connect(target: &str) -> Result<Self, AgentError> {
        let endpoint = endpoint(target)?;
        let uri: tungstenite::http::Uri = endpoint.parse().map_err(|_| {
            failure("Invalid DevTools endpoint; use a port, ws:// URL or @session.json.")
        })?;
        if uri.scheme_str() != Some("ws") || uri.authority().is_none() {
            return Err(failure(
                "DevTools requires a ws:// endpoint (use a local forwarded port for devices).",
            ));
        }
        let host = uri.host().unwrap_or_default().trim_matches(['[', ']']);
        if host.contains('@') {
            return Err(failure("Endpoint userinfo is unsupported."));
        }
        let addresses = (host, uri.port_u16().unwrap_or(80))
            .to_socket_addrs()
            .map_err(|_| failure("Cannot resolve DevTools host."))?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = None;
        for address in addresses {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            if let Ok(socket) = TcpStream::connect_timeout(&address, remaining) {
                stream = Some(socket);
                break;
            }
        }
        let stream = stream.ok_or_else(|| failure("Cannot connect to DevTools. Start the app with COVE_DEVTOOLS=1; check --endpoint or the device port forwarding."))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|_| failure("Cannot set socket timeout."))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|_| failure("Cannot set socket timeout."))?;
        let _ = stream.set_nodelay(true);
        let config = WebSocketConfig::default()
            .max_message_size(Some(8 * 1024 * 1024))
            .max_frame_size(Some(8 * 1024 * 1024));
        // Error text from HTTP libraries may include the URL's authentication token.
        let (socket, _) =
            client_with_config(endpoint.as_str(), stream, Some(config)).map_err(|_| {
                failure(
                    "DevTools WebSocket handshake failed; check the session token and endpoint.",
                )
            })?;
        let mut connection = Self {
            socket,
            hello: HelloPayload::new(
                cove_devtools_protocol::HelloHostInfo {
                    pid: 0,
                    app_title: String::new(),
                    platform: String::new(),
                },
                Vec::new(),
            ),
            id: 0,
            events: VecDeque::new(),
            event_bytes: 0,
            dropped: 0,
            healthy: true,
            mirroring: false,
        };
        let hello = connection
            .receive(Instant::now() + Duration::from_secs(3))?
            .ok_or_else(|| failure("Timed out waiting for DevTools hello."))?;
        if hello.r#type != "hello" {
            return Err(failure("Expected DevTools hello."));
        }
        connection.hello = serde_json::from_value(hello.payload.unwrap_or(Value::Null))
            .map_err(|_| failure("Malformed DevTools hello."))?;
        if connection.hello.protocol != PROTOCOL_VERSION {
            return Err(failure(format!(
                "Unsupported DevTools protocol; expected {PROTOCOL_VERSION}."
            )));
        }
        connection.send(&Envelope::new("hello.ack", None, None))?;
        Ok(connection)
    }

    pub fn require(&self, capability: &str) -> Result<(), AgentError> {
        if self
            .hello
            .capabilities
            .iter()
            .any(|name| name == capability)
        {
            return Ok(());
        }
        Err(failure(format!(
            "Host does not advertise {capability}; use get_ui_session to inspect available capabilities."
        )))
    }

    fn send(&mut self, message: &Envelope) -> Result<(), AgentError> {
        let text = serde_json::to_string(message)
            .map_err(|_| failure("Cannot encode DevTools request."))?;
        if self.socket.send(Message::Text(text.into())).is_err() {
            self.healthy = false;
            return Err(failure(
                "DevTools connection lost. Delivery is uncertain; the request was not retried.",
            ));
        }
        Ok(())
    }

    fn receive(&mut self, deadline: Instant) -> Result<Option<Envelope>, AgentError> {
        loop {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(None);
            };
            self.socket
                .get_ref()
                .set_read_timeout(Some(remaining.min(Duration::from_millis(100))))
                .map_err(|_| failure("Cannot set socket timeout."))?;
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    let envelope = serde_json::from_str(&text).map_err(|_| {
                        self.healthy = false;
                        failure("Malformed DevTools response.")
                    })?;
                    return Ok(Some(envelope));
                }
                Ok(Message::Ping(_)) => {
                    let _ = self.socket.flush();
                }
                Ok(Message::Pong(_)) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                _ => {
                    self.healthy = false;
                    return Err(failure(
                        "DevTools connection closed. Delivery is uncertain; reconnect on the next call. Mutations are never retried automatically.",
                    ));
                }
            }
        }
    }

    fn remember(&mut self, event: Envelope) {
        let bytes = serde_json::to_vec(&event).map_or(0, |bytes| bytes.len());
        if bytes > EVENT_BUFFER_BYTES {
            self.dropped += event_loss(&event);
            return;
        }
        while self.events.len() >= 256 || self.event_bytes + bytes > EVENT_BUFFER_BYTES {
            let Some(old) = self.events.pop_front() else {
                break;
            };
            self.event_bytes = self
                .event_bytes
                .saturating_sub(serde_json::to_vec(&old).map_or(0, |bytes| bytes.len()));
            self.dropped += event_loss(&old);
        }
        self.event_bytes += bytes;
        self.events.push_back(event);
    }

    pub fn request(&mut self, kind: &str, payload: Option<Value>) -> Result<Value, AgentError> {
        self.request_with_missing(kind, payload, false)
    }

    fn request_with_missing(
        &mut self,
        kind: &str,
        payload: Option<Value>,
        allow_missing: bool,
    ) -> Result<Value, AgentError> {
        self.id += 1;
        let id = self.id;
        self.send(&Envelope::new(kind, Some(id), payload))?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while let Some(message) = self.receive(deadline)? {
            if message.id == Some(id) {
                if message.r#type.ends_with(".err") {
                    if allow_missing
                        && message
                            .payload
                            .as_ref()
                            .is_some_and(|payload| payload["code"] == "stale_node")
                    {
                        return Ok(
                            json!({"ok": true, "data": {"items": [], "total": 0, "nextOffset": null}}),
                        );
                    }
                    return Err(failure(format!(
                        "{kind}: {}",
                        message
                            .payload
                            .as_ref()
                            .and_then(|value| value.get("message"))
                            .and_then(Value::as_str)
                            .unwrap_or("Host rejected request.")
                    )));
                }
                if kind == "mirror.start" {
                    self.mirroring = true;
                }
                if kind == "mirror.stop" {
                    self.mirroring = false;
                }
                return Ok(message.payload.unwrap_or_else(|| json!({})));
            }
            self.remember(message);
        }
        self.healthy = false;
        Err(failure(format!(
            "Timed out waiting for {kind}. Delivery is uncertain; the request was not retried."
        )))
    }

    pub fn diagnostic(&mut self, name: &str, mut arguments: Value) -> Result<Value, AgentError> {
        self.require("diagnostics")?;
        if name == "query_ui_nodes" && arguments.get("compact").is_none_or(Value::is_null) {
            arguments["compact"] = json!(true);
        }
        self.request(
            "diagnostics.call",
            Some(json!({ "name": name, "arguments": arguments })),
        )
    }

    pub fn poll_nodes(
        &mut self,
        arguments: Value,
        allow_missing: bool,
    ) -> Result<Value, AgentError> {
        self.require("diagnostics")?;
        self.request_with_missing(
            "diagnostics.call",
            Some(json!({"name": "query_ui_nodes", "arguments": arguments})),
            allow_missing,
        )
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
        self.event_bytes = 0;
        self.dropped = 0;
    }

    /// Drain while MCP is idle so an active timeline never blocks the UI host.
    pub fn poll(&mut self) {
        for _ in 0..32 {
            match self.receive(Instant::now() + Duration::from_millis(1)) {
                Ok(Some(event)) => self.remember(event),
                _ => break,
            }
        }
    }

    pub fn discard_events(&mut self, kind: &str) {
        self.events.retain(|event| event.r#type != kind);
        self.event_bytes = self
            .events
            .iter()
            .map(|event| serde_json::to_vec(event).map_or(0, |bytes| bytes.len()))
            .sum();
    }

    pub fn wait_event(
        &mut self,
        kind: &str,
        deadline: Instant,
    ) -> Result<Option<Value>, AgentError> {
        loop {
            if let Some(position) = self.events.iter().position(|event| {
                event.r#type == kind || (event.r#type == "mirror.stopped" && kind == "mirror.frame")
            }) {
                let event = self.events.remove(position).expect("position exists");
                self.event_bytes = self
                    .event_bytes
                    .saturating_sub(serde_json::to_vec(&event).map_or(0, |bytes| bytes.len()));
                if event.r#type == kind {
                    return Ok(event.payload);
                }
                if event.r#type == "mirror.stopped" && kind == "mirror.frame" {
                    return Err(failure(
                        "Host stopped mirror capture before a frame was available.",
                    ));
                }
            }
            let Some(event) = self.receive(deadline)? else {
                return Ok(None);
            };
            self.remember(event);
        }
    }
}

fn event_loss(event: &Envelope) -> usize {
    event
        .payload
        .as_ref()
        .and_then(|value| value.get("events"))
        .and_then(Value::as_array)
        .map_or(1, Vec::len)
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.socket.close(None);
        let _ = self.socket.get_ref().shutdown(std::net::Shutdown::Both);
    }
}
