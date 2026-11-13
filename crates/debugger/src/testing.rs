//! An in-process DAP server used by tests and adapter-less verification.
//!
//! It speaks real `Content-Length` frames over OS pipes, so the same client
//! code used against `lldb-dap` is exercised here. It never pretends to be a
//! debugger for production: the controller only builds it from test code.
use crate::error::Result;
use crate::framing::{read_message, write_message};
use crate::model::SessionState;
use crate::session::DapSession;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufReader, PipeReader, PipeWriter};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Changes the mock's fixed answers.
#[derive(Clone, Debug, Default)]
pub struct MockOptions {
    /// Refuse `attach` with this adapter message.
    pub fail_attach: Option<String>,
    /// Refuse `evaluate` with this adapter message.
    pub fail_evaluate: Option<String>,
    /// Requests the mock accepts but never answers, for timeout tests.
    pub ignore: BTreeSet<String>,
}

impl MockOptions {
    pub fn fail_attach(mut self, reason: impl Into<String>) -> Self {
        self.fail_attach = Some(reason.into());
        self
    }

    pub fn fail_evaluate(mut self, reason: impl Into<String>) -> Self {
        self.fail_evaluate = Some(reason.into());
        self
    }

    pub fn ignore(mut self, command: impl Into<String>) -> Self {
        self.ignore.insert(command.into());
        self
    }
}

struct Writer(Arc<Mutex<PipeWriter>>);

impl Writer {
    fn send(&self, message: &Value) -> bool {
        let mut writer = self.0.lock().unwrap();
        write_message(&mut *writer, message).is_ok()
    }
}

/// A handle to the mock's recorded traffic and event stream.
pub struct MockHandle {
    requests: Arc<Mutex<Vec<Value>>>,
    writer: Writer,
    thread: Option<JoinHandle<()>>,
}

impl MockHandle {
    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }

    pub fn requests_named(&self, command: &str) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request["command"] == command)
            .cloned()
            .collect()
    }

    /// Sends an adapter event to the client.
    pub fn emit(&self, event: &str, body: Value) {
        let sequence = self.requests.lock().unwrap().len() as u64 + 1;
        let _ = self.writer.send(&json!({
            "seq": sequence,
            "type": "event",
            "event": event,
            "body": body,
        }));
    }

    /// Waits until the server loop has ended, with a bound.
    pub fn wait(mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while let Some(handle) = self.thread.as_ref() {
            if handle.is_finished() || Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(handle) = self.thread.take()
            && handle.is_finished()
        {
            let _ = handle.join();
        }
    }
}

pub struct MockAdapter;

impl MockAdapter {
    pub fn pair() -> (DapSession, MockHandle) {
        Self::with(MockOptions::default())
    }

    pub fn with(options: MockOptions) -> (DapSession, MockHandle) {
        let (client_reader, server_writer) = std::io::pipe().expect("pipe");
        let (server_reader, client_writer) = std::io::pipe().expect("pipe");
        let session = DapSession::from_io(Box::new(client_reader), Box::new(client_writer))
            .expect("mock session");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let writer = Writer(Arc::new(Mutex::new(server_writer)));
        let thread_writer = Writer(Arc::clone(&writer.0));
        let thread_requests = Arc::clone(&requests);
        let handle = thread::Builder::new()
            .name("cove.debugger.mock".into())
            .spawn(move || serve(server_reader, thread_writer, thread_requests, options))
            .expect("mock server thread");
        (
            session,
            MockHandle {
                requests,
                writer,
                thread: Some(handle),
            },
        )
    }
}

struct MockState {
    breakpoints: BTreeMap<(String, u32), u64>,
    next_breakpoint: u64,
}

fn serve(
    reader: PipeReader,
    writer: Writer,
    requests: Arc<Mutex<Vec<Value>>>,
    options: MockOptions,
) {
    let mut reader = BufReader::new(reader);
    let mut state = MockState {
        breakpoints: BTreeMap::new(),
        next_breakpoint: 1,
    };
    while let Ok(Some(message)) = read_message(&mut reader) {
        let command = message
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let arguments = message.get("arguments").cloned().unwrap_or(Value::Null);
        let sequence = message.get("seq").and_then(Value::as_u64).unwrap_or(0);
        requests.lock().unwrap().push(message);
        if options.ignore.contains(&command) {
            continue;
        }
        let mut events = Vec::new();
        let response = match command.as_str() {
            "initialize" => success(json!({
                "$__lldb_version": "mock-lldb 21.0.0",
                "supportsConfigurationDoneRequest": true,
                "supportsConditionalBreakpoints": true,
                "supportsFunctionBreakpoints": false,
            })),
            "attach" => match &options.fail_attach {
                Some(reason) => failure("attach", reason.clone()),
                None => {
                    events.push((
                        "initialized",
                        json!({"supportsConfigurationDoneRequest": true}),
                    ));
                    success(json!({"processId": arguments["pid"]}))
                }
            },
            "configurationDone" => {
                events.push((
                    "stopped",
                    json!({
                        "reason": "attach",
                        "threadId": 1,
                        "allThreadsStopped": true,
                        "description": "attached",
                    }),
                ));
                success(json!({}))
            }
            "threads" => success(json!({"threads": [
                {"id": 1, "name": "main"},
                {"id": 2, "name": "worker"},
            ]})),
            "stackTrace" => success(json!({
                "totalFrames": 2,
                "stackFrames": [
                    {
                        "id": 11,
                        "name": "cove::Mock::run",
                        "source": {"path": "/tmp/mock/src/lib.rs", "name": "lib.rs"},
                        "line": 42,
                        "column": 9,
                    },
                    {
                        "id": 12,
                        "name": "cove::main",
                        "source": {"path": "/tmp/mock/src/main.rs", "name": "main.rs"},
                        "line": 7,
                        "column": 1,
                    },
                ],
            })),
            "scopes" => success(json!({"scopes": [
                {"name": "Locals", "variablesReference": 100, "expensive": false},
                {"name": "Globals", "variablesReference": 200, "expensive": true},
            ]})),
            "variables" => {
                let reference = arguments["variablesReference"].as_u64().unwrap_or(0);
                let variables = match reference {
                    100 => json!([{
                        "name": "x", "value": "42", "type": "Int",
                        "variablesReference": 0, "namedVariables": 0, "indexedVariables": 0,
                    }]),
                    200 => json!([{
                        "name": "g", "value": "true", "type": "Bool",
                        "variablesReference": 0, "namedVariables": 0, "indexedVariables": 0,
                    }]),
                    _ => json!([]),
                };
                success(json!({"variables": variables}))
            }
            "evaluate" => match &options.fail_evaluate {
                Some(reason) => failure("evaluate", reason.clone()),
                None => success(json!({"result": "42", "type": "Int", "variablesReference": 0})),
            },
            "setBreakpoints" => {
                let path = arguments["source"]["path"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                let requested = arguments["breakpoints"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                for entry in &requested {
                    let line = entry["line"].as_u64().unwrap_or(0) as u32;
                    if !state.breakpoints.contains_key(&(path.clone(), line)) {
                        let id = state.next_breakpoint;
                        state.next_breakpoint += 1;
                        state.breakpoints.insert((path.clone(), line), id);
                    }
                }
                let breakpoints: Vec<_> = requested
                    .iter()
                    .map(|entry| {
                        let line = entry["line"].as_u64().unwrap_or(0) as u32;
                        let id = state.breakpoints[&(path.clone(), line)];
                        json!({"id": id, "verified": true, "line": line})
                    })
                    .collect();
                success(json!({"breakpoints": breakpoints}))
            }
            "continue" => {
                events.push(("continued", json!({"allThreadsContinued": true})));
                success(json!({"allThreadsContinued": true}))
            }
            "pause" => {
                events.push((
                    "stopped",
                    json!({"reason": "pause", "threadId": 1, "allThreadsStopped": true}),
                ));
                success(json!({}))
            }
            "next" | "stepIn" | "stepOut" => {
                events.push((
                    "stopped",
                    json!({"reason": "step", "threadId": 1, "allThreadsStopped": true}),
                ));
                success(json!({}))
            }
            "disconnect" => success(json!({})),
            _ => success(json!({})),
        };
        let response_sequence = requests.lock().unwrap().len() as u64;
        if !writer.send(&json!({
            "seq": response_sequence,
            "type": "response",
            "request_seq": sequence,
            "success": response.0,
            "command": command,
            "message": response.1.clone(),
            "body": response.2,
        })) {
            break;
        }
        for (event, body) in events {
            if !writer.send(&json!({
                "seq": response_sequence,
                "type": "event",
                "event": event,
                "body": body,
            })) {
                return;
            }
        }
        if command == "disconnect" {
            break;
        }
    }
}

/// `(success, message, body)` for a response.
fn success(body: Value) -> (bool, Option<String>, Value) {
    (true, None, body)
}

fn failure(command: &str, reason: String) -> (bool, Option<String>, Value) {
    (
        false,
        Some(reason.clone()),
        json!({"error": {"id": 3, "format": reason, "showUser": true, "command": command}}),
    )
}

/// Waits until a mock session reports `state`, bounded by `timeout`.
#[doc(hidden)]
pub fn wait_for_state(session: &DapSession, state: SessionState, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let current = session.snapshot().state;
        if current == state {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(crate::error::DebuggerError::timeout(
                format!("session state {state:?}"),
                timeout,
            ));
        }
        thread::sleep(Duration::from_millis(2));
    }
}
