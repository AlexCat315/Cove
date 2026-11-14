//! One debugger session over in-process liblldb (the SB API).
//!
//! The session drives the shim loaded by [`crate::ffi`]. An event thread pulls
//! process events from LLDB and wakes waiters; paused state is whatever LLDB
//! reported for the last stop and is never remembered across a resume.
use crate::backend::{normalized_source_path, state_name};
use crate::error::{DebuggerError, Result};
use crate::ffi::{self, Library, RawFrame, RawOutput, RawScope, RawState, RawThread, RawVariable};
use crate::model::{
    Breakpoint, DebuggerOutput, EvaluateResult, ExceptionFilter, InitializeInfo, Scope,
    SessionSnapshot, SessionState, SourceInfo, StackFrame, StackTrace, StopInfo, ThreadInfo,
    Variable, VariablePage,
};
use crate::session::Step;
use std::collections::{BTreeMap, VecDeque};
use std::ffi::{CStr, CString, c_int};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const ATTACH_TIMEOUT: Duration = Duration::from_secs(10);
const CONTROL_WAIT: Duration = Duration::from_secs(3);
const RESUME_WAIT: Duration = Duration::from_millis(500);
const EVALUATE_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_POLL_MS: u32 = 1000;
const MAX_FRAMES: u64 = 256;
const MAX_VARIABLES: u64 = 200;
const MAX_THREADS: usize = 512;
const MAX_OUTPUT: usize = 256;

const EXCEPTION_FILTERS: [(&str, &str); 5] = [
    ("rust_panic", "Rust Panic"),
    ("cpp_throw", "C++ Throw"),
    ("cpp_catch", "C++ Catch"),
    ("objc_throw", "Objective-C Throw"),
    ("objc_catch", "Objective-C Catch"),
];

fn invalid(message: impl Into<String>) -> DebuggerError {
    DebuggerError::new("invalid_request", message)
}

fn closed_error() -> DebuggerError {
    DebuggerError::new("closed", "The debugger session is closed.")
}

/// The shim-owned session pointer.
#[derive(Clone, Copy)]
struct Raw(ffi::Session);

// SAFETY: the shim guards its session state with a mutex and the SB API is
// thread-safe; the pointer is freed only in `Drop`, after the event thread ends.
unsafe impl Send for Raw {}
// SAFETY: see `Send`.
unsafe impl Sync for Raw {}

struct OutputLog {
    entries: VecDeque<DebuggerOutput>,
    sequence: u64,
    dropped: u64,
}

struct Shared {
    library: &'static Library,
    raw: Raw,
    /// Counts consumed LLDB events; waiters sleep on `changed`.
    tick: Mutex<u64>,
    changed: Condvar,
    closed: AtomicBool,
    output: Mutex<OutputLog>,
}

impl Shared {
    fn state(&self) -> RawState {
        // SAFETY: `raw` is live until `Drop`; the shim fills the whole struct.
        unsafe {
            let mut state: RawState = std::mem::zeroed();
            (self.library.get_state)(self.raw.0, &mut state);
            state
        }
    }

    fn last_error(&self) -> String {
        // SAFETY: the library is resolved and the string is copied immediately.
        let text = unsafe { ffi::last_error(self.library) };
        if text.trim().is_empty() {
            "LLDB gave no reason".to_owned()
        } else {
            text
        }
    }

    fn rejected(&self, command: &str) -> DebuggerError {
        DebuggerError::native(command, self.last_error())
    }

    fn notify(&self) {
        *self.tick.lock().unwrap() += 1;
        self.changed.notify_all();
    }

    fn drain_output(&self) {
        let mut log = self.output.lock().unwrap();
        loop {
            // SAFETY: the buffer holds 16 entries and the shim writes at most 16.
            let mut buffer: [RawOutput; 16] = unsafe { std::mem::zeroed() };
            let mut count: c_int = 0;
            // SAFETY: `buffer` holds 16 entries and outlives the call.
            let code = unsafe {
                (self.library.drain_output)(self.raw.0, buffer.as_mut_ptr(), 16, &mut count)
            };
            if code != ffi::OK || count <= 0 {
                break;
            }
            for entry in &buffer[..count as usize] {
                log.sequence += 1;
                let sequence = log.sequence;
                log.entries.push_back(DebuggerOutput {
                    sequence,
                    category: match entry.category {
                        ffi::OUTPUT_STDOUT => "stdout",
                        ffi::OUTPUT_STDERR => "stderr",
                        ffi::OUTPUT_LOG => "console",
                        _ => "console",
                    }
                    .to_owned(),
                    text: ffi::char_array(&entry.text),
                });
                while log.entries.len() > MAX_OUTPUT {
                    log.entries.pop_front();
                    log.dropped += 1;
                }
            }
        }
    }

    /// Waits until `done` accepts the state, the session closes, or `timeout`.
    fn wait(&self, timeout: Duration, mut done: impl FnMut(&RawState) -> bool) -> Option<RawState> {
        let deadline = Instant::now() + timeout;
        let mut tick = self.tick.lock().unwrap();
        loop {
            let state = self.state();
            if done(&state) || self.closed.load(Ordering::Acquire) {
                return Some(state);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            tick = self.changed.wait_timeout(tick, remaining).unwrap().0;
        }
    }
}

fn event_loop(shared: Arc<Shared>) {
    while !shared.closed.load(Ordering::Acquire) {
        // SAFETY: `raw` outlives this thread; `Drop` joins it before freeing.
        let consumed = unsafe { (shared.library.wait_event)(shared.raw.0, EVENT_POLL_MS) };
        if consumed != 0 {
            shared.drain_output();
            shared.notify();
        }
    }
}

fn session_state(code: c_int) -> SessionState {
    match code {
        ffi::STATE_ATTACHED => SessionState::Attached,
        ffi::STATE_RUNNING => SessionState::Running,
        ffi::STATE_STOPPED => SessionState::Stopped,
        ffi::STATE_EXITED => SessionState::Exited,
        ffi::STATE_TERMINATED => SessionState::Terminated,
        ffi::STATE_CLOSED => SessionState::Closed,
        _ => SessionState::Starting,
    }
}

fn terminal(state: &RawState) -> bool {
    matches!(
        state.state,
        ffi::STATE_EXITED | ffi::STATE_TERMINATED | ffi::STATE_CLOSED
    )
}

fn stop_info(state: &RawState) -> StopInfo {
    let description = ffi::char_array(&state.description);
    let count = (state.num_breakpoints.max(0) as usize).min(state.breakpoint_ids.len());
    StopInfo {
        reason: match state.stop_reason {
            ffi::STOP_BREAKPOINT => "breakpoint",
            ffi::STOP_STEP => "step",
            ffi::STOP_SIGNAL => "signal",
            ffi::STOP_EXCEPTION => "exception",
            ffi::STOP_WATCHPOINT => "data breakpoint",
            ffi::STOP_NONE => "pause",
            _ => "unknown",
        }
        .to_owned(),
        description: (!description.is_empty()).then_some(description),
        thread_id: (state.thread_id != 0).then_some(state.thread_id),
        all_threads_stopped: true,
        text: None,
        hit_breakpoint_ids: state.breakpoint_ids[..count].to_vec(),
    }
}

fn optional(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

fn variable(raw: &RawVariable) -> Variable {
    Variable {
        name: ffi::char_array(&raw.name),
        value: ffi::char_array(&raw.value),
        value_truncated: raw.value_truncated != 0,
        r#type: optional(ffi::char_array(&raw.r#type)),
        variables_reference: raw.reference,
        evaluate_name: optional(ffi::char_array(&raw.evaluate_name)),
        named_variables: None,
        indexed_variables: None,
        memory_reference: (raw.has_address != 0).then(|| format!("0x{:x}", raw.address)),
    }
}

#[derive(Clone, Debug)]
struct BreakpointSpec {
    line: u32,
    condition: Option<String>,
    log_message: Option<String>,
    id: u64,
    verified: bool,
    resolved_line: Option<u32>,
}

type Registry = BTreeMap<String, BTreeMap<u32, BreakpointSpec>>;

/// A live in-process LLDB session.
pub struct NativeSession {
    shared: Arc<Shared>,
    version: String,
    capabilities: InitializeInfo,
    breakpoints: Mutex<Registry>,
    control: Mutex<()>,
    aborted: AtomicBool,
    initialized: AtomicBool,
    attached: AtomicBool,
    debuggee_pid: Mutex<Option<u32>>,
    exception_filters: Mutex<Vec<String>>,
    event_thread: Mutex<Option<JoinHandle<()>>>,
}

impl NativeSession {
    /// Loads liblldb into this process and creates a debugger.
    pub fn create() -> Result<Self> {
        let library = ffi::library().ok_or_else(|| {
            DebuggerError::unavailable(
                ffi::unavailable_reason()
                    .unwrap_or("The native LLDB backend could not be loaded.")
                    .to_owned(),
            )
        })?;
        // SAFETY: the library is resolved; initialize is idempotent.
        let code = unsafe { (library.init)() };
        if code != ffi::OK {
            // SAFETY: as above.
            return Err(DebuggerError::unavailable(unsafe {
                ffi::last_error(library)
            }));
        }
        // SAFETY: the version string is static in liblldb.
        let version = unsafe {
            let pointer = (library.version)();
            if pointer.is_null() {
                String::new()
            } else {
                CStr::from_ptr(pointer).to_string_lossy().into_owned()
            }
        };
        // SAFETY: initialize succeeded.
        let raw = unsafe { (library.session_create)() };
        if raw.is_null() {
            // SAFETY: as above.
            return Err(DebuggerError::unavailable(unsafe {
                ffi::last_error(library)
            }));
        }
        let capabilities = InitializeInfo {
            adapter_version: optional(version.clone()),
            supports_configuration_done: false,
            supports_function_breakpoints: false,
            supports_conditional_breakpoints: true,
            supports_log_points: true,
            exception_filters: EXCEPTION_FILTERS
                .iter()
                .map(|(id, label)| ExceptionFilter {
                    id: (*id).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
        };
        Ok(Self {
            shared: Arc::new(Shared {
                library,
                raw: Raw(raw),
                tick: Mutex::new(0),
                changed: Condvar::new(),
                closed: AtomicBool::new(false),
                output: Mutex::new(OutputLog {
                    entries: VecDeque::new(),
                    sequence: 0,
                    dropped: 0,
                }),
            }),
            version,
            capabilities,
            breakpoints: Mutex::new(BTreeMap::new()),
            control: Mutex::new(()),
            aborted: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            attached: AtomicBool::new(false),
            debuggee_pid: Mutex::new(None),
            exception_filters: Mutex::new(Vec::new()),
            event_thread: Mutex::new(None),
        })
    }

    fn library(&self) -> &'static Library {
        self.shared.library
    }

    fn raw(&self) -> ffi::Session {
        self.shared.raw.0
    }

    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::Acquire)
    }

    pub fn diagnostics(&self) -> String {
        String::new()
    }

    fn ensure_open(&self) -> Result<()> {
        if self.is_closed() {
            return Err(closed_error());
        }
        Ok(())
    }

    fn ensure_attached(&self) -> Result<RawState> {
        self.ensure_open()?;
        let state = self.shared.state();
        match session_state(state.state) {
            SessionState::Attached | SessionState::Running | SessionState::Stopped => Ok(state),
            other => Err(DebuggerError::new(
                "not_attached",
                format!("The debugger session is {}.", state_name(other)),
            )),
        }
    }

    fn ensure_stopped(&self) -> Result<RawState> {
        let state = self.ensure_attached()?;
        if state.state != ffi::STATE_STOPPED {
            return Err(DebuggerError::new(
                "not_stopped",
                format!(
                    "The debuggee is {}; pause it or hit a breakpoint before reading paused state.",
                    state_name(session_state(state.state))
                ),
            ));
        }
        Ok(state)
    }

    pub fn initialize(&self) -> Result<InitializeInfo> {
        if self.initialized.swap(true, Ordering::AcqRel) {
            return Err(invalid("The debugger is already initialized."));
        }
        Ok(self.capabilities.clone())
    }

    pub fn capabilities(&self) -> Option<InitializeInfo> {
        Some(self.capabilities.clone())
    }

    pub fn exception_filters(&self) -> Vec<String> {
        self.exception_filters.lock().unwrap().clone()
    }

    pub fn set_exception_filters(&self, filters: &[String]) -> Result<Vec<String>> {
        let _control = self.control.lock().unwrap();
        self.ensure_attached()?;
        let unique: std::collections::BTreeSet<_> = filters.iter().collect();
        if unique.len() != filters.len()
            || filters
                .iter()
                .any(|id| !EXCEPTION_FILTERS.iter().any(|(known, _)| known == id))
        {
            return Err(invalid(
                "Choose distinct exception filter IDs advertised by the debugger.",
            ));
        }
        for (id, _) in EXCEPTION_FILTERS {
            let enabled = filters.iter().any(|filter| filter == id);
            let name = CString::new(id).map_err(|_| invalid("Invalid exception filter."))?;
            // SAFETY: `name` is a valid C string for the duration of the call.
            let code = unsafe {
                (self.library().set_exception_filter)(self.raw(), name.as_ptr(), enabled as c_int)
            };
            if code != ffi::OK {
                return Err(self.shared.rejected("setExceptionBreakpoints"));
            }
        }
        *self.exception_filters.lock().unwrap() = filters.to_vec();
        Ok(filters.to_vec())
    }

    pub fn output(&self, after: u64) -> Vec<DebuggerOutput> {
        if !self.is_closed() {
            self.shared.drain_output();
        }
        let log = self.shared.output.lock().unwrap();
        log.entries
            .iter()
            .filter(|entry| entry.sequence > after)
            .take(40)
            .cloned()
            .collect()
    }

    /// Attaches to `pid`. The debuggee is resumed once LLDB reports the attach
    /// stop, matching `stopOnEntry: false` on the DAP path.
    pub fn attach(&self, pid: u32) -> Result<SessionSnapshot> {
        self.attach_with(Some(pid), || {
            // SAFETY: the session pointer is live.
            unsafe { (self.library().attach)(self.raw(), pid) }
        })
    }

    /// Attaches through a GDB remote stub forwarded to the controller host.
    pub fn attach_loopback(
        &self,
        port: u16,
        program: Option<&str>,
        pid: u32,
    ) -> Result<SessionSnapshot> {
        let url = CString::new(format!("connect://127.0.0.1:{port}"))
            .map_err(|_| invalid("Invalid remote URL."))?;
        let program = program
            .map(CString::new)
            .transpose()
            .map_err(|_| invalid("The program path contains a NUL byte."))?;
        self.attach_with(Some(pid), || {
            // SAFETY: both strings outlive the call; a null program is allowed.
            unsafe {
                (self.library().attach_remote)(
                    self.raw(),
                    url.as_ptr(),
                    program
                        .as_ref()
                        .map_or(std::ptr::null(), |program| program.as_ptr()),
                )
            }
        })
    }

    fn attach_with(
        &self,
        pid: Option<u32>,
        start: impl FnOnce() -> c_int,
    ) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        self.ensure_open()?;
        if self.attached.swap(true, Ordering::AcqRel) {
            return Err(invalid("This session is already attached."));
        }
        if start() != ffi::OK {
            let error = DebuggerError::unavailable(self.shared.last_error());
            self.shutdown();
            return Err(error);
        }
        if self.is_closed() {
            // Aborted while LLDB was still starting the attach.
            // SAFETY: the session pointer is live; abort is idempotent.
            unsafe { (self.library().abort)(self.raw()) };
            return Err(closed_error());
        }
        self.start_events()?;
        let reached = self.shared.wait(ATTACH_TIMEOUT, |state| {
            state.state == ffi::STATE_STOPPED || terminal(state)
        });
        if self.is_closed() {
            return Err(closed_error());
        }
        let Some(state) = reached else {
            self.shutdown();
            return Err(DebuggerError::new(
                "timeout",
                format!(
                    "LLDB did not finish attaching within {} ms.",
                    ATTACH_TIMEOUT.as_millis()
                ),
            ));
        };
        if terminal(&state) {
            let reason = ffi::char_array(&state.description);
            self.shutdown();
            return Err(DebuggerError::unavailable(if reason.is_empty() {
                "The process ended before LLDB finished attaching.".to_owned()
            } else {
                reason
            }));
        }
        *self.debuggee_pid.lock().unwrap() = pid;
        // SAFETY: the session pointer is live.
        if unsafe { (self.library().continue_)(self.raw()) } != ffi::OK {
            let error = DebuggerError::unavailable(format!(
                "LLDB attached but could not resume the process: {}",
                self.shared.last_error()
            ));
            self.shutdown();
            return Err(error);
        }
        self.shared
            .wait(RESUME_WAIT, |state| state.state != ffi::STATE_STOPPED);
        Ok(self.snapshot())
    }

    fn start_events(&self) -> Result<()> {
        let shared = Arc::clone(&self.shared);
        let handle = thread::Builder::new()
            .name("cove.debugger.lldb".into())
            .spawn(move || event_loop(shared))
            .map_err(|error| {
                self.shutdown();
                DebuggerError::new(
                    "adapter_io",
                    format!("Cannot start the LLDB event thread: {error}"),
                )
            })?;
        *self.event_thread.lock().unwrap() = Some(handle);
        Ok(())
    }

    /// Detaches without terminating the debuggee.
    pub fn detach(&self) -> Result<()> {
        let _control = self.control.lock().unwrap();
        if self.is_closed() || terminal(&self.shared.state()) {
            self.shutdown();
            return Ok(());
        }
        // SAFETY: the session pointer is live.
        let code = unsafe { (self.library().detach)(self.raw()) };
        let reason = (code != ffi::OK).then(|| self.shared.last_error());
        self.shutdown();
        match reason {
            None => Ok(()),
            Some(reason) => Err(DebuggerError::new(
                "detach_failed",
                format!("LLDB did not detach cleanly: {reason}"),
            )),
        }
    }

    /// Detaches and stops the event thread. Safe to call repeatedly and from
    /// any thread; waiters are released rather than left blocked.
    pub fn abort(&self) {
        self.shutdown();
    }

    fn shutdown(&self) {
        if self.aborted.swap(true, Ordering::AcqRel) {
            return;
        }
        self.shared.closed.store(true, Ordering::Release);
        // SAFETY: the session pointer is live; abort and wake are idempotent.
        unsafe {
            (self.library().abort)(self.raw());
            (self.library().wake)(self.raw());
        }
        self.shared.notify();
        if let Some(handle) = self.event_thread.lock().unwrap().take() {
            let _ = handle.join();
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let state = self.shared.state();
        let closed = self.is_closed();
        let session = if closed && !terminal(&state) {
            SessionState::Closed
        } else {
            session_state(state.state)
        };
        let events_dropped = self.shared.output.lock().unwrap().dropped;
        SessionSnapshot {
            state: session,
            stop: (session == SessionState::Stopped).then(|| stop_info(&state)),
            pid: self
                .debuggee_pid
                .lock()
                .unwrap()
                .or((state.pid != 0).then_some(state.pid)),
            process_name: optional(ffi::char_array(&state.process_name)),
            adapter_version: optional(self.version.clone()),
            exit_code: (state.has_exit_code != 0).then_some(state.exit_code as i64),
            events_dropped,
            breakpoints: self.breakpoints(),
        }
    }

    fn refresh(&self, spec: &mut BreakpointSpec) {
        let mut verified: c_int = 0;
        let mut line: u32 = 0;
        // SAFETY: both out pointers are valid for the call.
        let code = unsafe {
            (self.library().breakpoint_info)(self.raw(), spec.id, &mut verified, &mut line)
        };
        if code == ffi::OK {
            spec.verified = verified != 0;
            if line != 0 {
                spec.resolved_line = Some(line);
            }
        }
    }

    fn listed(path: &str, spec: &BreakpointSpec) -> Breakpoint {
        Breakpoint {
            path: path.to_owned(),
            line: spec.line,
            resolved_line: spec.resolved_line,
            condition: spec.condition.clone(),
            log_message: spec.log_message.clone(),
            id: Some(spec.id),
            verified: spec.verified,
            message: None,
        }
    }

    fn list(&self, registry: &mut Registry, only: Option<&str>) -> Vec<Breakpoint> {
        let live = !self.is_closed();
        let mut breakpoints = Vec::new();
        for (path, lines) in registry.iter_mut() {
            if only.is_some_and(|only| only != path) {
                continue;
            }
            for spec in lines.values_mut() {
                if live {
                    self.refresh(spec);
                }
                breakpoints.push(Self::listed(path, spec));
            }
        }
        breakpoints
    }

    pub fn breakpoints(&self) -> Vec<Breakpoint> {
        let mut registry = self.breakpoints.lock().unwrap();
        self.list(&mut registry, None)
    }

    fn delete(&self, id: u64) -> Result<()> {
        // SAFETY: the session pointer is live.
        if unsafe { (self.library().breakpoint_delete)(self.raw(), id) } != ffi::OK {
            return Err(self.shared.rejected("setBreakpoints"));
        }
        Ok(())
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
        let path = normalized_source_path(path)?;
        let c_path = CString::new(path.as_str())
            .map_err(|_| invalid("The source path contains a NUL byte."))?;
        let c_condition = condition
            .map(CString::new)
            .transpose()
            .map_err(|_| invalid("The condition contains a NUL byte."))?;
        let c_log = log_message
            .map(CString::new)
            .transpose()
            .map_err(|_| invalid("The log message contains a NUL byte."))?;
        let _control = self.control.lock().unwrap();
        let mut registry = self.breakpoints.lock().unwrap();
        let mut id: u64 = 0;
        let mut verified: c_int = 0;
        let mut resolved: u32 = 0;
        // SAFETY: every pointer is valid for the call; null strings are allowed.
        let code = unsafe {
            (self.library().breakpoint_create)(
                self.raw(),
                c_path.as_ptr(),
                line,
                c_condition
                    .as_ref()
                    .map_or(std::ptr::null(), |text| text.as_ptr()),
                c_log
                    .as_ref()
                    .map_or(std::ptr::null(), |text| text.as_ptr()),
                &mut id,
                &mut verified,
                &mut resolved,
            )
        };
        if code != ffi::OK {
            return Err(self.shared.rejected("setBreakpoints"));
        }
        let previous = registry.entry(path.clone()).or_default().insert(
            line,
            BreakpointSpec {
                line,
                condition: condition.map(str::to_owned),
                log_message: log_message.map(str::to_owned),
                id,
                verified: verified != 0,
                resolved_line: (resolved != 0).then_some(resolved),
            },
        );
        if let Some(previous) = previous {
            let _ = self.delete(previous.id);
        }
        Ok(self.list(&mut registry, Some(&path)))
    }

    /// Clears one line, or every breakpoint in the source when `line` is None.
    pub fn clear_breakpoints(&self, path: &str, line: Option<u32>) -> Result<Vec<Breakpoint>> {
        self.ensure_attached()?;
        let path = normalized_source_path(path)?;
        let _control = self.control.lock().unwrap();
        let mut registry = self.breakpoints.lock().unwrap();
        let removed: Vec<BreakpointSpec> = match (registry.get_mut(&path), line) {
            (Some(lines), Some(line)) => lines.remove(&line).into_iter().collect(),
            (Some(lines), None) => std::mem::take(lines).into_values().collect(),
            (None, _) => Vec::new(),
        };
        if registry.get(&path).is_some_and(BTreeMap::is_empty) {
            registry.remove(&path);
        }
        let mut failure = None;
        for spec in removed {
            if let Err(error) = self.delete(spec.id) {
                failure.get_or_insert(error);
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(self.list(&mut registry, Some(&path))),
        }
    }

    pub fn threads(&self) -> Result<Vec<ThreadInfo>> {
        self.ensure_attached()?;
        // SAFETY: zeroed RawThread values are valid and the shim writes at most
        // `MAX_THREADS` entries.
        let mut buffer: Vec<RawThread> = vec![unsafe { std::mem::zeroed() }; MAX_THREADS];
        let mut count: c_int = 0;
        // SAFETY: `buffer` holds `MAX_THREADS` entries and outlives the call.
        let code = unsafe {
            (self.library().threads)(
                self.raw(),
                buffer.as_mut_ptr(),
                MAX_THREADS as c_int,
                &mut count,
            )
        };
        if code != ffi::OK {
            return Err(self.shared.rejected("threads"));
        }
        Ok(buffer[..count.max(0) as usize]
            .iter()
            .map(|thread| ThreadInfo {
                id: thread.id,
                name: ffi::char_array(&thread.name),
            })
            .collect())
    }

    pub fn stack_trace(
        &self,
        thread_id: Option<u64>,
        start_frame: u64,
        levels: u64,
    ) -> Result<StackTrace> {
        let state = self.ensure_stopped()?;
        let thread_id = thread_id.unwrap_or(state.thread_id);
        let wanted = if levels == 0 {
            MAX_FRAMES
        } else {
            levels.min(MAX_FRAMES)
        };
        // SAFETY: zeroed RawFrame values are valid and the shim writes at most
        // `wanted` entries.
        let mut buffer: Vec<RawFrame> = vec![unsafe { std::mem::zeroed() }; wanted as usize];
        let mut total: u64 = 0;
        // SAFETY: `buffer` holds `wanted` entries and outlives the call.
        let code = unsafe {
            (self.library().stack)(
                self.raw(),
                thread_id,
                start_frame,
                wanted,
                buffer.as_mut_ptr(),
                wanted as c_int,
                &mut total,
            )
        };
        if code != ffi::OK {
            return Err(self.shared.rejected("stackTrace"));
        }
        let frames: Vec<StackFrame> = buffer
            .iter()
            .take_while(|frame| frame.id != 0)
            .map(|frame| {
                let path = ffi::char_array(&frame.path);
                let source = (!path.is_empty()).then(|| SourceInfo {
                    name: Path::new(&path)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned()),
                    path: Some(path),
                    source_reference: None,
                });
                StackFrame {
                    id: frame.id,
                    name: ffi::char_array(&frame.name),
                    source,
                    line: (frame.line != 0).then_some(frame.line),
                    column: (frame.column != 0).then_some(frame.column),
                    instruction_pointer: optional(ffi::char_array(&frame.pc)),
                }
            })
            .collect();
        let truncated = total > start_frame + frames.len() as u64;
        Ok(StackTrace {
            thread_id,
            frames,
            total_frames: Some(total),
            start_frame,
            truncated,
        })
    }

    pub fn scopes(&self, frame_id: u64) -> Result<Vec<Scope>> {
        self.ensure_stopped()?;
        // SAFETY: zeroed RawScope values are valid and the shim writes at most 8.
        let mut buffer: [RawScope; 8] = unsafe { std::mem::zeroed() };
        let mut count: c_int = 0;
        // SAFETY: `buffer` holds 8 entries and outlives the call.
        let code = unsafe {
            (self.library().scopes)(self.raw(), frame_id, buffer.as_mut_ptr(), 8, &mut count)
        };
        if code != ffi::OK {
            return Err(self.shared.rejected("scopes"));
        }
        Ok(buffer[..count.max(0) as usize]
            .iter()
            .map(|scope| {
                let name = ffi::char_array(&scope.name);
                Scope {
                    presentation_hint: match name.as_str() {
                        "Arguments" => Some("arguments".to_owned()),
                        "Locals" => Some("locals".to_owned()),
                        "Registers" => Some("registers".to_owned()),
                        _ => None,
                    },
                    name,
                    variables_reference: scope.reference,
                    expensive: scope.expensive != 0,
                    source: None,
                    line: None,
                }
            })
            .collect())
    }

    pub fn variables(
        &self,
        variables_reference: u64,
        start: u64,
        count: u64,
        _filter: Option<&str>,
    ) -> Result<VariablePage> {
        if variables_reference == 0 {
            return Err(invalid(
                "A variables reference of 0 has no children; read a scope reference instead.",
            ));
        }
        self.ensure_stopped()?;
        let wanted = if count == 0 {
            MAX_VARIABLES
        } else {
            count.min(MAX_VARIABLES)
        };
        // SAFETY: zeroed RawVariable values are valid and the shim writes at
        // most `wanted` entries.
        let mut buffer: Vec<RawVariable> = vec![unsafe { std::mem::zeroed() }; wanted as usize];
        let mut total: u64 = 0;
        // SAFETY: `buffer` holds `wanted` entries and outlives the call.
        let code = unsafe {
            (self.library().variables)(
                self.raw(),
                variables_reference,
                start,
                wanted,
                buffer.as_mut_ptr(),
                wanted as c_int,
                &mut total,
            )
        };
        if code != ffi::OK {
            return Err(self.shared.rejected("variables"));
        }
        let returned = (total.saturating_sub(start)).min(wanted) as usize;
        let variables: Vec<Variable> = buffer[..returned].iter().map(variable).collect();
        Ok(VariablePage {
            truncated: start + (returned as u64) < total,
            variables,
            start,
            count,
            total: Some(total),
        })
    }

    pub fn evaluate(
        &self,
        expression: &str,
        frame_id: Option<u64>,
        _context: Option<&str>,
    ) -> Result<EvaluateResult> {
        if expression.is_empty() {
            return Err(invalid("An evaluate expression must not be empty."));
        }
        self.ensure_stopped()?;
        let expression =
            CString::new(expression).map_err(|_| invalid("The expression contains a NUL byte."))?;
        // SAFETY: a zeroed RawVariable is valid; the shim fills it on success.
        let mut raw: RawVariable = unsafe { std::mem::zeroed() };
        // SAFETY: `expression` and `raw` outlive the call.
        let code = unsafe {
            (self.library().evaluate)(
                self.raw(),
                expression.as_ptr(),
                frame_id.unwrap_or(0),
                EVALUATE_TIMEOUT.as_millis() as u32,
                &mut raw,
            )
        };
        if code != ffi::OK {
            return Err(self.shared.rejected("evaluate"));
        }
        let value = variable(&raw);
        Ok(EvaluateResult {
            result: value.value,
            result_truncated: value.value_truncated,
            r#type: value.r#type,
            variables_reference: value.variables_reference,
            named_variables: None,
            indexed_variables: None,
            memory_reference: value.memory_reference,
        })
    }

    fn wait_for_stop(&self, before: u64, what: &str) -> Result<SessionSnapshot> {
        let reached = self.shared.wait(CONTROL_WAIT, |state| {
            (state.state == ffi::STATE_STOPPED && state.stop_count > before) || terminal(state)
        });
        if self.is_closed() {
            return Err(closed_error());
        }
        if reached.is_none() {
            return Err(DebuggerError::new(
                "timeout",
                format!(
                    "LLDB accepted {what} but reported no stop within {} ms.",
                    CONTROL_WAIT.as_millis()
                ),
            ));
        }
        Ok(self.snapshot())
    }

    /// Pauses a running debuggee; the snapshot says stopped only once LLDB
    /// reported the stop.
    pub fn pause(&self, _thread_id: Option<u64>) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        let state = self.ensure_attached()?;
        if state.state == ffi::STATE_STOPPED {
            return Err(invalid("The debuggee is already stopped."));
        }
        // SAFETY: the session pointer is live.
        if unsafe { (self.library().pause)(self.raw()) } != ffi::OK {
            return Err(self.shared.rejected("pause"));
        }
        self.wait_for_stop(state.stop_count, "pause")
    }

    /// Resumes a stopped debuggee.
    pub fn resume(&self) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        let state = self.ensure_stopped()?;
        // SAFETY: the session pointer is live.
        if unsafe { (self.library().continue_)(self.raw()) } != ffi::OK {
            return Err(self.shared.rejected("continue"));
        }
        self.shared.wait(RESUME_WAIT, |now| {
            now.state != ffi::STATE_STOPPED || now.stop_count > state.stop_count
        });
        Ok(self.snapshot())
    }

    /// Steps a stopped debuggee and waits for the next stop.
    pub fn step(&self, command: Step) -> Result<SessionSnapshot> {
        let _control = self.control.lock().unwrap();
        let state = self.ensure_stopped()?;
        let (kind, name) = match command {
            Step::Over => (ffi::STEP_OVER, "next"),
            Step::In => (ffi::STEP_IN, "stepIn"),
            Step::Out => (ffi::STEP_OUT, "stepOut"),
        };
        // SAFETY: the session pointer is live.
        if unsafe { (self.library().step)(self.raw(), state.thread_id, kind) } != ffi::OK {
            return Err(self.shared.rejected(name));
        }
        self.wait_for_stop(state.stop_count, name)
    }
}

impl std::fmt::Debug for NativeSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeSession")
            .field("version", &self.version)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl Drop for NativeSession {
    fn drop(&mut self) {
        self.shutdown();
        // SAFETY: the event thread has ended and no other reference remains.
        unsafe { (self.library().session_destroy)(self.raw()) };
    }
}
