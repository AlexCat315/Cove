#![cfg(all(feature = "openai-compatible", any(unix, windows)))]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use cove_agent::openai_compatible::ChatCompletionsModel;
use cove_agent::{
    AgentMessage, AgentSession, AgentSkillLibrary, AgentTool, AgentToolResult, AgentToolbox,
    JsonSchema,
};
use cove_http::recording;
use serde_json::{Value, json};

#[test]
fn tool_round_trips_with_the_provider_without_recording_private_data() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let (send, receive) = mpsc::channel();
    let server = std::thread::spawn(move || {
        for result in [
            json!({"choices": [{"finish_reason": "tool_calls", "message": {"reasoning_content": "Provider continuation", "tool_calls": [
                {"id": "call_1", "type": "function", "function": {"name": "read_note", "arguments": "{}"}}
            ]}}]}),
            json!({"choices": [{"finish_reason": "stop", "message": {"content": "A brief note."}}]}),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /v1/chat/completions "));
            let mut length = 0;
            let mut authorization = None;
            loop {
                line.clear();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((header, value)) = line.split_once(':') {
                    if header.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                    if header.eq_ignore_ascii_case("authorization") {
                        authorization = Some(value.trim().to_owned());
                    }
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            send.send((
                serde_json::from_slice::<Value>(&body).unwrap(),
                authorization,
            ))
            .unwrap();
            let response = result.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        }
    });

    recording::set_enabled(true);
    recording::set_capture_bodies(true);
    let adapter = ChatCompletionsModel::new(&base, "mock-model", Some("example-key")).unwrap();
    let tool = AgentTool::new(
        "read_note",
        "Read a note",
        JsonSchema::object(Default::default(), Vec::new()),
        true,
    );
    let toolbox = AgentToolbox::new(
        vec![tool],
        Arc::new(|_, _| {
            Ok(AgentToolResult::encode(
                &json!({"note": "Private text."}),
                false,
            ))
        }),
    )
    .unwrap();
    let session = AgentSession::new(
        toolbox,
        &AgentSkillLibrary::default(),
        &[],
        3,
        adapter.into_agent_model(),
    )
    .unwrap();
    let result = session
        .run(vec![AgentMessage::user("Summarize the note.")])
        .unwrap();

    assert_eq!(result.answer, "A brief note.");
    assert_eq!(result.tool_rounds, 1);
    let (first, key) = receive.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(key.as_deref(), Some("Bearer example-key"));
    assert_eq!(first["model"], "mock-model");
    assert_eq!(first["tools"][0]["function"]["name"], "read_note");
    let (second, _) = receive.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        second["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{}"
    );
    assert_eq!(
        second["messages"][1]["reasoning_content"],
        "Provider continuation"
    );
    assert_eq!(second["messages"][2]["tool_call_id"], "call_1");
    assert!(
        second["messages"][2]["content"]
            .as_str()
            .unwrap()
            .contains("Private text.")
    );
    assert!(
        recording::snapshot()
            .iter()
            .all(|record| !record.url.starts_with(&base))
    );
    server.join().unwrap();
}

#[test]
fn model_configuration_rejects_malformed_or_insecure_endpoints() {
    for url in [
        "http://example.com/v1",
        "https://user:secret@localhost/v1",
        "http://localhost/v1?key=secret",
    ] {
        assert!(ChatCompletionsModel::new(url, "my-model", None).is_err());
    }
    assert!(ChatCompletionsModel::new("http://[::1]:1234/v1", "my-model", None).is_ok());
    assert!(ChatCompletionsModel::new("https://example.org/v1", "my-model", None).is_ok());
    assert!(ChatCompletionsModel::new("https://example.org/v1", "", None).is_err());
}
