//! Runtime network recording against a loopback HTTP/1.1 server.
use cove_http::recording::{self, State};
use cove_http::{Client, ClientConfig, Request};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

fn serve(response: Vec<u8>) -> (String, mpsc::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        read_request(&stream);
        let _ = sender.send(());
        let mut stream = stream;
        let _ = stream.write_all(&response);
        let _ = stream.flush();
    });
    (base, receiver)
}

fn read_request(stream: &TcpStream) {
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
}

fn client() -> Client {
    Client::new(ClientConfig {
        timeout: Duration::from_secs(5),
        ..ClientConfig::default()
    })
}

fn find(path: &str) -> recording::NetworkRecord {
    recording::snapshot()
        .into_iter()
        .find(|record| record.url.ends_with(path))
        .unwrap_or_else(|| panic!("no record for {path}"))
}

#[test]
fn records_request_response_and_body() {
    recording::set_enabled(true);
    recording::set_capture_bodies(true);
    let (base, _requests) = serve(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\npong"
            .to_vec(),
    );
    let response = client()
        .send(Request::post(
            format!("{base}/recording-meta"),
            b"ping".to_vec(),
        ))
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.bytes(1024).unwrap(), b"pong");

    let record = find("/recording-meta");
    assert_eq!(record.method, "POST");
    assert_eq!(record.status, Some(200));
    assert_eq!(record.state, State::Complete);
    assert_eq!(record.request_bytes, 4);
    assert_eq!(record.request_body, b"ping");
    assert_eq!(record.response_bytes, 4);
    assert_eq!(record.response_body, b"pong");
    assert!(record.ttfb_ms.is_some());
    assert!(record.duration_ms >= 0.0);
}

#[test]
fn redacts_credentials_and_records_timing() {
    recording::set_enabled(true);
    let (base, _requests) = serve(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_vec());
    let request = Request::get(format!("{base}/recording-redact"))
        .header("Authorization", "Bearer top-secret")
        .header("Cookie", "session=secret");
    let response = client().send(request).unwrap();
    assert_eq!(response.status, 204);
    let _ = response.bytes(1024).unwrap();

    let record = find("/recording-redact");
    let authorization = record
        .request_headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.as_str());
    let cookie = record
        .request_headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.as_str());
    assert_eq!(authorization, Some("<redacted>"));
    assert_eq!(cookie, Some("<redacted>"));
}

#[test]
fn truncates_large_bodies_but_counts_all_bytes() {
    recording::set_enabled(true);
    recording::set_capture_bodies(true);
    let payload = vec![b'x'; recording::MAX_BODY_BYTES + 4096];
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    let mut response_bytes = head.into_bytes();
    response_bytes.extend_from_slice(&payload);
    let (base, _requests) = serve(response_bytes);

    let response = client()
        .send(Request::get(format!("{base}/recording-large")))
        .unwrap();
    assert_eq!(
        response.bytes(recording::MAX_BODY_BYTES * 2).unwrap().len(),
        payload.len()
    );

    let record = find("/recording-large");
    assert_eq!(record.response_bytes, payload.len() as u64);
    assert_eq!(record.response_body.len(), recording::MAX_BODY_BYTES);
    assert!(record.response_body_truncated);
    assert_eq!(record.state, State::Complete);
}
