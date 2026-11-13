//! Controller-level debugger sessions keyed by runtime process ID.
use cove_debugger::{
    Adapter, Availability, DapSession, DebuggerError, SessionSnapshot, SessionState, Step,
};
use cove_devtools_protocol::control::Fault;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_ATTACH_TIMEOUT: Duration = Duration::from_secs(8);
pub(crate) const MIN_ATTACH_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const MAX_ATTACH_TIMEOUT: Duration = Duration::from_secs(30);
const ABORT_WAIT: Duration = Duration::from_secs(2);

type Connector = dyn Fn() -> Result<DapSession, DebuggerError> + Send + Sync;

/// Converts a debugger failure for the controller's JSON-RPC error shape.
pub(crate) fn fault(error: DebuggerError) -> Fault {
    Fault::new(error.code(), error.message())
}

fn now_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

/// A truthful warning for a denied task_for_pid attach.
fn developer_hint(developer_mode: Option<bool>) -> Option<&'static str> {
    (developer_mode == Some(false)).then_some(
        "Developer mode is disabled, so macOS can deny task_for_pid attach. Enable it with `sudo DevToolsSecurity -enable` or sign the target with get-task-allow.",
    )
}

fn decorate(mut error: DebuggerError, developer_mode: Option<bool>) -> DebuggerError {
    if let Some(hint) = developer_hint(developer_mode)
        && !error.message().contains("Developer mode")
    {
        error = DebuggerError::new(error.code(), format!("{} {hint}", error.message()));
    }
    error
}

fn live(session: &DapSession) -> bool {
    !session.is_closed()
        && !matches!(
            session.snapshot().state,
            SessionState::Exited | SessionState::Terminated | SessionState::Closed
        )
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

enum Phase {
    Attaching { session: Option<Arc<DapSession>> },
    Attached(Arc<DapSession>),
    Failed { error: DebuggerError },
}

struct SessionHandle {
    pid: u32,
    target: String,
    started: Instant,
    phase: Mutex<Phase>,
    changed: Condvar,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SessionKey {
    Local(u32),
    Remote(String),
}

impl From<u32> for SessionKey {
    fn from(pid: u32) -> Self {
        Self::Local(pid)
    }
}

pub(crate) struct ForwardedDebugger {
    pub port: u16,
    pub program: Option<String>,
}

impl SessionHandle {
    fn new(pid: u32, target: String) -> Self {
        Self {
            pid,
            target,
            started: Instant::now(),
            phase: Mutex::new(Phase::Attaching { session: None }),
            changed: Condvar::new(),
        }
    }

    fn summary(&self) -> Value {
        let mut value = {
            let phase = self.phase.lock().unwrap();
            match &*phase {
                Phase::Attaching { .. } => json!({
                    "pid": self.pid,
                    "target": self.target,
                    "state": "attaching",
                    "stop": Value::Null,
                    "last_error": Value::Null,
                }),
                Phase::Failed { error, .. } => json!({
                    "pid": self.pid,
                    "target": self.target,
                    "state": "failed",
                    "stop": Value::Null,
                    "last_error": {"code": error.code(), "message": error.message()},
                }),
                Phase::Attached(session) => {
                    let snapshot = session.snapshot();
                    json!({
                        "pid": self.pid,
                        "target": self.target,
                        "state": state_name(snapshot.state),
                        "stop": snapshot.stop,
                        "last_error": Value::Null,
                        "adapter_version": snapshot.adapter_version,
                        "breakpoints": snapshot.breakpoints,
                        "capabilities": session.capabilities(),
                        "exception_filters": session.exception_filters(),
                        "events_dropped": snapshot.events_dropped,
                    })
                }
            }
        };
        value["since_ms"] = json!(now_ms(self.started));
        value
    }
}

/// Owns sessions, async attach work and adapter availability.
pub(crate) struct DebuggerManager {
    availability: OnceLock<Availability>,
    connector: Option<Arc<Connector>>,
    sessions: Mutex<HashMap<SessionKey, Arc<SessionHandle>>>,
}

impl DebuggerManager {
    pub(crate) fn new() -> Self {
        Self {
            availability: OnceLock::new(),
            connector: None,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_source(availability: Availability, connector: Arc<Connector>) -> Self {
        let manager = Self {
            availability: OnceLock::new(),
            connector: Some(connector),
            sessions: Mutex::new(HashMap::new()),
        };
        let _ = manager.availability.set(availability);
        manager
    }

    pub(crate) fn availability(&self) -> &Availability {
        self.availability.get_or_init(Availability::discover)
    }

    pub(crate) fn availability_json(&self) -> Value {
        let availability = self.availability();
        let mut value = serde_json::to_value(availability).unwrap_or_else(|_| json!({}));
        value["available"] = json!(availability.is_available());
        value["warning"] = json!(availability.warning());
        value["sessions"] = json!(self.list());
        value
    }

    pub(crate) fn list(&self) -> Vec<Value> {
        let handles: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        let mut values: Vec<_> = handles.iter().map(|handle| handle.summary()).collect();
        values.sort_by_key(|value| value["pid"].as_u64().unwrap_or(0));
        values
    }

    pub(crate) fn summary(&self, key: impl Into<SessionKey>) -> Option<Value> {
        self.sessions
            .lock()
            .unwrap()
            .get(&key.into())
            .map(|handle| handle.summary())
    }

    /// Returns the live session for a PID, or an explicit state error.
    pub(crate) fn session(
        &self,
        key: impl Into<SessionKey>,
    ) -> Result<Arc<DapSession>, DebuggerError> {
        let handle = self
            .sessions
            .lock()
            .unwrap()
            .get(&key.into())
            .cloned()
            .ok_or_else(|| {
                DebuggerError::new(
                    "no_session",
                    "Attach the UI debugger to this runtime first.",
                )
            })?;
        match &*handle.phase.lock().unwrap() {
            Phase::Attached(session) if live(session) => Ok(Arc::clone(session)),
            Phase::Attached(_) => Err(DebuggerError::new(
                "not_attached",
                "The debugger session has closed; attach again.",
            )),
            Phase::Attaching { .. } => Err(DebuggerError::new(
                "attaching",
                "The UI debugger is still attaching; check status with get_ui_debugger.",
            )),
            Phase::Failed { error, .. } => Err(error.clone()),
        }
    }

    /// Starts attaching to `pid` on a worker and waits up to `timeout`.
    ///
    /// A refusal reports the adapter's own reason as `unavailable`; a hung
    /// authorization prompt reports `timeout` after the session is aborted.
    #[cfg(test)]
    pub(crate) fn attach(
        &self,
        target: &str,
        pid: u32,
        timeout: Duration,
    ) -> Result<SessionSnapshot, DebuggerError> {
        self.attach_with(target, pid, SessionKey::Local(pid), timeout, None)
    }

    pub(crate) fn attach_with(
        &self,
        target: &str,
        pid: u32,
        key: SessionKey,
        timeout: Duration,
        remote: Option<ForwardedDebugger>,
    ) -> Result<SessionSnapshot, DebuggerError> {
        let timeout = timeout.clamp(MIN_ATTACH_TIMEOUT, MAX_ATTACH_TIMEOUT);
        let availability = self.availability().clone();
        let developer_mode = if remote.is_some() {
            None
        } else {
            availability.developer_mode
        };
        if self.connector.is_none() && availability.adapter.is_none() {
            return Err(DebuggerError::unavailable(
                availability
                    .reason
                    .clone()
                    .unwrap_or_else(|| "lldb-dap is unavailable.".to_owned()),
            ));
        }
        let started = Instant::now();
        let (handle, creator) = {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(handle) = sessions.get(&key).cloned() {
                let reuse = match &*handle.phase.lock().unwrap() {
                    Phase::Attached(session) if live(session) => {
                        return Ok(session.snapshot());
                    }
                    Phase::Attaching { .. } => true,
                    Phase::Attached(_) | Phase::Failed { .. } => false,
                };
                if reuse {
                    (handle, false)
                } else {
                    let handle = Arc::new(SessionHandle::new(pid, target.to_owned()));
                    sessions.insert(key, Arc::clone(&handle));
                    (handle, true)
                }
            } else {
                let handle = Arc::new(SessionHandle::new(pid, target.to_owned()));
                sessions.insert(key, Arc::clone(&handle));
                (handle, true)
            }
        };
        if creator {
            self.spawn_attach(
                Arc::clone(&handle),
                availability.adapter.clone(),
                developer_mode,
                remote,
            );
        }
        let deadline = started + timeout;
        let mut phase = handle.phase.lock().unwrap();
        loop {
            match &*phase {
                Phase::Attached(session) if live(session) => {
                    return Ok(session.snapshot());
                }
                Phase::Attached(_) => {
                    return Err(DebuggerError::new(
                        "not_attached",
                        "The debugger session closed during attach.",
                    ));
                }
                Phase::Failed { error, .. } => {
                    return Err(decorate(error.clone(), developer_mode));
                }
                Phase::Attaching { .. } => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            phase = handle.changed.wait_timeout(phase, remaining).unwrap().0;
        }
        if !creator {
            return Err(DebuggerError::new(
                "timeout",
                format!(
                    "Another attach for PID {pid} is still pending after {} ms.",
                    timeout.as_millis()
                ),
            ));
        }
        let pending = match &*phase {
            Phase::Attaching { session } => session.clone(),
            _ => None,
        };
        drop(phase);
        if let Some(session) = &pending {
            session.abort();
        }
        let until = Instant::now() + ABORT_WAIT;
        let mut phase = handle.phase.lock().unwrap();
        while matches!(&*phase, Phase::Attaching { .. }) && Instant::now() < until {
            phase = handle
                .changed
                .wait_timeout(phase, Duration::from_millis(20))
                .unwrap()
                .0;
        }
        match &*phase {
            // A real denial that raced the deadline is more useful than the
            // timeout it interrupted; errors caused by our own abort are not.
            Phase::Failed { error, .. } if error.code() == "unavailable" => {
                Err(decorate(error.clone(), developer_mode))
            }
            Phase::Attached(session) if live(session) => Ok(session.snapshot()),
            _ => {
                let diagnostics = pending
                    .as_ref()
                    .map(|session| session.diagnostics())
                    .unwrap_or_default();
                let mut message = format!(
                    "lldb-dap did not finish attaching to PID {pid} within {} ms. An authorization prompt may be waiting.",
                    timeout.as_millis()
                );
                if !diagnostics.is_empty() {
                    message.push_str(&format!(" Adapter stderr: {diagnostics}"));
                }
                if let Some(hint) = developer_hint(developer_mode) {
                    message.push(' ');
                    message.push_str(hint);
                }
                let error = DebuggerError::new("timeout", message);
                *phase = Phase::Failed {
                    error: error.clone(),
                };
                handle.changed.notify_all();
                Err(error)
            }
        }
    }

    fn spawn_attach(
        &self,
        handle: Arc<SessionHandle>,
        adapter: Option<Adapter>,
        developer_mode: Option<bool>,
        remote: Option<ForwardedDebugger>,
    ) {
        let connector = self.connector.clone();
        let pid = handle.pid;
        let worker = Arc::clone(&handle);
        let spawned = std::thread::Builder::new()
            .name("cove.debugger.attach".into())
            .spawn(move || {
                let result = (|| -> Result<Arc<DapSession>, DebuggerError> {
                    let session = match &connector {
                        Some(connector) => connector()?,
                        None => {
                            let adapter = adapter.ok_or_else(|| {
                                DebuggerError::unavailable(
                                    "lldb-dap is unavailable; check get_ui_session debugger availability.",
                                )
                            })?;
                            DapSession::spawn(&adapter)?
                        }
                    };
                    let session = Arc::new(session);
                    {
                        let mut phase = worker.phase.lock().unwrap();
                        match &mut *phase {
                            Phase::Attaching { session: slot } => {
                                *slot = Some(Arc::clone(&session));
                            }
                            _ => {
                                // The controller timed out while the adapter was
                                // starting; do not leave a fresh adapter behind.
                                drop(phase);
                                session.abort();
                                return Err(DebuggerError::new(
                                    "timeout",
                                    "The attach wait ended before the session started.",
                                ));
                            }
                        }
                    }
                    worker.changed.notify_all();
                    session.initialize()?;
                    if let Some(remote) = remote {
                        session.attach_loopback(remote.port, remote.program.as_deref(), pid)?;
                    } else {
                        session.attach(pid)?;
                    }
                    Ok(session)
                })();
                let mut phase = worker.phase.lock().unwrap();
                if matches!(&*phase, Phase::Attaching { .. }) {
                    match result {
                        Ok(session) => {
                            *phase = Phase::Attached(session);
                        }
                        Err(error) => {
                            if let Phase::Attaching {
                                session: Some(session),
                            } = &*phase
                            {
                                session.abort();
                            }
                            *phase = Phase::Failed {
                                error: decorate(error, developer_mode),
                            };
                        }
                    }
                    worker.changed.notify_all();
                } else if let Ok(session) = result {
                    session.abort();
                }
            });
        if let Err(error) = spawned {
            *handle.phase.lock().unwrap() = Phase::Failed {
                error: DebuggerError::new(
                    "adapter_io",
                    format!("Cannot start the debugger attach worker: {error}"),
                ),
            };
            handle.changed.notify_all();
        }
    }

    /// Detaches or aborts one session and forgets it.
    pub(crate) fn detach(&self, key: impl Into<SessionKey>) -> Result<&'static str, DebuggerError> {
        let handle = self.sessions.lock().unwrap().remove(&key.into());
        let Some(handle) = handle else {
            return Err(DebuggerError::new(
                "no_session",
                "No UI debugger session is attached to this runtime.",
            ));
        };
        let phase = handle.phase.lock().unwrap();
        match &*phase {
            Phase::Attached(session) if !live(session) => Ok("closed"),
            Phase::Attached(session) => {
                let session = Arc::clone(session);
                drop(phase);
                session.detach()?;
                Ok("detached")
            }
            Phase::Attaching { session } => {
                let session = session.clone();
                drop(phase);
                *handle.phase.lock().unwrap() = Phase::Failed {
                    error: DebuggerError::new("closed", "The debugger session is closed."),
                };
                handle.changed.notify_all();
                if let Some(session) = session {
                    session.abort();
                }
                Ok("aborted")
            }
            Phase::Failed { .. } => Ok("failed"),
        }
    }
}

fn close_handle(handle: &SessionHandle) {
    let phase = handle.phase.lock().unwrap();
    match &*phase {
        Phase::Attached(session) if !live(session) => {}
        Phase::Attached(session) => {
            let session = Arc::clone(session);
            drop(phase);
            let _ = session.detach();
        }
        Phase::Attaching { session } => {
            let session = session.clone();
            drop(phase);
            *handle.phase.lock().unwrap() = Phase::Failed {
                error: DebuggerError::new("closed", "The debugger session is closed."),
            };
            handle.changed.notify_all();
            if let Some(session) = session {
                session.abort();
            }
        }
        Phase::Failed { .. } => {}
    }
}

impl Default for DebuggerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DebuggerManager {
    fn drop(&mut self) {
        let handles: Vec<_> = self
            .sessions
            .lock()
            .unwrap()
            .drain()
            .map(|(_, handle)| handle)
            .collect();
        for handle in handles {
            close_handle(&handle);
        }
    }
}

/// Control actions exposed by `control_ui_debugger`.
pub(crate) enum ControlAction {
    Pause { thread_id: Option<u64> },
    Resume,
    Step(Step),
}

impl ControlAction {
    pub(crate) fn parse(action: &str, thread_id: Option<u64>) -> Option<Self> {
        match action {
            "pause" => Some(Self::Pause { thread_id }),
            "resume" => Some(Self::Resume),
            "step_over" => Some(Self::Step(Step::Over)),
            "step_in" => Some(Self::Step(Step::In)),
            "step_out" => Some(Self::Step(Step::Out)),
            _ => None,
        }
    }

    pub(crate) fn apply(&self, session: &DapSession) -> Result<SessionSnapshot, DebuggerError> {
        match *self {
            Self::Pause { thread_id } => session.pause(thread_id),
            Self::Resume => session.resume(),
            Self::Step(step) => session.step(step),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_debugger::testing::{MockAdapter, MockOptions, wait_for_state};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn availability() -> Availability {
        Availability {
            adapter: Some(Adapter {
                path: "/mock/lldb-dap".into(),
                version: "mock-lldb".into(),
            }),
            developer_mode: Some(false),
            platform: "macos".into(),
            reason: None,
        }
    }

    fn mock_manager() -> (DebuggerManager, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let manager = DebuggerManager::with_source(
            availability(),
            Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                let (session, _handle) = MockAdapter::pair();
                Ok(session)
            }),
        );
        (manager, calls)
    }

    #[test]
    fn attach_is_keyed_by_pid_and_reuses_the_live_session() {
        let (manager, calls) = mock_manager();
        let snapshot = manager
            .attach("runtime-a", 123, Duration::from_secs(2))
            .unwrap();
        assert_eq!(snapshot.state, SessionState::Stopped);
        assert_eq!(snapshot.pid, Some(123));

        let again = manager
            .attach("runtime-a", 123, Duration::from_secs(2))
            .unwrap();
        assert_eq!(again.state, SessionState::Stopped);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no second adapter spawn");

        let summary = manager.summary(123).unwrap();
        assert_eq!(summary["target"], "runtime-a");
        assert_eq!(summary["state"], "stopped");
        assert!(manager.session(123).is_ok());

        assert_eq!(manager.detach(123).unwrap(), "detached");
        assert_eq!(manager.session(123).unwrap_err().code(), "no_session");
        assert!(manager.list().is_empty());
    }

    #[test]
    fn a_failed_attach_reports_unavailable_and_stays_inspectable() {
        let manager = DebuggerManager::with_source(
            availability(),
            Arc::new(|| {
                let (session, _handle) = MockAdapter::with(
                    MockOptions::default()
                        .fail_attach("attach failed (Not allowed to attach to process.)"),
                );
                Ok(session)
            }),
        );
        let error = manager
            .attach("runtime-a", 9, Duration::from_secs(2))
            .unwrap_err();
        assert_eq!(error.code(), "unavailable");
        assert!(error.message().contains("Not allowed"));
        assert!(error.message().contains("Developer mode is disabled"));
        assert_eq!(manager.summary(9).unwrap()["state"], "failed");
        assert_eq!(manager.session(9).unwrap_err().code(), "unavailable");
        assert_eq!(manager.detach(9).unwrap(), "failed");
    }

    #[test]
    fn a_hung_attach_times_out_and_scopes_its_session() {
        let manager = DebuggerManager::with_source(
            availability(),
            Arc::new(|| {
                let (session, _handle) = MockAdapter::with(MockOptions::default().ignore("attach"));
                Ok(session)
            }),
        );
        let started = Instant::now();
        let error = manager
            .attach("runtime-a", 5, Duration::from_millis(300))
            .unwrap_err();
        assert_eq!(error.code(), "timeout");
        assert!(error.message().contains("PID 5"));
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(manager.summary(5).unwrap()["state"], "failed");
        assert_eq!(manager.detach(5).unwrap(), "failed");
    }

    #[test]
    fn control_actions_require_a_real_stop_event() {
        let (manager, _calls) = mock_manager();
        manager
            .attach("runtime-a", 11, Duration::from_secs(2))
            .unwrap();
        let session = manager.session(11).unwrap();
        let running = ControlAction::parse("resume", None)
            .unwrap()
            .apply(&session)
            .unwrap();
        assert_eq!(running.state, SessionState::Running);
        let error = ControlAction::parse("step_over", None)
            .unwrap()
            .apply(&session)
            .unwrap_err();
        assert_eq!(error.code(), "not_stopped");
        let stopped = ControlAction::parse("pause", None)
            .unwrap()
            .apply(&session)
            .unwrap();
        assert_eq!(stopped.state, SessionState::Stopped);
        assert_eq!(manager.detach(11).unwrap(), "detached");
    }

    #[test]
    fn availability_exposes_the_developer_mode_warning_without_faking_a_session() {
        let manager = DebuggerManager::new();
        let value = manager.availability_json();
        assert!(value.get("available").is_some());
        assert!(value["sessions"].as_array().unwrap().is_empty());
        if value["available"] == true {
            assert!(value["adapter"]["path"].is_string());
            assert!(value["adapter"]["version"].is_string());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_real_attach_through_the_manager_reports_the_authorization_reason() {
        use std::process::{Child, Command};

        struct Disposable(Child);
        impl Drop for Disposable {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let manager = DebuggerManager::new();
        if !manager.availability().is_available() {
            eprintln!("lldb-dap is unavailable; skipping the real attach check");
            return;
        }
        let target = Disposable(Command::new("/bin/sleep").arg("30").spawn().unwrap());
        let pid = target.0.id();
        let result = manager.attach("gallery", pid, Duration::from_secs(5));
        match result {
            Ok(_) => {
                assert_ne!(
                    manager.availability().developer_mode,
                    Some(false),
                    "attach succeeded while Developer mode is disabled"
                );
                manager.detach(pid).expect("detach the disposable target");
            }
            Err(error) => {
                assert_eq!(error.code(), "unavailable", "{}", error.message());
                assert!(
                    error.message().to_lowercase().contains("attach"),
                    "explicit attach reason: {}",
                    error.message()
                );
                if manager.availability().developer_mode == Some(false) {
                    assert!(error.message().contains("Developer mode is disabled"));
                }
            }
        }
        assert_eq!(manager.summary(pid).unwrap()["state"], "failed");
    }

    #[test]
    fn a_mock_session_reaches_stopped_through_the_public_state_helper() {
        let (session, _handle) = MockAdapter::pair();
        session.initialize().unwrap();
        session.attach(1).unwrap();
        wait_for_state(&session, SessionState::Stopped, Duration::from_secs(1)).unwrap();
    }
}
