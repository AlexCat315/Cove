//! Client behaviour against a loopback HTTP/1.1 server.
//!
//! Run with `--features openssl` on macOS to exercise the socket + OpenSSL
//! backend that Linux and Android use. `COVE_HTTP_NETWORK_TESTS=1` adds a
//! real HTTPS request.
use cove_http::{Client, ClientConfig, HttpError, Request};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

struct Received {
    head: String,
    body: Vec<u8>,
}

/// Serves `responses` in order, one connection each, and reports each request.
fn serve(responses: Vec<Vec<u8>>) -> (String, mpsc::Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for response in responses {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let received = read_request(&stream);
            let _ = sender.send(received);
            let mut stream = stream;
            let _ = stream.write_all(&response);
        }
    });
    (base, receiver)
}

fn read_request(stream: &TcpStream) -> Received {
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
    Received { head, body }
}

fn client() -> Client {
    Client::new(ClientConfig {
        timeout: Duration::from_secs(5),
        ..ClientConfig::default()
    })
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[test]
fn posts_a_body_and_reads_headers() {
    let (base, requests) = serve(vec![
        b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nX-Trace: a\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}".to_vec(),
    ]);
    let response = client()
        .send(
            Request::post(format!("{base}/v1/items?x=1"), br#"{"a":1}"#.to_vec())
                .header("Content-Type", "application/json")
                .header("Authorization", "Bearer k"),
        )
        .unwrap();
    assert_eq!(response.status, 201);
    assert!(response.is_success());
    assert_eq!(response.header("x-trace"), Some("a"));
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(response.bytes(1024).unwrap(), br#"{"ok":true}"#);
    let received = requests.recv().unwrap();
    assert!(
        received.head.starts_with("POST /v1/items?x=1 HTTP/1.1\r\n"),
        "{}",
        received.head
    );
    assert_eq!(header(&received.head, "authorization"), Some("Bearer k"));
    assert_eq!(received.body, br#"{"a":1}"#);
}

#[test]
fn streams_a_chunked_body() {
    let (base, _requests) = serve(vec![
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n6\r\ndata: \r\n5\r\nhello\r\n0\r\n\r\n".to_vec(),
    ]);
    let mut response = client()
        .send(Request::get(format!("{base}/stream")))
        .unwrap();
    let mut lines = BufReader::new(response.body());
    let mut text = String::new();
    lines.read_to_string(&mut text).unwrap();
    assert_eq!(text, "data: hello");
}

#[test]
fn reports_error_statuses_as_responses() {
    let (base, _requests) = serve(vec![
        b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 3\r\nContent-Length: 4\r\nConnection: close\r\n\r\nslow".to_vec(),
    ]);
    let response = client().send(Request::get(format!("{base}/"))).unwrap();
    assert_eq!(response.status, 429);
    assert_eq!(response.header("retry-after"), Some("3"));
    assert_eq!(response.bytes(16).unwrap(), b"slow");
}

#[test]
fn follows_redirects_and_rewrites_post_after_303() {
    let (base, requests) = serve(vec![
        b"HTTP/1.1 303 See Other\r\nLocation: /done\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec(),
    ]);
    let response = client()
        .send(Request::post(format!("{base}/start"), b"payload".to_vec()))
        .unwrap();
    assert_eq!(response.status, 200);
    assert!(response.url.ends_with("/done"), "{}", response.url);
    assert_eq!(response.bytes(16).unwrap(), b"ok");
    let first = requests.recv().unwrap();
    assert!(first.head.starts_with("POST /start"));
    let second = requests.recv().unwrap();
    assert!(second.head.starts_with("GET /done"), "{}", second.head);
    assert!(second.body.is_empty());
}

#[test]
fn stops_after_the_redirect_budget() {
    let redirect =
        b"HTTP/1.1 302 Found\r\nLocation: /again\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec();
    let (base, _requests) = serve(vec![redirect.clone(), redirect.clone(), redirect]);
    let client = Client::new(ClientConfig {
        timeout: Duration::from_secs(5),
        max_redirects: 2,
        ..ClientConfig::default()
    });
    assert_eq!(
        client.send(Request::get(format!("{base}/"))).unwrap_err(),
        HttpError::TooManyRedirects
    );
}

#[test]
fn returns_the_redirect_when_following_is_off() {
    let (base, _requests) = serve(vec![
        b"HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    ]);
    let client = Client::new(ClientConfig {
        timeout: Duration::from_secs(5),
        max_redirects: 0,
        ..ClientConfig::default()
    });
    let response = client.send(Request::get(format!("{base}/"))).unwrap();
    assert_eq!(response.status, 302);
    assert_eq!(response.header("location"), Some("/elsewhere"));
}

#[test]
fn enforces_body_limits() {
    let (base, _requests) = serve(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789".to_vec(),
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n0123456789".to_vec(),
    ]);
    let response = client().send(Request::get(format!("{base}/a"))).unwrap();
    assert_eq!(response.bytes(4).unwrap_err(), HttpError::BodyTooLarge(4));
    let response = client().send(Request::get(format!("{base}/b"))).unwrap();
    assert_eq!(response.bytes(4).unwrap_err(), HttpError::BodyTooLarge(4));
}

#[test]
fn times_out_on_a_silent_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hold = std::thread::spawn(move || {
        let connection = listener.accept();
        std::thread::sleep(Duration::from_secs(3));
        drop(connection);
    });
    let client = Client::new(ClientConfig {
        timeout: Duration::from_millis(500),
        ..ClientConfig::default()
    });
    let error = client
        .send(Request::get(format!("http://127.0.0.1:{port}/")))
        .unwrap_err();
    assert!(
        matches!(error, HttpError::Timeout | HttpError::Network(_)),
        "{error:?}"
    );
    hold.join().unwrap();
}

#[test]
fn rejects_header_injection_and_bad_urls() {
    let client = client();
    assert!(matches!(
        client.send(Request::get("http://127.0.0.1:1/").header("X", "a\r\nInjected: 1")),
        Err(HttpError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.send(Request::new("GE T", "http://127.0.0.1:1/")),
        Err(HttpError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.send(Request::get("ftp://example.com/")),
        Err(HttpError::UnsupportedUrl(_))
    ));
}

#[test]
fn tls_to_a_plain_server_fails_cleanly() {
    let (base, _requests) = serve(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
    ]);
    let url = base.replace("http://", "https://");
    let capabilities = Client::capabilities();
    let error = client().send(Request::get(url)).unwrap_err();
    if capabilities.https {
        assert!(
            matches!(error, HttpError::Tls(_) | HttpError::Network(_)),
            "{error:?}"
        );
    }
}

#[test]
fn reaches_a_real_https_server() {
    if std::env::var_os("COVE_HTTP_NETWORK_TESTS").is_none() {
        return;
    }
    let capabilities = Client::capabilities();
    assert!(capabilities.https, "{:?}", capabilities.limitation);
    let response = client().send(Request::get("https://example.com/")).unwrap();
    assert_eq!(response.status, 200);
    let body = response.bytes(1 << 20).unwrap();
    assert!(String::from_utf8_lossy(&body).contains("Example Domain"));
    let error = client()
        .send(Request::get("https://expired.badssl.com/"))
        .unwrap_err();
    assert!(
        matches!(error, HttpError::Tls(_) | HttpError::Network(_)),
        "{error:?}"
    );
}
