//! One Debug Adapter Protocol session over a child `lldb-dap`.
use crate::adapter::Adapter;
use crate::error::{DebuggerError, Result};
use crate::framing::{read_message, write_message};
use crate::model::{
    Breakpoint, EvaluateResult, InitializeInfo, Scope, SessionSnapshot, SessionState, SourceInfo,
    StackFrame, StackTrace, StopInfo, ThreadInfo, Variable, VariablePage,
};
use crate::process::{AdapterProcess, join_bounded, read_stderr_tail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
const ATTACH_TIMEOUT: Duration = Duration::from_secs(10);
const CONFIGURATION_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const DETACH_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_WAIT: Duration = Duration::from_secs(2);
const CONTROL_WAIT: Duration = Duration::from_secs(3);
const JOIN_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_EVENTS: usize = 256;
const MAX_VALUE_CHARS: usize = 2048;
const MAX_VERSION_CHARS: usize = 2048;

fn invalid(message: impl Into<String>) -> DebuggerError {
    DebuggerError::new("invalid_request", message)
}

fn state_name(state: SessionState) -> &'static str {
    match state {
        SessionState::Starting => "starting",
        SessionState::Attached => "attached",
        SessionState::Running => "running",
        SessionState::Stopped => "stopped",
        SessionState::Exited => "exited",
        SessionState::Terminated => "terminated",
        SessionState::Closed => "closed",
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn truncate(text: &str) -> (String, bool) {
    if text.chars().count() <= MAX_VALUE_CHARS {
        return (text.to_owned(), false);
    }
    (text.chars().take(MAX_VALUE_CHARS).collect(), true)
}

fn source(value: Option<&Value>) -> Option<SourceInfo> {
    let value = value?;
    if !value.is_object() {
        return None;
    }
    let source = SourceInfo {
        path: text(value, "path"),
        name: text(value, "name"),
        source_reference: value.get("sourceReference").and_then(Value::as_i64),
    };
    (source.path.is_some() || source.name.is_some() || source.source_reference.is_some())
        .then_some(source)
}

fn response_reason(response: &Value) -> String {
    response
        .pointer("/body/error/format")
        .and_then(Value::as_str)
        .or_else(|| {
            response
                .pointer("/body/error/message")
                .and_then(Value::as_str)
        })
        .or_else(|| response.get("message").and_then(Value::as_str))
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("the adapter gave no reason")
        .to_owned()
}

fn stop_info(body: &Value) -> StopInfo {
    StopInfo {
        reason: text(body, "reason").unwrap_or_else(|| "unknown".to_owned()),
        description: text(body, "description"),
        thread_id: number(body, "threadId"),
        all_threads_stopped: body
            .get("allThreadsStopped")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        text: text(body, "text"),
        hit_breakpoint_ids: body
            .get("hitBreakpointIds")
            .and_then(Value::as_array)
            .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default(),
    }
}

#[derive(Clone, Debug)]
struct Observed {
    state: SessionState,
    stop: Option<StopInfo>,
    process_id: Option<u32>,
    process_name: Option<String>,
    exit_code: Option<i64>,
    generation: u64,
}

impl Default for Observed {
    fn default() -> Self {
        Self {
            state: SessionState::Starting,
            stop: None,
            process_id: None,
            process_name: None,
            exit_code: None,
            generation: 0,
        }
    }
}

struct EventEntry {
    index: u64,
    value: Value,
}

#[derive(Default)]
struct SharedState {
    pending: HashMap<u64, mpsc::Sender<Result<Value>>>,
    events: VecDeque<EventEntry>,
    event_index: u64,
    events_dropped: u64,
    closed: Option<DebuggerError>,
    observed: Observed,
    breakpoint_updates: HashMap<u64, BreakpointUpdate>,
}

struct Shared {
    state: Mutex<SharedState>,
    changed: Condvar,
}

impl Shared {
    fn new() -> Self {
        Self {
            state: Mutex::new(SharedState::default()),
            changed: Condvar::new(),
        }
    }

    fn closed(&self) -> Option<DebuggerError> {
        self.state.lock().unwrap().closed.clone()
    }

    fn observed(&self) -> Observed {
        self.state.lock().unwrap().observed.clone()
    }

    fn cursor(&self) -> u64 {
        self.state.lock().unwrap().event_index
    }

    /// Records a terminal failure and releases every waiter.
    fn fail(&self, error: DebuggerError) {
        let mut state = self.state.lock().unwrap();
        if state.closed.is_none() {
            state.closed = Some(error.clone());
        }
        if !matches!(
            state.observed.state,
            SessionState::Exited | SessionState::Terminated
        ) {
            state.observed.state = SessionState::Closed;
            state.observed.stop = None;
            state.observed.generation += 1;
        }
        for (_, sender) in state.pending.drain() {
            let _ = sender.send(Err(error.clone()));
        }
        drop(state);
        self.changed.notify_all();
    }

    fn set_attached(&self) {
        let mut state = self.state.lock().unwrap();
        if state.observed.state == SessionState::Starting {
            state.observed.state = SessionState::Attached;
            state.observed.generation += 1;
        }
    }

    fn set_running_if_unchanged(&self, generation: u64) {
        let mut state = self.state.lock().unwrap();
        if state.observed.generation == generation {
            state.observed.state = SessionState::Running;
            state.observed.stop = None;
        }
    }

    fn breakpoint_updates(&self) -> HashMap<u64, BreakpointUpdate> {
        self.state.lock().unwrap().breakpoint_updates.clone()
    }

    fn apply_event(&self, event: &str, body: &Value) {
        let mut state = self.state.lock().unwrap();
        if matches!(event, "stopped" | "continued" | "exited" | "terminated") {
            state.observed.generation += 1;
        }
        match event {
            "stopped" => {
                state.observed.stop = Some(stop_info(body));
                state.observed.state = SessionState::Stopped;
            }
            "continued" => {
                state.observed.state = SessionState::Running;
                state.observed.stop = None;
            }
            "exited" => {
                state.observed.state = SessionState::Exited;
                state.observed.stop = None;
                state.observed.exit_code = body.get("exitCode").and_then(Value::as_i64);
            }
            "terminated" => {
                state.observed.state = SessionState::Terminated;
                state.observed.stop = None;
            }
            "process" => {
                if let Some(pid) = number(body, "systemProcessId")
                    && pid <= u32::MAX as u64
                {
                    state.observed.process_id = Some(pid as u32);
                }
                if let Some(name) = text(body, "name").filter(|name| !name.is_empty()) {
                    state.observed.process_name = Some(name);
                }
            }
            "breakpoint" => {
                let breakpoint = body.get("breakpoint").cloned().unwrap_or(Value::Null);
                if let Some(id) = number(&breakpoint, "id")
                    && (state.breakpoint_updates.len() < 512
                        || state.breakpoint_updates.contains_key(&id))
                {
                    let removed = text(body, "reason").as_deref() == Some("removed");
                    state.breakpoint_updates.insert(
                        id,
                        BreakpointUpdate {
                            verified: !removed
                                && breakpoint
                                    .get("verified")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            line: number(&breakpoint, "line")
                                .map(|line| line.min(u32::MAX as u64) as u32),
                            message: text(&breakpoint, "message")
                                .or_else(|| removed.then(|| "Removed by the adapter.".to_owned())),
                        },
                    );
                }
            }
            _ => {}
        }
        state.event_index += 1;
        let entry = EventEntry {
            index: state.event_index,
            value: json!({"event": event, "body": body}),
        };
        state.events.push_back(entry);
        while state.events.len() > MAX_EVENTS {
            state.events.pop_front();
            state.events_dropped += 1;
        }
        drop(state);
        self.changed.notify_all();
    }

    /// Returns the first event after `after` that `matches` accepts.
    ///
    /// Events that do not match stay queued for other observers.
    fn wait_event(
        &self,
        after: u64,
        mut matches: impl FnMut(&Value) -> bool,
        timeout: Duration,
    ) -> Result<Option<Value>> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().unwrap();
        loop {
            let position = state
                .events
                .iter()
                .position(|entry| entry.index > after && matches(&entry.value));
            if let Some(position) = position {
                return Ok(Some(state.events.remove(position).unwrap().value));
            }
            if let Some(error) = &state.closed {
                return Err(error.clone());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            state = self.changed.wait_timeout(state, remaining).unwrap().0;
        }
    }
}

type Writer = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

struct Transport {
    writer: Writer,
    shared: Arc<Shared>,
    reader: Mutex<Option<JoinHandle<()>>>,
    sequence: Arc<AtomicU64>,
}

impl Transport {
    fn new(reader: Box<dyn Read + Send>, writer: Box<dyn Write + Send>) -> Result<Self> {
        let writer = Arc::new(Mutex::new(Some(writer)));
        let shared = Arc::new(Shared::new());
        let sequence = Arc::new(AtomicU64::new(0));
        let thread_writer = Arc::clone(&writer);
        let thread_shared = Arc::clone(&shared);
        let thread_sequence = Arc::clone(&sequence);
        let handle = thread::Builder::new()
            .name("cove.debugger.reader".into())
            .spawn(move || {
                reader_loop(
                    BufReader::new(reader),
                    thread_writer,
                    thread_shared,
                    thread_sequence,
                );
            })
            .map_err(|error| {
                DebuggerError::new(
                    "adapter_io",
                    format!("Cannot start the DAP reader thread: {error}"),
                )
            })?;
        Ok(Self {
            writer,
            shared,
            reader: Mutex::new(Some(handle)),
            sequence,
        })
    }

    fn write(&self, message: &Value) -> Result<()> {
        let mut guard = self.writer.lock().unwrap();
        let writer = guard
            .as_mut()
            .ok_or_else(|| DebuggerError::new("adapter_io", "The adapter input is closed."))?;
        write_message(writer, message)
    }

    fn request(&self, command: &str, arguments: Value, timeout: Duration) -> Result<Value> {
        if let Some(error) = self.shared.closed() {
            return Err(error);
        }
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let (sender, receiver) = mpsc::channel();
        self.shared
            .state
            .lock()
            .unwrap()
            .pending
            .insert(sequence, sender);
        let message = json!({
            "seq": sequence,
            "type": "request",
            "command": command,
            "arguments": arguments,
        });
        if let Err(error) = self.write(&message) {
            self.shared.state.lock().unwrap().pending.remove(&sequence);
            return Err(error);
        }
        match receiver.recv_timeout(timeout) {
            Ok(response) => {
                let response = response?;
                if !response
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    return Err(DebuggerError::adapter(command, response_reason(&response)));
                }
                Ok(response.get("body").cloned().unwrap_or(Value::Null))
            }
            Err(RecvTimeoutError::Timeout) => {
                self.shared.state.lock().unwrap().pending.remove(&sequence);
                Err(DebuggerError::timeout(command, timeout))
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.shared.state.lock().unwrap().pending.remove(&sequence);
                Err(self.shared.closed().unwrap_or_else(|| {
                    DebuggerError::new("adapter_io", "The adapter response channel closed.")
                }))
            }
        }
    }

    fn answer_reverse_request(&self, message: &Value) {
        let request_seq = message.get("seq").and_then(Value::as_u64).unwrap_or(0);
        let command = message
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let response = json!({
            "seq": sequence,
            "type": "response",
            "request_seq": request_seq,
            "success": false,
            "command": command,
            "message": format!("Cove DevTools does not support the adapter request {command}."),
        });
        let _ = self.write(&response);
    }

    fn close_writer(&self) {
        let writer = self.writer.lock().unwrap().take();
        drop(writer);
    }

    fn join_reader(&self) {
        if let Some(handle) = self.reader.lock().unwrap().take() {
            join_bounded(handle, JOIN_TIMEOUT);
        }
    }
}

fn reader_loop(
    mut reader: BufReader<Box<dyn Read + Send>>,
    writer: Writer,
    shared: Arc<Shared>,
    sequence: Arc<AtomicU64>,
) {
    loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => match message.get("type").and_then(Value::as_str) {
                Some("response") => {
                    let Some(request_seq) = message.get("request_seq").and_then(Value::as_u64)
                    else {
                        continue;
                    };
                    let sender = shared.state.lock().unwrap().pending.remove(&request_seq);
                    if let Some(sender) = sender {
                        let _ = sender.send(Ok(message));
                    }
                }
                Some("event") => {
                    let event = message
                        .get("event")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let body = message.get("body").cloned().unwrap_or(Value::Null);
                    shared.apply_event(event, &body);
                }
                Some("request") => {
                    let transport = Transport {
                        writer: Arc::clone(&writer),
                        shared: Arc::clone(&shared),
                        reader: Mutex::new(None),
                        sequence: Arc::clone(&sequence),
                    };
                    transport.answer_reverse_request(&message);
                }
                _ => {}
            },
            Ok(None) => {
                shared.fail(DebuggerError::new(
                    "adapter_io",
                    "lldb-dap closed its output before the session ended.",
                ));
                break;
            }
            Err(error) => {
                shared.fail(error);
                break;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct BreakpointUpdate {
    verified: bool,
    line: Option<u32>,
    message: Option<String>,
}

#[derive(Clone, Debug)]
struct BreakpointSpec {
    line: u32,
    condition: Option<String>,
    log_message: Option<String>,
    id: Option<u64>,
    verified: bool,
    message: Option<String>,
    resolved_line: Option<u32>,
}

impl BreakpointSpec {
    fn to_breakpoint(&self, path: &str, updates: &HashMap<u64, BreakpointUpdate>) -> Breakpoint {
        let update = self.id.and_then(|id| updates.get(&id));
        Breakpoint {
            path: path.to_owned(),
            line: self.line,
            resolved_line: update.and_then(|update| update.line).or(self.resolved_line),
            condition: self.condition.clone(),
            log_message: self.log_message.clone(),
            id: self.id,
            verified: update.map_or(self.verified, |update| update.verified),
            message: update
                .map(|update| update.message.clone())
                .unwrap_or_else(|| self.message.clone()),
        }
    }
}

/// A live DAP session. All state is copied from adapter responses or events.
pub struct DapSession {
    process: Mutex<Option<AdapterProcess>>,
    transport: Transport,
    breakpoints: Mutex<BTreeMap<String, BTreeMap<u32, BreakpointSpec>>>,
    control: Mutex<()>,
    closed: AtomicBool,
    aborted: AtomicBool,
    initialized: AtomicBool,
    attached: AtomicBool,
    debuggee_pid: Mutex<Option<u32>>,
    adapter_version: Mutex<Option<String>>,
    capabilities: Mutex<Option<InitializeInfo>>,
    exception_filters: Mutex<Vec<String>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_thread: Mutex<Option<JoinHandle<()>>>,
    adapter_pid: u32,
    request_timeout_ms: AtomicU64,
}

impl DapSession {
    /// Starts `lldb-dap` and begins reading its DAP stream.
    pub fn spawn(adapter: &Adapter) -> Result<Self> {
        let (process, io) = AdapterProcess::spawn(adapter.program())?;
        let adapter_pid = process.pid();
        let (stderr, stderr_thread) = read_stderr_tail(io.stderr);
        let transport = Transport::new(Box::new(io.stdout), Box::new(io.stdin))?;
        Ok(Self {
            process: Mutex::new(Some(process)),
            transport,
            breakpoints: Mutex::new(BTreeMap::new()),
            control: Mutex::new(()),
            closed: AtomicBool::new(false),
            aborted: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            attached: AtomicBool::new(false),
            debuggee_pid: Mutex::new(None),
            adapter_version: Mutex::new(None),
            capabilities: Mutex::new(None),
            exception_filters: Mutex::new(Vec::new()),
            stderr,
            stderr_thread: Mutex::new(Some(stderr_thread)),
            adapter_pid,
            request_timeout_ms: AtomicU64::new(REQUEST_TIMEOUT.as_millis() as u64),
        })
    }

    /// Connects to a DAP stream that is already open (used by the in-process
    /// mock adapter in tests and adapter-less verification).
    pub fn from_io(reader: Box<dyn Read + Send>, writer: Box<dyn Write + Send>) -> Result<Self> {
        let transport = Transport::new(reader, writer)?;
        Ok(Self {
            process: Mutex::new(None),
            transport,
            breakpoints: Mutex::new(BTreeMap::new()),
            control: Mutex::new(()),
            closed: AtomicBool::new(false),
            aborted: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            attached: AtomicBool::new(false),
            debuggee_pid: Mutex::new(None),
            adapter_version: Mutex::new(None),
            capabilities: Mutex::new(None),
            exception_filters: Mutex::new(Vec::new()),
            stderr: Arc::new(Mutex::new(Vec::new())),
            stderr_thread: Mutex::new(None),
            adapter_pid: 0,
            request_timeout_ms: AtomicU64::new(REQUEST_TIMEOUT.as_millis() as u64),
        })
    }

    pub fn adapter_pid(&self) -> u32 {
        self.adapter_pid
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.transport.shared.closed().is_some()
    }

    /// Shortens ordinary request timeouts. Test support for timeout paths.
    #[doc(hidden)]
    pub fn set_request_timeout(&self, timeout: Duration) {
        self.request_timeout_ms
            .store(timeout.as_millis().max(1) as u64, Ordering::Relaxed);
    }

    fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms.load(Ordering::Relaxed))
    }

    pub fn diagnostics(&self) -> String {
        let bytes = self.stderr.lock().unwrap().clone();
        String::from_utf8_lossy(&bytes).trim().to_owned()
    }

    fn ensure_open(&self) -> Result<()> {
        if let Some(error) = self.transport.shared.closed() {
            return Err(error);
        }
        Ok(())
    }

    fn observed(&self) -> Observed {
        self.transport.shared.observed()
    }

    fn ensure_attached(&self) -> Result<Observed> {
        self.ensure_open()?;
        let observed = self.observed();
        match observed.state {
            SessionState::Attached | SessionState::Running | SessionState::Stopped => Ok(observed),
            state => Err(DebuggerError::new(
                "not_attached",
                format!("The debugger session is {}.", state_name(state)),
            )),
        }
    }

    fn ensure_stopped(&self) -> Result<Observed> {
        let observed = self.ensure_attached()?;
        if observed.state != SessionState::Stopped {
            return Err(DebuggerError::new(
                "not_stopped",
                format!(
                    "The debuggee is {}; pause it or hit a breakpoint before reading paused state.",
                    state_name(observed.state)
                ),
            ));
        }
        Ok(observed)
    }

    /// Initializes the adapter and keeps its advertised version.
    pub fn initialize(&self) -> Result<InitializeInfo> {
        if self.initialized.swap(true, Ordering::AcqRel) {
            return Err(invalid("The debug adapter is already initialized."));
        }
        let body = self.transport.request(
            "initialize",
            json!({
                "clientID": "cove-devtools",
                "clientName": "Cove DevTools",
                "adapterID": "lldb",
                "locale": "en-US",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "pathFormat": "path",
                "supportsVariableType": true,
                "supportsVariablePaging": true,
                "supportsRunInTerminalRequest": false,
                "supportsStartDebuggingRequest": false,
                "supportsMemoryReferences": true,
                "supportsInvalidatedEvent": true,
            }),
            INITIALIZE_TIMEOUT,
        )?;
        let version = ["$__lldb_version", "adapterVersion", "version"]
            .iter()
            .find_map(|key| text(&body, key))
            .map(|version| version.chars().take(MAX_VERSION_CHARS).collect::<String>());
        if let Some(version) = &version {
            *self.adapter_version.lock().unwrap() = Some(version.clone());
        }
        let info = InitializeInfo {
            adapter_version: version,
            supports_configuration_done: body
                .get("supportsConfigurationDoneRequest")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            supports_function_breakpoints: body
                .get("supportsFunctionBreakpoints")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            supports_conditional_breakpoints: body
                .get("supportsConditionalBreakpoints")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            supports_log_points: body["supportsLogPoints"].as_bool().unwrap_or(false),
            exception_filters: body["exceptionBreakpointFilters"]
                .as_array()
                .into_iter()
                .flatten()
                .take(32)
                .filter_map(|filter| {
                    let id = filter["filter"].as_str()?;
                    if id.is_empty() || id.len() > 128 {
                        return None;
                    }
                    Some(crate::model::ExceptionFilter {
                        id: id.to_owned(),
                        label: filter["label"]
                            .as_str()
                            .unwrap_or(id)
                            .chars()
                            .take(160)
                            .collect(),
                    })
                })
                .collect(),
        };
        *self.capabilities.lock().unwrap() = Some(info.clone());
        Ok(info)
    }

    pub fn capabilities(&self) -> Option<InitializeInfo> {
        self.capabilities.lock().unwrap().clone()
    }

    pub fn exception_filters(&self) -> Vec<String> {
        self.exception_filters.lock().unwrap().clone()
    }

    pub fn set_exception_filters(&self, filters: &[String]) -> Result<Vec<String>> {
        let _control = self.control.lock().unwrap();
        self.ensure_attached()?;
        let available = self
            .capabilities()
            .map(|info| info.exception_filters)
            .unwrap_or_default();
        if available.is_empty() {
            return Err(DebuggerError::new(
                "unavailable",
                "The adapter reports no exception filters.",
            ));
        }
        let unique: std::collections::BTreeSet<_> = filters.iter().collect();
        if unique.len() != filters.len()
            || filters
                .iter()
                .any(|id| !available.iter().any(|filter| filter.id == *id))
        {
            return Err(invalid(
                "Choose distinct exception filter IDs advertised by the adapter.",
            ));
        }
        self.transport.request(
            "setExceptionBreakpoints",
            json!({"filters": filters}),
            self.request_timeout(),
        )?;
        *self.exception_filters.lock().unwrap() = filters.to_vec();
        Ok(filters.to_vec())
    }

    pub fn output(&self, after: u64) -> Vec<crate::model::DebuggerOutput> {
        let state = self.transport.shared.state.lock().unwrap();
        state
            .events
            .iter()
            .filter(|entry| entry.index > after && entry.value["event"] == "output")
            .filter_map(|entry| {
                let body = &entry.value["body"];
                Some(crate::model::DebuggerOutput {
                    sequence: entry.index,
                    category: body["category"]
                        .as_str()
                        .unwrap_or("console")
                        .chars()
                        .take(32)
                        .collect(),
                    text: body["output"].as_str()?.chars().take(2048).collect(),
                })
            })
            .take(40)
            .collect()
    }

    /// Attaches to `pid` and completes DAP configuration.
    ///
    /// A refusal (permissions, missing process) is reported with the adapter's
    /// own words and the session is torn down; no paused state is invented.
    pub fn attach(&self, pid: u32) -> Result<SessionSnapshot> {
        self.attach_with(json!({"pid": pid, "stopOnEntry": false}), Some(pid))
    }

    /// Attaches through a GDB remote stub forwarded to the controller host.
    pub fn attach_loopback(
        &self,
        port: u16,
        program: Option<&str>,
        pid: u32,
    ) -> Result<SessionSnapshot> {
        let mut args =
            json!({"gdb-remote-host": "127.0.0.1", "gdb-remote-port": port, "stopOnEntry": false});
        if let Some(program) = program {
            args["program"] = json!(program);
        }
        self.attach_with(args, Some(pid))
    }

    fn attach_with(&self, arguments: Value, pid: Option<u32>) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        self.ensure_open()?;
        if self.attached.swap(true, Ordering::AcqRel) {
            return Err(invalid("This session is already attached."));
        }
        let cursor = self.transport.shared.cursor();
        let attach = self.transport.request("attach", arguments, ATTACH_TIMEOUT);
        let body = match attach {
            Ok(body) => body,
            Err(error) => {
                let error = self.attach_failure(error);
                self.shutdown(error.clone());
                return Err(error);
            }
        };
        let _ = self.transport.shared.wait_event(
            cursor,
            |event| event["event"] == "initialized",
            EVENT_WAIT,
        );
        if let Err(error) =
            self.transport
                .request("configurationDone", json!({}), CONFIGURATION_TIMEOUT)
        {
            let error = self.attach_failure(error);
            self.shutdown(error.clone());
            return Err(error);
        }
        *self.debuggee_pid.lock().unwrap() = pid;
        let _ = self.transport.shared.wait_event(
            cursor,
            |event| event["event"] == "stopped",
            EVENT_WAIT,
        );
        self.transport.shared.set_attached();
        if let Some(process_id) = body.get("systemProcessId").and_then(Value::as_u64)
            && process_id <= u32::MAX as u64
        {
            self.transport
                .shared
                .state
                .lock()
                .unwrap()
                .observed
                .process_id = Some(process_id as u32);
        }
        Ok(self.snapshot())
    }

    fn attach_reason(&self, adapter_message: &str) -> String {
        let diagnostics = self.diagnostics();
        if diagnostics.is_empty() {
            adapter_message.to_owned()
        } else {
            format!("{adapter_message} Adapter stderr: {diagnostics}")
        }
    }

    /// Keeps a timeout or our own abort distinct from an adapter refusal.
    fn attach_failure(&self, error: DebuggerError) -> DebuggerError {
        let reason = self.attach_reason(error.message());
        let code = match error.code() {
            "timeout" => "timeout",
            "closed" => "closed",
            "adapter_io" | "protocol_error" => "adapter_io",
            _ => "unavailable",
        };
        DebuggerError::new(code, reason)
    }

    /// Detaches without terminating the debuggee, then closes the adapter.
    pub fn detach(&self) -> Result<()> {
        let _control = self.control.lock().unwrap();
        let state = self.observed().state;
        if matches!(
            state,
            SessionState::Exited | SessionState::Terminated | SessionState::Closed
        ) {
            self.shutdown(DebuggerError::new(
                "closed",
                "The debugger session is closed.",
            ));
            return Ok(());
        }
        let result = self.transport.request(
            "disconnect",
            json!({"terminateDebuggee": false, "restart": false}),
            DETACH_TIMEOUT,
        );
        let diagnostics = self.diagnostics();
        self.shutdown(DebuggerError::new(
            "closed",
            "The debugger session is closed.",
        ));
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                let mut message = format!("lldb-dap did not detach cleanly: {}", error.message());
                if !diagnostics.is_empty() {
                    message.push_str(&format!(" Adapter stderr: {diagnostics}"));
                }
                Err(DebuggerError::new("detach_failed", message))
            }
        }
    }

    /// Ends the adapter and its helpers. Safe to call repeatedly and from any
    /// thread; pending requests are failed rather than left waiting.
    ///
    /// A debuggee that already stopped is asked to detach first so a controller
    /// timeout cannot leave the target suspended when the adapter dies.
    pub fn abort(&self) {
        if self.aborted.load(Ordering::Acquire) {
            return;
        }
        let state = self.observed().state;
        if !self.is_closed()
            && !matches!(
                state,
                SessionState::Starting
                    | SessionState::Exited
                    | SessionState::Terminated
                    | SessionState::Closed
            )
        {
            let _ = self.transport.request(
                "disconnect",
                json!({"terminateDebuggee": false, "restart": false}),
                Duration::from_secs(1),
            );
        }
        self.shutdown(DebuggerError::new(
            "closed",
            "The debugger session is closed.",
        ));
    }

    fn shutdown(&self, error: DebuggerError) {
        if self.aborted.swap(true, Ordering::AcqRel) {
            return;
        }
        self.closed.store(true, Ordering::Release);
        self.transport.shared.fail(error);
        self.transport.close_writer();
        if let Some(mut process) = self.process.lock().unwrap().take() {
            process.terminate();
        }
        self.transport.join_reader();
        if let Some(handle) = self.stderr_thread.lock().unwrap().take() {
            join_bounded(handle, JOIN_TIMEOUT);
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let observed = self.observed();
        let events_dropped = self.transport.shared.state.lock().unwrap().events_dropped;
        SessionSnapshot {
            state: observed.state,
            stop: observed.stop,
            pid: self.debuggee_pid.lock().unwrap().or(observed.process_id),
            process_name: observed.process_name,
            adapter_version: self.adapter_version.lock().unwrap().clone(),
            exit_code: observed.exit_code,
            events_dropped,
            breakpoints: self.breakpoints(),
        }
    }

    pub fn breakpoints(&self) -> Vec<Breakpoint> {
        let updates = self.transport.shared.breakpoint_updates();
        let registry = self.breakpoints.lock().unwrap();
        registry
            .iter()
            .flat_map(|(path, lines)| {
                lines
                    .values()
                    .map(|spec| spec.to_breakpoint(path, &updates))
            })
            .collect()
    }

    fn normalized_path(path: &str) -> Result<String> {
        if path.is_empty() || !Path::new(path).is_absolute() {
            return Err(invalid("Use an absolute source path."));
        }
        Ok(std::fs::canonicalize(path)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_owned()))
    }

    fn breakpoints_for(
        registry: &BTreeMap<String, BTreeMap<u32, BreakpointSpec>>,
        path: &str,
        updates: &HashMap<u64, BreakpointUpdate>,
    ) -> Vec<Breakpoint> {
        registry
            .get(path)
            .map(|lines| {
                lines
                    .values()
                    .map(|spec| spec.to_breakpoint(path, updates))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Adds or replaces one breakpoint and sends the complete source list.
    pub fn set_breakpoint(
        &self,
        path: &str,
        line: u32,
        condition: Option<&str>,
    ) -> Result<Vec<Breakpoint>> {
        self.set_source_breakpoint(path, line, condition, None)
    }

    pub fn set_source_breakpoint(
        &self,
        path: &str,
        line: u32,
        condition: Option<&str>,
        log_message: Option<&str>,
    ) -> Result<Vec<Breakpoint>> {
        if line == 0 {
            return Err(invalid("A breakpoint line must be at least 1."));
        }
        self.ensure_attached()?;
        if log_message.is_some()
            && !self
                .capabilities()
                .is_some_and(|info| info.supports_log_points)
        {
            return Err(DebuggerError::new(
                "unavailable",
                "The adapter does not advertise log points.",
            ));
        }
        let path = Self::normalized_path(path)?;
        let _control = self.control.lock().unwrap();
        let mut registry = self.breakpoints.lock().unwrap();
        let previous = registry.clone();
        registry.entry(path.clone()).or_default().insert(
            line,
            BreakpointSpec {
                line,
                condition: condition.map(str::to_owned),
                log_message: log_message.map(str::to_owned),
                id: None,
                verified: false,
                message: None,
                resolved_line: None,
            },
        );
        let specs: Vec<_> = registry
            .get(&path)
            .map(|lines| lines.values().cloned().collect())
            .unwrap_or_default();
        match self.set_remote_breakpoints(&path, &specs) {
            Ok(updated) => {
                if let Some(lines) = registry.get_mut(&path) {
                    for spec in updated {
                        if let Some(stored) = lines.get_mut(&spec.line) {
                            stored.id = spec.id;
                            stored.verified = spec.verified;
                            stored.message = spec.message;
                            stored.resolved_line = spec.resolved_line;
                        }
                    }
                }
                let updates = self.transport.shared.breakpoint_updates();
                Ok(Self::breakpoints_for(&registry, &path, &updates))
            }
            Err(error) => {
                *registry = previous;
                Err(error)
            }
        }
    }

    /// Clears one line, or every breakpoint in the source when `line` is None.
    pub fn clear_breakpoints(&self, path: &str, line: Option<u32>) -> Result<Vec<Breakpoint>> {
        self.ensure_attached()?;
        let path = Self::normalized_path(path)?;
        let _control = self.control.lock().unwrap();
        let mut registry = self.breakpoints.lock().unwrap();
        let previous = registry.clone();
        if let Some(line) = line {
            if let Some(lines) = registry.get_mut(&path) {
                lines.remove(&line);
            }
        } else {
            registry.remove(&path);
        }
        if registry.get(&path).is_some_and(BTreeMap::is_empty) {
            registry.remove(&path);
        }
        let specs: Vec<_> = registry
            .get(&path)
            .map(|lines| lines.values().cloned().collect())
            .unwrap_or_default();
        match self.set_remote_breakpoints(&path, &specs) {
            Ok(updated) => {
                if let Some(lines) = registry.get_mut(&path) {
                    for spec in updated {
                        if let Some(stored) = lines.get_mut(&spec.line) {
                            stored.id = spec.id;
                            stored.verified = spec.verified;
                            stored.message = spec.message;
                            stored.resolved_line = spec.resolved_line;
                        }
                    }
                }
                let updates = self.transport.shared.breakpoint_updates();
                Ok(Self::breakpoints_for(&registry, &path, &updates))
            }
            Err(error) => {
                *registry = previous;
                Err(error)
            }
        }
    }

    fn set_remote_breakpoints(
        &self,
        path: &str,
        specs: &[BreakpointSpec],
    ) -> Result<Vec<Breakpoint>> {
        let requested: Vec<_> = specs
            .iter()
            .map(|spec| {
                let mut entry = json!({"line": spec.line});
                if let Some(condition) = &spec.condition {
                    entry["condition"] = json!(condition);
                }
                if let Some(log_message) = &spec.log_message {
                    entry["logMessage"] = json!(log_message);
                }
                entry
            })
            .collect();
        let body = self.transport.request(
            "setBreakpoints",
            json!({
                "source": {"path": path},
                "breakpoints": requested,
                "sourceModified": false,
            }),
            self.request_timeout(),
        )?;
        let returned = body
            .get("breakpoints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let updates = self.transport.shared.breakpoint_updates();
        Ok(specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let actual = returned.get(index);
                let id = actual.and_then(|value| number(value, "id"));
                let update = id.and_then(|id| updates.get(&id));
                Breakpoint {
                    path: path.to_owned(),
                    line: spec.line,
                    resolved_line: update.and_then(|update| update.line).or_else(|| {
                        actual
                            .and_then(|value| number(value, "line"))
                            .map(|line| line.min(u32::MAX as u64) as u32)
                    }),
                    condition: spec.condition.clone(),
                    log_message: spec.log_message.clone(),
                    id,
                    verified: update.map_or_else(
                        || {
                            actual
                                .and_then(|value| value.get("verified"))
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                        },
                        |update| update.verified,
                    ),
                    message: update
                        .map(|update| update.message.clone())
                        .unwrap_or_else(|| actual.and_then(|value| text(value, "message"))),
                }
            })
            .collect())
    }

    pub fn threads(&self) -> Result<Vec<ThreadInfo>> {
        self.ensure_attached()?;
        let body = self
            .transport
            .request("threads", json!({}), self.request_timeout())?;
        let threads = body
            .get("threads")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(threads
            .into_iter()
            .filter_map(|thread| {
                let id = number(&thread, "id")?;
                Some(ThreadInfo {
                    id,
                    name: text(&thread, "name").unwrap_or_default(),
                })
            })
            .collect())
    }

    fn control_thread(&self, observed: &Observed) -> Result<u64> {
        if let Some(thread) = observed.stop.as_ref().and_then(|stop| stop.thread_id) {
            return Ok(thread);
        }
        self.threads()?
            .first()
            .map(|thread| thread.id)
            .ok_or_else(|| DebuggerError::new("not_stopped", "The adapter reported no threads."))
    }

    pub fn stack_trace(
        &self,
        thread_id: Option<u64>,
        start_frame: u64,
        levels: u64,
    ) -> Result<StackTrace> {
        let observed = self.ensure_stopped()?;
        let thread_id = match thread_id {
            Some(thread) => thread,
            None => self.control_thread(&observed)?,
        };
        let body = self.transport.request(
            "stackTrace",
            json!({"threadId": thread_id, "startFrame": start_frame, "levels": levels}),
            self.request_timeout(),
        )?;
        let frames = body
            .get("stackFrames")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let frames: Vec<StackFrame> = frames
            .into_iter()
            .filter_map(|frame| {
                let id = number(&frame, "id")?;
                Some(StackFrame {
                    id,
                    name: text(&frame, "name").unwrap_or_default(),
                    source: source(frame.get("source")),
                    line: number(&frame, "line").map(|line| line.min(u32::MAX as u64) as u32),
                    column: number(&frame, "column")
                        .map(|column| column.min(u32::MAX as u64) as u32),
                    instruction_pointer: text(&frame, "instructionPointerReference"),
                })
            })
            .collect();
        let total_frames = number(&body, "totalFrames");
        let truncated = total_frames.is_some_and(|total| total > start_frame + frames.len() as u64)
            || frames.len() as u64 >= levels.max(1);
        Ok(StackTrace {
            thread_id,
            frames,
            total_frames,
            start_frame,
            truncated,
        })
    }

    pub fn scopes(&self, frame_id: u64) -> Result<Vec<Scope>> {
        self.ensure_stopped()?;
        let body = self.transport.request(
            "scopes",
            json!({"frameId": frame_id}),
            self.request_timeout(),
        )?;
        let scopes = body
            .get("scopes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(scopes
            .into_iter()
            .filter_map(|scope| {
                let variables_reference = number(&scope, "variablesReference")?;
                Some(Scope {
                    name: text(&scope, "name").unwrap_or_default(),
                    variables_reference,
                    expensive: scope
                        .get("expensive")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    presentation_hint: text(&scope, "presentationHint"),
                    source: source(scope.get("source")),
                    line: number(&scope, "line").map(|line| line.min(u32::MAX as u64) as u32),
                })
            })
            .collect())
    }

    pub fn variables(
        &self,
        variables_reference: u64,
        start: u64,
        count: u64,
        filter: Option<&str>,
    ) -> Result<VariablePage> {
        if variables_reference == 0 {
            return Err(invalid(
                "A variables reference of 0 has no children; read a scope reference instead.",
            ));
        }
        self.ensure_stopped()?;
        let mut arguments = json!({
            "variablesReference": variables_reference,
            "start": start,
            "count": count,
        });
        if let Some(filter) = filter {
            arguments["filter"] = json!(filter);
        }
        let body = self
            .transport
            .request("variables", arguments, self.request_timeout())?;
        let variables = body
            .get("variables")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let variables: Vec<Variable> = variables
            .into_iter()
            .map(|variable| {
                let (value, value_truncated) =
                    truncate(variable.get("value").and_then(Value::as_str).unwrap_or(""));
                Variable {
                    name: variable
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    value,
                    value_truncated,
                    r#type: text(&variable, "type"),
                    variables_reference: number(&variable, "variablesReference").unwrap_or(0),
                    evaluate_name: text(&variable, "evaluateName"),
                    named_variables: number(&variable, "namedVariables"),
                    indexed_variables: number(&variable, "indexedVariables"),
                    memory_reference: text(&variable, "memoryReference"),
                }
            })
            .collect();
        let returned = variables.len() as u64;
        Ok(VariablePage {
            variables,
            start,
            count,
            total: None,
            truncated: count > 0 && returned >= count,
        })
    }

    pub fn evaluate(
        &self,
        expression: &str,
        frame_id: Option<u64>,
        context: Option<&str>,
    ) -> Result<EvaluateResult> {
        if expression.is_empty() {
            return Err(invalid("An evaluate expression must not be empty."));
        }
        self.ensure_stopped()?;
        let mut arguments = json!({"expression": expression, "context": context.unwrap_or("repl")});
        if let Some(frame_id) = frame_id {
            arguments["frameId"] = json!(frame_id);
        }
        let body = self
            .transport
            .request("evaluate", arguments, self.request_timeout())?;
        let (result, result_truncated) = truncate(
            body.get("result")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        Ok(EvaluateResult {
            result,
            result_truncated,
            r#type: text(&body, "type"),
            variables_reference: number(&body, "variablesReference").unwrap_or(0),
            named_variables: number(&body, "namedVariables"),
            indexed_variables: number(&body, "indexedVariables"),
            memory_reference: text(&body, "memoryReference"),
        })
    }

    /// Pauses a running debuggee. Acknowledging the request is not enough: the
    /// returned snapshot only says stopped when a `stopped` event arrived.
    pub fn pause(&self, thread_id: Option<u64>) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        let observed = self.ensure_attached()?;
        match observed.state {
            SessionState::Stopped => {
                return Err(invalid("The debuggee is already stopped."));
            }
            SessionState::Attached | SessionState::Running => {}
            state => {
                return Err(DebuggerError::new(
                    "not_attached",
                    format!("The debugger session is {}.", state_name(state)),
                ));
            }
        }
        let cursor = self.transport.shared.cursor();
        let mut arguments = json!({});
        if let Some(thread_id) = thread_id {
            arguments["threadId"] = json!(thread_id);
        }
        self.transport
            .request("pause", arguments, self.request_timeout())?;
        let event = self.transport.shared.wait_event(
            cursor,
            |event| event["event"] == "stopped",
            CONTROL_WAIT,
        )?;
        if event.is_none() {
            return Err(DebuggerError::new(
                "timeout",
                format!(
                    "lldb-dap acknowledged pause but sent no stopped event within {} ms.",
                    CONTROL_WAIT.as_millis()
                ),
            ));
        }
        Ok(self.snapshot())
    }

    /// Resumes a stopped debuggee.
    pub fn resume(&self) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        let observed = self.ensure_stopped()?;
        let thread_id = self.control_thread(&observed)?;
        let generation = observed.generation;
        self.transport.request(
            "continue",
            json!({"threadId": thread_id, "singleThread": false}),
            self.request_timeout(),
        )?;
        self.transport.shared.set_running_if_unchanged(generation);
        Ok(self.snapshot())
    }

    /// Steps a stopped debuggee and waits for the next `stopped` event.
    pub fn step(&self, command: Step) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        let observed = self.ensure_stopped()?;
        let thread_id = self.control_thread(&observed)?;
        let cursor = self.transport.shared.cursor();
        self.transport.request(
            command.command(),
            json!({"threadId": thread_id}),
            self.request_timeout(),
        )?;
        let event = self.transport.shared.wait_event(
            cursor,
            |event| event["event"] == "stopped",
            CONTROL_WAIT,
        )?;
        if event.is_none() {
            return Err(DebuggerError::new(
                "timeout",
                format!(
                    "lldb-dap acknowledged {} but sent no stopped event within {} ms.",
                    command.command(),
                    CONTROL_WAIT.as_millis()
                ),
            ));
        }
        Ok(self.snapshot())
    }
}

impl std::fmt::Debug for DapSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DapSession")
            .field("adapter_pid", &self.adapter_pid)
            .field("state", &self.observed().state)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl Drop for DapSession {
    fn drop(&mut self) {
        self.abort();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Over,
    In,
    Out,
}

impl Step {
    fn command(self) -> &'static str {
        match self {
            Self::Over => "next",
            Self::In => "stepIn",
            Self::Out => "stepOut",
        }
    }
}
