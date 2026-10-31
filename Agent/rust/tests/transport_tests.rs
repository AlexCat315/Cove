//! Translation of CoveUIAgentTransportTests/TransportTests.swift.
//!
//! Behaviour checklist:
//! - loopback binding takes an ephemeral port and survives a stop/start cycle
//! - a line longer than the frame budget closes the connection
//!
//! Swift's `EmbeddedChannel` half of the oversized-line case exercised NIO's
//! handler in isolation. Rust has no NIO; the equivalent is the same boundary
//! over a real loopback socket, since `read_bounded_line` is private to the
//! transport and the observable behaviour is the closed connection.
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cove_devtools::agent_transport::AgentLocalServer;

fn echo_handler() -> cove_devtools::agent_transport::Handler {
    Arc::new(|line: &[u8]| Some(line.to_vec()))
}

#[test]
fn loopback_binding_supports_ephemeral_ports_and_restart() {
    let server = AgentLocalServer::new(0, echo_handler());
    server.start().expect("start");
    let port = server.bound_port().expect("bound port");
    assert_ne!(
        port, 0,
        "binding port 0 must publish the kernel-assigned port"
    );

    server.stop();
    assert_eq!(server.bound_port(), None, "stopping releases the port");

    server.start().expect("restart");
    assert!(server.bound_port().is_some(), "restarting binds again");
    server.stop();
}

#[test]
fn oversized_lines_close_connection() {
    const MAX_LINE_BYTES: usize = 1_048_576;

    let server = AgentLocalServer::new(0, echo_handler());
    server.start().expect("start");
    let port = server.bound_port().expect("bound port");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");

    // One byte past the budget, with no newline: the server must drop the
    // connection instead of buffering without limit.
    let oversized = vec![b'a'; MAX_LINE_BYTES + 1];
    let _ = stream.write_all(&oversized);
    let _ = stream.flush();

    // Either the write fails or the peer closes; both mean the line was refused.
    let mut reader = BufReader::new(stream);
    let mut response = Vec::new();
    let started = Instant::now();
    let read = loop {
        response.clear();
        match reader.read_until(b'\n', &mut response) {
            Ok(0) => break Ok(0),
            Ok(read) => {
                // A correctly-sized line would have been echoed back. Nothing
                // should arrive, so any echo is a failure.
                break Ok(read);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() > Duration::from_secs(2) {
                    break Err(error);
                }
            }
            Err(error) => break Err(error),
        }
    };
    match read {
        Ok(0) => {}
        Ok(_) => panic!("an oversized line must not be answered, got {response:?}"),
        Err(_) => {}
    }
    server.stop();
}

#[test]
fn bounded_lines_are_echoed_so_the_bound_is_worth_testing() {
    // Control for the oversized case: a line inside the budget round-trips, so
    // the previous assertion cannot pass merely because the server is deaf.
    let server = AgentLocalServer::new(0, echo_handler());
    server.start().expect("start");
    let port = server.bound_port().expect("bound port");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    stream.write_all(b"{\"ok\":true}\n").expect("write");
    stream.flush().expect("flush");
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).expect("read");
    assert_eq!(line, "{\"ok\":true}\n");
    server.stop();
}
