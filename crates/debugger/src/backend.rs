//! Backend selection: in-process liblldb first, `lldb-dap` as the fallback.
use crate::adapter::Availability;
use crate::error::{DebuggerError, Result};
use crate::ffi;
use crate::model::{
    Breakpoint, DebuggerOutput, EvaluateResult, InitializeInfo, Scope, SessionSnapshot,
    SessionState, StackTrace, ThreadInfo, VariablePage,
};
use crate::native::NativeSession;
use crate::session::{DapSession, Step};
use serde::Serialize;
use std::ffi::CStr;
use std::path::Path;

pub(crate) fn state_name(state: SessionState) -> &'static str {
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

/// Canonicalizes an absolute source path into the form debug info records.
pub(crate) fn normalized_source_path(path: &str) -> Result<String> {
    let native = Path::new(path).is_absolute();
    // A rooted POSIX path is absolute for a remote target's debug info even
    // when the controller runs on Windows.
    if path.is_empty() || !(native || path.starts_with('/')) {
        return Err(DebuggerError::new(
            "invalid_request",
            "Use an absolute source path.",
        ));
    }
    if !native {
        return Ok(path.to_owned());
    }
    let resolved = std::fs::canonicalize(path)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned());
    // Windows canonical paths carry a verbatim prefix that no line table uses.
    Ok(match resolved.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => resolved
            .strip_prefix(r"\\?\")
            .map(str::to_owned)
            .unwrap_or(resolved),
    })
}

/// Which backend a session runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Native,
    Dap,
}

/// The in-process liblldb the controller can load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NativeBackend {
    pub version: String,
    pub library: Option<String>,
}

impl NativeBackend {
    /// Loads the shim and liblldb and reads the LLDB version.
    pub fn discover() -> std::result::Result<Self, String> {
        if std::env::var_os("COVE_DEBUGGER_BACKEND").is_some_and(|value| value == "dap") {
            return Err("COVE_DEBUGGER_BACKEND=dap disables the native LLDB backend.".to_owned());
        }
        let library = ffi::library().ok_or_else(|| {
            ffi::unavailable_reason()
                .unwrap_or("The native LLDB backend could not be loaded.")
                .to_owned()
        })?;
        // SAFETY: the version string is static in the shim or liblldb.
        let version = unsafe {
            let pointer = (library.version)();
            if pointer.is_null() {
                String::new()
            } else {
                CStr::from_ptr(pointer).to_string_lossy().into_owned()
            }
        };
        if version == "unavailable" {
            // SAFETY: the stub returns a static explanation.
            return Err(unsafe { ffi::last_error(library) });
        }
        Ok(Self {
            version,
            library: ffi::runtime_path().map(|path| path.to_string_lossy().into_owned()),
        })
    }
}

/// A live debugger session on either backend.
#[derive(Debug)]
pub enum DebugSession {
    Native(NativeSession),
    Dap(DapSession),
}

impl From<DapSession> for DebugSession {
    fn from(session: DapSession) -> Self {
        Self::Dap(session)
    }
}

impl From<NativeSession> for DebugSession {
    fn from(session: NativeSession) -> Self {
        Self::Native(session)
    }
}

macro_rules! delegate {
    ($self:ident, $session:ident => $call:expr) => {
        match $self {
            DebugSession::Native($session) => $call,
            DebugSession::Dap($session) => $call,
        }
    };
}

impl DebugSession {
    /// Opens a session on the preferred available backend.
    ///
    /// The native backend is used when liblldb loads; a failure to start it
    /// falls back to `lldb-dap` when one was found.
    pub fn open(availability: &Availability) -> Result<Self> {
        let native_error = if availability.native.is_some() {
            match NativeSession::create() {
                Ok(session) => return Ok(Self::Native(session)),
                Err(error) => Some(error),
            }
        } else {
            None
        };
        match (&availability.adapter, native_error) {
            (Some(adapter), _) => Ok(Self::Dap(DapSession::spawn(adapter)?)),
            (None, Some(error)) => Err(error),
            (None, None) => Err(DebuggerError::unavailable(
                availability
                    .reason
                    .clone()
                    .unwrap_or_else(|| "No debugger backend is available.".to_owned()),
            )),
        }
    }

    pub fn backend(&self) -> BackendKind {
        match self {
            Self::Native(_) => BackendKind::Native,
            Self::Dap(_) => BackendKind::Dap,
        }
    }

    pub fn is_closed(&self) -> bool {
        delegate!(self, session => session.is_closed())
    }

    pub fn diagnostics(&self) -> String {
        delegate!(self, session => session.diagnostics())
    }

    pub fn initialize(&self) -> Result<InitializeInfo> {
        delegate!(self, session => session.initialize())
    }

    pub fn capabilities(&self) -> Option<InitializeInfo> {
        delegate!(self, session => session.capabilities())
    }

    pub fn exception_filters(&self) -> Vec<String> {
        delegate!(self, session => session.exception_filters())
    }

    pub fn set_exception_filters(&self, filters: &[String]) -> Result<Vec<String>> {
        delegate!(self, session => session.set_exception_filters(filters))
    }

    pub fn output(&self, after: u64) -> Vec<DebuggerOutput> {
        delegate!(self, session => session.output(after))
    }

    pub fn attach(&self, pid: u32) -> Result<SessionSnapshot> {
        delegate!(self, session => session.attach(pid))
    }

    pub fn attach_loopback(
        &self,
        port: u16,
        program: Option<&str>,
        pid: u32,
    ) -> Result<SessionSnapshot> {
        delegate!(self, session => session.attach_loopback(port, program, pid))
    }

    pub fn detach(&self) -> Result<()> {
        delegate!(self, session => session.detach())
    }

    pub fn abort(&self) {
        delegate!(self, session => session.abort())
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        delegate!(self, session => session.snapshot())
    }

    pub fn breakpoints(&self) -> Vec<Breakpoint> {
        delegate!(self, session => session.breakpoints())
    }

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
        delegate!(self, session => session.set_source_breakpoint(path, line, condition, log_message))
    }

    pub fn clear_breakpoints(&self, path: &str, line: Option<u32>) -> Result<Vec<Breakpoint>> {
        delegate!(self, session => session.clear_breakpoints(path, line))
    }

    pub fn threads(&self) -> Result<Vec<ThreadInfo>> {
        delegate!(self, session => session.threads())
    }

    pub fn stack_trace(
        &self,
        thread_id: Option<u64>,
        start_frame: u64,
        levels: u64,
    ) -> Result<StackTrace> {
        delegate!(self, session => session.stack_trace(thread_id, start_frame, levels))
    }

    pub fn scopes(&self, frame_id: u64) -> Result<Vec<Scope>> {
        delegate!(self, session => session.scopes(frame_id))
    }

    pub fn variables(
        &self,
        variables_reference: u64,
        start: u64,
        count: u64,
        filter: Option<&str>,
    ) -> Result<VariablePage> {
        delegate!(self, session => session.variables(variables_reference, start, count, filter))
    }

    pub fn evaluate(
        &self,
        expression: &str,
        frame_id: Option<u64>,
        context: Option<&str>,
    ) -> Result<EvaluateResult> {
        delegate!(self, session => session.evaluate(expression, frame_id, context))
    }

    pub fn pause(&self, thread_id: Option<u64>) -> Result<SessionSnapshot> {
        delegate!(self, session => session.pause(thread_id))
    }

    pub fn resume(&self) -> Result<SessionSnapshot> {
        delegate!(self, session => session.resume())
    }

    pub fn step(&self, command: Step) -> Result<SessionSnapshot> {
        delegate!(self, session => session.step(command))
    }
}
