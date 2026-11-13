//! Network recording controls drive the real process-wide HTTP recorder.
//!
//! The recorder sits on `cove_http::Client::send`, so these tests use real
//! loopback requests instead of inspecting a client table: pausing must stop a
//! request from being recorded while keeping earlier records, resuming must
//! record the next request, and clearing must empty the retained log without
//! changing the enabled state. The recorder is process-global, so every test
//! holds one mutex and restores recording afterwards.

use cove_core::scene::{Node, NodeTree};
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use cove_http::recording::{self, MAX_RECORDS};
use cove_http::{Client, ClientConfig, Request};
use serde_json::json;
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

static RECORDER: Mutex<()> = Mutex::new(());

fn exclusive() -> MutexGuard<'static, ()> {
    RECORDER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Leaves the process recording when a test finishes or panics.
struct Recording;
impl Drop for Recording {
    fn drop(&mut self) {
        recording::set_enabled(true);
    }
}

fn probe() -> RuntimeProbe {
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(Node::new()));
    RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None)
}

fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let mut stream = stream;
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        );
        let _ = stream.flush();
    });
    base
}

/// One real request through `Client::send`, the call the recorder observes.
fn request(path: &str) {
    let base = serve();
    let response = Client::new(ClientConfig {
        timeout: Duration::from_secs(5),
        ..ClientConfig::default()
    })
    .send(Request::get(format!("{base}{path}")))
    .unwrap();
    assert_eq!(response.bytes(64).unwrap(), b"ok");
}

fn retained(path: &str) -> usize {
    recording::snapshot()
        .iter()
        .filter(|record| record.url.ends_with(path))
        .count()
}

#[test]
fn the_probe_advertises_network_control() {
    assert!(
        probe()
            .capabilities()
            .contains(&"network_control".to_owned())
    );
}

#[test]
fn pausing_stops_new_records_and_resuming_records_them_again() {
    let _serial = exclusive();
    let _recording = Recording;
    let mut probe = probe();
    recording::clear();
    recording::set_enabled(true);

    assert_eq!(
        probe
            .execute(ProbeCommand::NetworkRecording { enabled: true })
            .unwrap(),
        json!({"enabled": true}),
        "the reply carries the recorder's real state"
    );
    request("/before-pause");
    assert_eq!(retained("/before-pause"), 1);
    let kept = recording::snapshot().len();

    assert_eq!(
        probe
            .execute(ProbeCommand::NetworkRecording { enabled: false })
            .unwrap(),
        json!({"enabled": false})
    );
    assert!(!recording::is_enabled(), "pause reaches the real recorder");
    request("/while-paused");
    assert_eq!(
        retained("/while-paused"),
        0,
        "a paused recorder keeps no new request"
    );
    assert_eq!(
        recording::snapshot().len(),
        kept,
        "pausing preserves the records taken before the pause"
    );
    assert_eq!(retained("/before-pause"), 1);
    let read = probe
        .execute(ProbeCommand::Read {
            name: "query_ui_network".into(),
            arguments: json!({}),
        })
        .unwrap();
    assert_eq!(
        read["data"]["enabled"],
        json!(false),
        "the read wire reports the live paused state"
    );

    assert_eq!(
        probe
            .execute(ProbeCommand::NetworkRecording { enabled: true })
            .unwrap(),
        json!({"enabled": true})
    );
    assert!(recording::is_enabled());
    request("/after-resume");
    assert_eq!(retained("/after-resume"), 1, "resume records again");
    assert_eq!(
        retained("/before-pause"),
        1,
        "resume does not discard the retained history"
    );
}

#[test]
fn clearing_removes_retained_records_and_reports_the_bounded_count() {
    let _serial = exclusive();
    let _recording = Recording;
    let mut probe = probe();
    recording::set_enabled(true);
    recording::clear();

    // `begin` is the recorder's own entry point, so 300 requests exercise the
    // real 256-record bound without opening 300 sockets.
    for index in 0..MAX_RECORDS + 44 {
        assert!(
            recording::begin(&Request::get(format!("http://127.0.0.1/synthetic-{index}")))
                .is_some(),
            "the recorder accepts requests while enabled"
        );
    }
    assert_eq!(
        recording::snapshot().len(),
        MAX_RECORDS,
        "the log evicts the oldest records at the bound"
    );

    assert_eq!(
        probe.execute(ProbeCommand::ClearNetwork).unwrap(),
        json!({"cleared": MAX_RECORDS, "limit": MAX_RECORDS}),
        "the clear reports exactly the records it removed"
    );
    assert!(recording::snapshot().is_empty(), "no record survives");
    assert_eq!(
        probe.execute(ProbeCommand::ClearNetwork).unwrap()["cleared"],
        json!(0),
        "an empty log clears nothing"
    );

    request("/after-clear");
    assert_eq!(
        retained("/after-clear"),
        1,
        "clearing does not pause the recorder"
    );
    assert_eq!(recording::snapshot().len(), 1);
}

#[test]
fn disconnect_restores_the_build_default_recording_state() {
    let _serial = exclusive();
    let _recording = Recording;
    let mut probe = probe();
    recording::set_enabled(true);
    probe
        .execute(ProbeCommand::NetworkRecording { enabled: false })
        .unwrap();
    assert!(!recording::is_enabled());

    probe.disconnect();

    assert_eq!(
        recording::is_enabled(),
        cfg!(debug_assertions),
        "a closed DevTools session must not leave the process silently unrecorded"
    );
}

#[test]
fn network_control_commands_dispatch_from_their_wire_form() {
    let _serial = exclusive();
    let _recording = Recording;
    let mut probe = probe();
    recording::set_enabled(true);

    let wire = serde_json::to_value(ProbeCommand::NetworkRecording { enabled: false }).unwrap();
    assert_eq!(wire["method"], json!("network_recording"));
    let command: ProbeCommand = serde_json::from_value(wire).unwrap();
    assert_eq!(probe.execute(command).unwrap(), json!({"enabled": false}));

    let wire = serde_json::to_value(ProbeCommand::ClearNetwork).unwrap();
    assert_eq!(wire, json!({"method": "clear_network"}));
    let command: ProbeCommand = serde_json::from_value(wire).unwrap();
    recording::clear();
    assert_eq!(probe.execute(command).unwrap()["cleared"], json!(0));
}
