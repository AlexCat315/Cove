//! The runtime's network recorder reaches the DevTools read wire.
use cove_core::scene::NodeTree;
use cove_devtools::inspection::{Diagnostics, Tool};
use serde_json::json;
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::time::Duration;

fn serve(response: Vec<u8>) -> String {
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
        let _ = stream.write_all(&response);
        let _ = stream.flush();
    });
    base
}

#[test]
fn network_records_reach_the_devtools_wire() {
    cove_http::recording::set_enabled(true);
    cove_http::recording::set_capture_bodies(true);
    let base = serve(
        b"HTTP/1.1 201 Created\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\npong"
            .to_vec(),
    );
    let client = cove_http::Client::new(cove_http::ClientConfig {
        timeout: Duration::from_secs(5),
        ..cove_http::ClientConfig::default()
    });
    let response = client
        .send(
            cove_http::Request::get(format!("{base}/devtools-network"))
                .header("Authorization", "Bearer secret"),
        )
        .unwrap();
    assert_eq!(response.bytes(1024).unwrap(), b"pong");

    let diagnostics = Diagnostics::new(Rc::new(RefCell::new(NodeTree::default())), None);
    let result = diagnostics
        .execute_read_json(Tool::QueryUiNetwork, &json!({}))
        .unwrap();
    let records = result["data"]["records"].as_array().unwrap();
    let record = records
        .iter()
        .find(|record| {
            record["url"]
                .as_str()
                .is_some_and(|url| url.ends_with("/devtools-network"))
        })
        .unwrap_or_else(|| panic!("network record missing: {result}"));
    assert_eq!(record["status"], 201);
    assert_eq!(record["state"], "complete");
    assert_eq!(record["responseBody"], "pong");
    assert_eq!(record["responseBytes"], 4);
    let authorization = record["requestHeaders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|header| {
            header[0]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("authorization"))
        })
        .map(|header| header[1].as_str().unwrap());
    assert_eq!(authorization, Some("<redacted>"));
}
