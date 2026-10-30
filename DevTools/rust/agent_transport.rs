//! Loopback JSON-lines transport for MCP or a host's private tool adapter.
//!
//! The handler receives bytes and returns bytes; protocol and application data
//! stay outside the transport, so the same handler serves stdio, a socket or an
//! embedded pipe.
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Largest single line accepted from a client.
pub const MAX_LINE_BYTES: usize = 1_048_576;
/// Largest response written back.
pub const MAX_RESPONSE_BYTES: usize = 2_097_152;
/// Queued lines per client before the connection is closed.
pub const MAX_PENDING_LINES: usize = 64;

/// Synchronous request handler. Returning `None` means "no response" — used for
/// JSON-RPC notifications.
pub type Handler = Arc<dyn Fn(&[u8]) -> Option<Vec<u8>> + Send + Sync>;

/// Loopback JSON-lines server.
pub struct AgentLocalServer {
    port: u16,
    handler: Handler,
    state: Mutex<ServerState>,
}

struct ServerState {
    listener: Option<TcpListener>,
    shutdown: Arc<AtomicBool>,
    port: Option<u16>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl AgentLocalServer {
    pub fn new(port: u16, handler: Handler) -> Self {
        Self {
            port,
            handler,
            state: Mutex::new(ServerState {
                listener: None,
                shutdown: Arc::new(AtomicBool::new(false)),
                port: None,
                threads: Vec::new(),
            }),
        }
    }

    pub fn bound_port(&self) -> Option<u16> {
        self.state.lock().unwrap_or_else(|error| error.into_inner()).port
    }

    pub fn start(&self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.listener.is_some() {
            return Ok(());
        }
        let listener = TcpListener::bind(("127.0.0.1", self.port))?;
        let bound = listener.local_addr()?.port();
        let accept_listener = listener.try_clone()?;
        state.shutdown.store(false, Ordering::SeqCst);
        let shutdown = state.shutdown.clone();
        state.port = Some(bound);
        state.listener = Some(listener);
        let handler = self.handler.clone();
        // One thread per connection keeps a slow client from blocking the
        // others, and a synchronous handler needs no executor.
        let thread = std::thread::Builder::new()
            .name("cove.agent.accept".into())
            .spawn(move || {
                for incoming in accept_listener.incoming() {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = incoming else { continue };
                    let handler = handler.clone();
                    let _ = std::thread::Builder::new()
                        .name("cove.agent.client".into())
                        .spawn(move || serve(stream, &handler));
                }
            })?;
        state.threads.push(thread);
        Ok(())
    }

    pub fn stop(&self) {
        let (port, threads) = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.shutdown.store(true, Ordering::SeqCst);
            let port = state.port;
            state.listener.take();
            state.port = None;
            (port, std::mem::take(&mut state.threads))
        };
        // Break the blocking accept.
        if let Some(port) = port {
            let _ = TcpStream::connect(("127.0.0.1", port));
        }
        for thread in threads {
            let _ = thread.join();
        }
    }
}

impl Drop for AgentLocalServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Read newline-delimited requests, answer each with one JSON line.
///
/// Bounds are enforced before the handler runs: an oversized line, too many
/// queued lines or an oversized response closes the connection rather than
/// growing without limit.
fn serve(stream: TcpStream, handler: &Handler) {
    let Ok(writer_stream) = stream.try_clone() else { return };
    let mut writer = std::io::BufWriter::new(writer_stream);
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    let mut pending = 0usize;
    loop {
        line.clear();
        match read_bounded_line(&mut reader, &mut line) {
            Ok(0) => break,
            Ok(_) => {}
            // Oversized line: the connection is no longer trustworthy.
            Err(LineError::TooLong) | Err(LineError::Io) => break,
        }
        if line.is_empty() {
            continue;
        }
        pending += 1;
        if pending > MAX_PENDING_LINES {
            break;
        }
        // Strip a trailing CR from CRLF clients.
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let Some(response) = (handler)(&line) else {
            pending -= 1;
            continue;
        };
        if response.len() > MAX_RESPONSE_BYTES {
            break;
        }
        if writer.write_all(&response).is_err() || writer.write_all(b"\n").is_err() || writer.flush().is_err() {
            break;
        }
        pending -= 1;
    }
}

enum LineError {
    Io,
    TooLong,
}

/// Read one line without exceeding `MAX_LINE_BYTES`.
fn read_bounded_line(reader: &mut BufReader<TcpStream>, out: &mut Vec<u8>) -> Result<usize, LineError> {
    let mut byte = [0u8; 1];
    loop {
        match std::io::Read::read(reader, &mut byte) {
            Ok(0) => return Ok(out.len()),
            Ok(_) => {
                if byte[0] == b'\n' {
                    return Ok(out.len());
                }
                if out.len() >= MAX_LINE_BYTES {
                    return Err(LineError::TooLong);
                }
                out.push(byte[0]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(LineError::Io),
        }
    }
}

/// Serve MCP over stdin/stdout, one JSON message per line.
///
/// This is the shape a command-line agent host needs; it shares the handler and
/// bounds with the socket server.
pub fn serve_stdio(handler: Handler) -> io::Result<()> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        if line.len() > MAX_LINE_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "request exceeds 1 MiB"));
        }
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            continue;
        }
        let Some(response) = handler(trimmed.as_bytes()) else { continue };
        if response.len() > MAX_RESPONSE_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "response exceeds 2 MiB"));
        }
        writer.write_all(&response)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn echo() -> Handler {
        Arc::new(|input: &[u8]| Some(input.to_vec()))
    }

    #[test]
    fn loopback_server_round_trips_one_line() {
        let server = AgentLocalServer::new(0, echo());
        server.start().expect("starts");
        let port = server.bound_port().expect("port");
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.write_all(b"{\"a\":1}\n").expect("write");
        stream.flush().expect("flush");
        // The transport is a *persistent* line protocol: `serve` writes one
        // response and keeps reading for the next line. `read_to_end` only
        // returns at EOF, so the client must close its writing half first —
        // otherwise the client waits for EOF while the server waits for more
        // input, and the test deadlocks.
        stream.shutdown(std::net::Shutdown::Write).expect("half-close");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read");
        let text = String::from_utf8_lossy(&response);
        assert_eq!(text.lines().next(), Some("{\"a\":1}"));
        server.stop();
        assert!(server.bound_port().is_none());
    }

    #[test]
    fn notifications_produce_no_bytes() {
        let server = AgentLocalServer::new(0, Arc::new(|_input: &[u8]| None));
        server.start().expect("starts");
        let port = server.bound_port().expect("port");
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.write_all(b"{\"method\":\"ping\"}\n").expect("write");
        stream.flush().expect("flush");
        // Closing from our side lets the reader observe EOF; the server wrote
        // nothing because the handler returned `None`.
        drop(stream);
        server.stop();
    }
}
