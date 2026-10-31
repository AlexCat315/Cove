//! WebSocket transport with no async runtime dependency.
//!
//! The Swift server used SwiftNIO. NIO is not in this workspace, so this module
//! implements the same wire behaviour directly: an HTTP/1.1 handshake on
//! `127.0.0.1:<port>/?token=…`, RFC 6455 text framing with masking enforcement,
//! ping/pong, close, and bounded fragment reassembly. It runs one accept thread
//! plus one reader thread per connection and delivers events through a channel,
//! so the server keeps its single-threaded protocol core.
use crate::server::config::DevServerConfigurationError;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A connected client. Identity is stable until disconnection.
pub trait DevToolsConnection: Send + Sync {
    fn is_writable(&self) -> bool;
    fn send_text(&self, text: &[u8]);
    fn close(&self);
}

#[derive(Clone)]
pub enum DevToolsTransportEvent {
    Connected(Arc<dyn DevToolsConnection>),
    Text(Arc<dyn DevToolsConnection>, Vec<u8>),
    Disconnected(Arc<dyn DevToolsConnection>),
}

/// Socket-independent boundary for protocol dispatch.
pub trait DevToolsTransport {
    fn bound_port(&self) -> Option<u16>;
    fn start(
        &self,
        host: &str,
        port: u16,
        on_event: Arc<dyn Fn(DevToolsTransportEvent) + Send + Sync>,
    ) -> io::Result<()>;
    fn stop(&self);
}

/// Reject a host that would expose DevTools beyond loopback unless the caller
/// explicitly asked for all interfaces.
pub fn resolve_bind_address(host: &str) -> Result<&'static str, DevServerConfigurationError> {
    match host {
        "localhost" | "127.0.0.1" => Ok("127.0.0.1"),
        "::1" => Ok("::1"),
        "0.0.0.0" => Ok("0.0.0.0"),
        "::" => Ok("::"),
        "*" => Ok("0.0.0.0"),
        other => Err(DevServerConfigurationError::UnsupportedHost(other.into())),
    }
}

impl From<DevServerConfigurationError> for io::Error {
    fn from(error: DevServerConfigurationError) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, error.message())
    }
}

pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// Loopback WebSocket transport.
pub struct StdDevToolsTransport {
    access_token: Option<String>,
    state: Mutex<TransportState>,
}

struct TransportState {
    listener: Option<TcpListener>,
    shutdown: Arc<AtomicBool>,
    port: Option<u16>,
    threads: Vec<std::thread::JoinHandle<()>>,
    connections: Vec<Arc<StdConnection>>,
}

impl StdDevToolsTransport {
    pub fn new(access_token: Option<String>) -> Self {
        Self {
            access_token,
            state: Mutex::new(TransportState {
                listener: None,
                shutdown: Arc::new(AtomicBool::new(false)),
                port: None,
                threads: Vec::new(),
                connections: Vec::new(),
            }),
        }
    }
}

impl DevToolsTransport for StdDevToolsTransport {
    fn bound_port(&self) -> Option<u16> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .port
    }

    fn start(
        &self,
        host: &str,
        port: u16,
        on_event: Arc<dyn Fn(DevToolsTransportEvent) + Send + Sync>,
    ) -> io::Result<()> {
        let address = resolve_bind_address(host)?;
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.listener.is_some() {
            return Ok(());
        }
        let listener = TcpListener::bind((address, port))?;
        let bound = listener.local_addr()?.port();
        listener.set_nonblocking(false)?;
        let token = self.access_token.clone();
        state.shutdown.store(false, Ordering::SeqCst);
        let shutdown = state.shutdown.clone();
        state.port = Some(bound);
        // The accept loop owns a clone; `stop` drops the original handle, flips
        // the flag and connects to itself to break the blocking accept.
        let accept_listener = listener.try_clone()?;
        state.listener = Some(listener);
        let connections = Arc::new(Mutex::new(Vec::<Arc<StdConnection>>::new()));
        let registry = connections.clone();
        let thread = std::thread::Builder::new()
            .name("cove.devtools.accept".into())
            .spawn(move || {
                for incoming in accept_listener.incoming() {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = incoming else { continue };
                    let token = token.clone();
                    let on_event = on_event.clone();
                    let registry = registry.clone();
                    let _ = std::thread::Builder::new()
                        .name("cove.devtools.client".into())
                        .spawn(move || {
                            if let Some(connection) = handshake(stream, token.as_deref()) {
                                registry
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner())
                                    .push(connection.clone());
                                on_event(DevToolsTransportEvent::Connected(connection.clone()));
                                read_loop(&connection, &on_event);
                                on_event(DevToolsTransportEvent::Disconnected(connection));
                            }
                        });
                }
            })?;
        state.threads.push(thread);
        Ok(())
    }

    fn stop(&self) {
        let state = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.shutdown.store(true, Ordering::SeqCst);
            let port = state.port;
            state.listener.take();
            (
                port,
                state.connections.clone(),
                std::mem::take(&mut state.threads),
            )
        };
        // Wake the blocking accept with a throwaway connection.
        if let Some(port) = state.0 {
            let _ = TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(100),
            );
        }
        for connection in &state.1 {
            connection.close();
        }
        for thread in state.2 {
            let _ = thread.join();
        }
    }
}

impl Drop for StdDevToolsTransport {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One accepted client.
pub struct StdConnection {
    stream: Mutex<TcpStream>,
    writer: Mutex<SendHalf>,
    writable: AtomicBool,
    closed: AtomicBool,
}

struct SendHalf {
    stream: TcpStream,
}

impl DevToolsConnection for StdConnection {
    fn is_writable(&self) -> bool {
        !self.closed.load(Ordering::SeqCst) && self.writable.load(Ordering::SeqCst)
    }

    fn send_text(&self, text: &[u8]) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        // A stalled mirror client must not accumulate unbounded frames.
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if write_frame(&mut writer.stream, Opcode::Text, text).is_err() {
            self.close();
        }
    }

    fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _ = write_frame(&mut writer.stream, Opcode::Close, &[0x03, 0xE8]);
        let _ = writer.stream.shutdown(std::net::Shutdown::Both);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

fn opcode_of(byte: u8) -> Option<Opcode> {
    Some(match byte & 0x0F {
        0x0 => Opcode::Continuation,
        0x1 => Opcode::Text,
        0x2 => Opcode::Binary,
        0x8 => Opcode::Close,
        0x9 => Opcode::Ping,
        0xA => Opcode::Pong,
        _ => return None,
    })
}

/// Perform the HTTP upgrade. Returns `None` (and closes) when the request is
/// not an acceptable WebSocket handshake, including a bad or missing token.
fn handshake(mut stream: TcpStream, access_token: Option<&str>) -> Option<Arc<StdConnection>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    // Read until the end of the HTTP head.
    loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 16 * 1024 {
            return None;
        }
    }
    let head = String::from_utf8_lossy(&buffer);
    let mut lines = head.lines();
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return None;
    }
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    if path != "/" {
        let _ = respond_forbidden(&mut stream);
        return None;
    }
    let query = target.split_once('?').map(|(_, query)| query).unwrap_or("");
    if let Some(expected) = access_token {
        let supplied = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("token="));
        if supplied != Some(expected) {
            let _ = respond_forbidden(&mut stream);
            return None;
        }
    }
    let mut key = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("sec-websocket-key") {
            key = Some(value.trim().to_string());
        }
    }
    let key = key?;
    let accept = base64(&sha1(
        format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
    ));
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).ok()?;
    stream.flush().ok()?;
    let writer = stream.try_clone().ok()?;
    Some(Arc::new(StdConnection {
        stream: Mutex::new(stream),
        writer: Mutex::new(SendHalf { stream: writer }),
        writable: AtomicBool::new(true),
        closed: AtomicBool::new(false),
    }))
}

fn respond_forbidden(stream: &mut TcpStream) -> io::Result<()> {
    stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

/// Read frames until the peer closes. Fragments are reassembled with a total
/// limit, including when pings interrupt them.
fn read_loop(
    connection: &Arc<StdConnection>,
    on_event: &Arc<dyn Fn(DevToolsTransportEvent) + Send + Sync>,
) {
    let mut fragment_opcode: Option<Opcode> = None;
    let mut fragments: Vec<u8> = Vec::new();
    loop {
        let frame = {
            let mut stream = connection
                .stream
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            read_frame(&mut stream)
        };
        let Some((fin, opcode, payload, masked)) = frame else {
            break;
        };
        // Clients must mask every frame.
        if !masked {
            connection.close();
            break;
        }
        match opcode {
            Opcode::Ping => {
                let mut writer = connection
                    .writer
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let _ = write_frame(&mut writer.stream, Opcode::Pong, &payload);
            }
            Opcode::Pong => {}
            Opcode::Close => {
                connection.close();
                break;
            }
            Opcode::Text | Opcode::Binary => {
                if fragment_opcode.is_some() {
                    connection.close();
                    break;
                }
                fragment_opcode = Some(opcode);
                if !append_fragment(connection, &mut fragments, &payload) {
                    break;
                }
                if !fin {
                    continue;
                }
                let opcode = fragment_opcode.take();
                let data = std::mem::take(&mut fragments);
                if opcode == Some(Opcode::Text) {
                    if std::str::from_utf8(&data).is_err() {
                        connection.close();
                        break;
                    }
                    on_event(DevToolsTransportEvent::Text(connection.clone(), data));
                }
            }
            Opcode::Continuation => {
                if fragment_opcode.is_none() {
                    connection.close();
                    break;
                }
                if !append_fragment(connection, &mut fragments, &payload) {
                    break;
                }
                if !fin {
                    continue;
                }
                let opcode = fragment_opcode.take();
                let data = std::mem::take(&mut fragments);
                if opcode == Some(Opcode::Text) {
                    if std::str::from_utf8(&data).is_err() {
                        connection.close();
                        break;
                    }
                    on_event(DevToolsTransportEvent::Text(connection.clone(), data));
                }
            }
        }
    }
}

fn append_fragment(
    connection: &Arc<StdConnection>,
    fragments: &mut Vec<u8>,
    payload: &[u8],
) -> bool {
    if fragments.len() + payload.len() > MAX_MESSAGE_BYTES {
        connection.close();
        return false;
    }
    fragments.extend_from_slice(payload);
    true
}

fn read_exact_or_none(stream: &mut TcpStream, buffer: &mut [u8]) -> Option<()> {
    stream.read_exact(buffer).ok()
}

fn read_frame(stream: &mut TcpStream) -> Option<(bool, Opcode, Vec<u8>, bool)> {
    let mut header = [0u8; 2];
    read_exact_or_none(stream, &mut header)?;
    let fin = header[0] & 0x80 != 0;
    let opcode = opcode_of(header[0])?;
    let masked = header[1] & 0x80 != 0;
    let short_length = header[1] & 0x7F;
    let length = match short_length {
        126 => {
            let mut extended = [0u8; 2];
            read_exact_or_none(stream, &mut extended)?;
            u16::from_be_bytes(extended) as usize
        }
        127 => {
            let mut extended = [0u8; 8];
            read_exact_or_none(stream, &mut extended)?;
            let value = u64::from_be_bytes(extended);
            // A control frame claiming a huge length is malformed.
            usize::try_from(value)
                .ok()
                .filter(|length| *length <= MAX_MESSAGE_BYTES)?
        }
        other => other as usize,
    };
    if length > MAX_MESSAGE_BYTES {
        return None;
    }
    let mask = if masked {
        let mut key = [0u8; 4];
        read_exact_or_none(stream, &mut key)?;
        Some(key)
    } else {
        None
    };
    let mut payload = vec![0u8; length];
    if length > 0 {
        read_exact_or_none(stream, &mut payload)?;
    }
    if let Some(mask) = mask {
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }
    }
    Some((fin, opcode, payload, masked))
}

fn write_frame(stream: &mut TcpStream, opcode: Opcode, payload: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 10);
    frame.push(0x80 | opcode as u8);
    // Server frames are never masked.
    match payload.len() {
        length if length < 126 => frame.push(length as u8),
        length if length <= u16::MAX as usize => {
            frame.push(126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()
}

// MARK: - SHA-1 (handshake only)

/// SHA-1 for the WebSocket accept token. This is a protocol fixed point, not a
/// security primitive: the handshake requires exactly this digest.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = [
        0x67452301u32,
        0xEFCDAB89,
        0x98BADCFE,
        0x10325476,
        0xC3D2E1F0,
    ];
    let mut message = data.to_vec();
    let bit_length = (data.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    for chunk in message.chunks(64) {
        let mut words = [0u32; 80];
        for index in 0..16 {
            words[index] = u32::from_be_bytes([
                chunk[index * 4],
                chunk[index * 4 + 1],
                chunk[index * 4 + 2],
                chunk[index * 4 + 3],
            ]);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | (!b & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temporary = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            // Standard SHA-1 rotation: the 30-bit rotate belongs on `c` (the
            // incoming `b`), while `b` simply becomes the old `a`. Swapping the
            // two — `c = b` with `b = a.rotate_left(30)` — still produces a
            // plausible-looking digest but fails every published vector.
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temporary;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut digest = [0u8; 20];
    for (index, word) in h.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = ((chunk[0] as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | *chunk.get(2).unwrap_or(&0) as u32;
        output.push(ALPHABET[((triple >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((triple >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(triple & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

/// Inbox the server drains each frame.
pub type EventReceiver = std::sync::mpsc::Receiver<DevToolsTransportEvent>;

/// Blocking event pump for a synchronous host. `None` when the transport
/// stopped or no event arrived within `timeout`.
pub fn recv_timeout(events: &EventReceiver, timeout: Duration) -> Option<DevToolsTransportEvent> {
    events.recv_timeout(timeout).ok()
}

/// Create the channel pair the server uses to consume transport events.
pub fn event_channel() -> (Sender<DevToolsTransportEvent>, EventReceiver) {
    channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_the_fips_vectors() {
        let digest = sha1(b"abc");
        assert_eq!(
            digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        let digest = sha1(b"");
        assert_eq!(
            digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }

    #[test]
    fn websocket_accept_token_matches_the_rfc_example() {
        // RFC 6455 section 1.3.
        let accept = base64(&sha1(
            b"dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11",
        ));
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn unsupported_hosts_are_rejected() {
        assert_eq!(resolve_bind_address("localhost").ok(), Some("127.0.0.1"));
        assert_eq!(resolve_bind_address("*").ok(), Some("0.0.0.0"));
        assert!(resolve_bind_address("example.com").is_err());
    }
}

impl std::fmt::Debug for DevToolsTransportEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected(_) => formatter.write_str("Connected"),
            Self::Text(_, data) => write!(formatter, "Text({} bytes)", data.len()),
            Self::Disconnected(_) => formatter.write_str("Disconnected"),
        }
    }
}
