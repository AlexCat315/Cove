//! Real in-process LLDB against a disposable child process.
//!
//! Skipped (with a note) when liblldb cannot be loaded on this host.
use crate::backend::{BackendKind, DebugSession, NativeBackend, normalized_source_path};
use crate::model::SessionState;
use crate::native::NativeSession;
use crate::session::Step;
use std::process::{Child, Command, Stdio};

struct Disposable(Child);

impl Drop for Disposable {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn sleeper() -> Disposable {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("ping");
        command.args(["-n", "120", "127.0.0.1"]);
        command
    } else {
        let mut command = Command::new("sleep");
        command.arg("120");
        command
    };
    Disposable(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the disposable debuggee"),
    )
}

fn native() -> Option<NativeSession> {
    match NativeBackend::discover() {
        Ok(backend) => {
            assert!(!backend.version.is_empty());
            Some(NativeSession::create().expect("liblldb loaded but no debugger was created"))
        }
        Err(reason) => {
            eprintln!("native LLDB is unavailable; skipping: {reason}");
            None
        }
    }
}

#[test]
fn attach_pause_inspect_step_and_detach_leave_the_debuggee_alive() {
    let Some(session) = native() else { return };
    let mut target = sleeper();
    let pid = target.0.id();
    let info = session.initialize().unwrap();
    assert!(info.supports_log_points);
    assert!(
        info.exception_filters
            .iter()
            .any(|filter| filter.id == "rust_panic")
    );

    let attached = match session.attach(pid) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            // A host that forbids attach (ptrace scope, task_for_pid) says so.
            assert_eq!(error.code(), "unavailable", "{}", error.message());
            assert!(session.is_closed());
            eprintln!("attach was refused; skipping: {}", error.message());
            return;
        }
    };
    assert_eq!(attached.pid, Some(pid));
    assert_ne!(
        attached.state,
        SessionState::Stopped,
        "attach resumes the debuggee"
    );
    assert!(attached.adapter_version.is_some());
    assert_eq!(
        session.stack_trace(None, 0, 8).unwrap_err().code(),
        "not_stopped"
    );

    let stopped = session.pause(None).unwrap();
    assert_eq!(stopped.state, SessionState::Stopped);
    let stop = stopped.stop.expect("a stop carries its reason");
    assert!(stop.all_threads_stopped);
    assert_eq!(session.pause(None).unwrap_err().code(), "invalid_request");

    let threads = session.threads().unwrap();
    assert!(!threads.is_empty());
    let stack = session.stack_trace(None, 0, 8).unwrap();
    assert!(!stack.frames.is_empty());
    assert!(stack.total_frames.unwrap() >= stack.frames.len() as u64);
    let frame = &stack.frames[0];
    assert!(
        frame
            .instruction_pointer
            .as_deref()
            .unwrap()
            .starts_with("0x")
    );

    let scopes = session.scopes(frame.id).unwrap();
    let registers = scopes
        .iter()
        .find(|scope| scope.name == "Registers")
        .expect("every frame has registers");
    assert!(registers.expensive);
    let sets = session
        .variables(registers.variables_reference, 0, 4, None)
        .unwrap();
    assert!(!sets.variables.is_empty());
    let set = &sets.variables[0];
    assert_ne!(set.variables_reference, 0);
    let page = session
        .variables(set.variables_reference, 0, 2, None)
        .unwrap();
    assert_eq!(page.variables.len(), 2);
    assert!(page.total.unwrap() > 2);
    assert!(page.truncated);

    let error = session
        .evaluate("cove_no_such_symbol + 1", Some(frame.id), None)
        .unwrap_err();
    assert_eq!(error.code(), "adapter_error");
    assert!(error.message().starts_with("LLDB rejected evaluate"));

    let stepped = session.step(Step::Over).unwrap();
    assert!(matches!(
        stepped.state,
        SessionState::Stopped | SessionState::Exited
    ));
    if stepped.state == SessionState::Stopped {
        // Handles from the previous stop are gone, not silently reused.
        assert!(
            session
                .variables(set.variables_reference, 0, 2, None)
                .is_err()
        );
        let resumed = session.resume().unwrap();
        assert_ne!(resumed.state, SessionState::Stopped);
    }

    session.detach().unwrap();
    assert!(session.is_closed());
    assert_eq!(session.snapshot().state, SessionState::Closed);
    assert_eq!(session.threads().unwrap_err().code(), "closed");
    assert!(
        target.0.try_wait().unwrap().is_none(),
        "detach must not kill the debuggee"
    );
}

#[test]
fn breakpoints_and_exception_filters_are_tracked_per_session() {
    let Some(session) = native() else { return };
    let target = sleeper();
    if session.attach(target.0.id()).is_err() {
        return;
    }
    let source = std::env::temp_dir().join("cove-debugger-no-such-source.rs");
    let source = source.to_string_lossy().into_owned();
    let listed = session
        .set_source_breakpoint(&source, 7, Some("x > 1"), Some("x = {x}"))
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].verified, "no module has this source");
    assert!(listed[0].id.is_some());
    assert_eq!(listed[0].log_message.as_deref(), Some("x = {x}"));

    // Replacing the same line keeps one entry with a new LLDB breakpoint.
    let replaced = session
        .set_source_breakpoint(&source, 7, None, None)
        .unwrap();
    assert_eq!(replaced.len(), 1);
    assert_ne!(replaced[0].id, listed[0].id);
    assert_eq!(session.snapshot().breakpoints.len(), 1);
    assert!(
        session
            .clear_breakpoints(&source, Some(7))
            .unwrap()
            .is_empty()
    );
    assert!(session.breakpoints().is_empty());

    let enabled = session
        .set_exception_filters(&["rust_panic".to_owned()])
        .unwrap();
    assert_eq!(enabled, ["rust_panic"]);
    assert_eq!(session.exception_filters(), ["rust_panic"]);
    assert!(session.set_exception_filters(&[]).unwrap().is_empty());
    let error = session
        .set_exception_filters(&["nope".to_owned()])
        .unwrap_err();
    assert_eq!(error.code(), "invalid_request");
    session.detach().unwrap();
}

#[test]
fn a_missing_process_is_refused_and_closes_the_session() {
    let Some(session) = native() else { return };
    let error = session.attach(u32::MAX - 7).unwrap_err();
    assert!(
        matches!(error.code(), "unavailable" | "timeout"),
        "{}",
        error.message()
    );
    assert!(session.is_closed());
    assert_eq!(session.snapshot().state, SessionState::Closed);
    session.abort();
}

#[test]
fn abort_releases_a_session_from_another_thread() {
    let Some(session) = native() else { return };
    let target = sleeper();
    if session.attach(target.0.id()).is_err() {
        return;
    }
    let session = std::sync::Arc::new(DebugSession::from(session));
    assert_eq!(session.backend(), BackendKind::Native);
    let other = std::sync::Arc::clone(&session);
    std::thread::spawn(move || other.abort()).join().unwrap();
    assert!(session.is_closed());
    assert_eq!(session.pause(None).unwrap_err().code(), "closed");
}

#[test]
fn source_paths_lose_the_windows_verbatim_prefix() {
    let directory = std::env::temp_dir();
    let path = normalized_source_path(&directory.to_string_lossy()).unwrap();
    assert!(!path.starts_with(r"\\?\"), "{path}");
    assert_eq!(
        normalized_source_path("relative.rs").unwrap_err().code(),
        "invalid_request"
    );
}

const TICK_LINE: u32 = line!() + 4;

#[inline(never)]
fn tick(counter: u64) -> u64 {
    let doubled = counter.wrapping_mul(2);
    std::hint::black_box(doubled)
}

/// The debuggee for the source-level test: this test binary re-run with
/// `COVE_DEBUGGEE` set. Without the variable it returns at once.
#[test]
#[ignore = "debuggee fixture, spawned by the source-level test"]
fn debuggee_fixture() {
    if std::env::var_os("COVE_DEBUGGEE").is_none() {
        return;
    }
    let started = std::time::Instant::now();
    let mut counter = 0u64;
    while started.elapsed() < std::time::Duration::from_secs(90) {
        counter = counter.wrapping_add(1);
        std::hint::black_box(tick(counter));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn wait_for_stop(session: &NativeSession) -> crate::model::SessionSnapshot {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let snapshot = session.snapshot();
        if snapshot.state == SessionState::Stopped || std::time::Instant::now() >= deadline {
            return snapshot;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_source_breakpoint_stops_with_real_locals_and_a_log_point_reports_values() {
    let Some(session) = native() else { return };
    let target = Disposable(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::native::debuggee_fixture",
                "--ignored",
                "--test-threads=1",
            ])
            .env("COVE_DEBUGGEE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    if let Err(error) = session.attach(target.0.id()) {
        eprintln!("attach was refused; skipping: {}", error.message());
        return;
    }
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/native.rs");

    let listed = session
        .set_source_breakpoint(source, TICK_LINE, None, None)
        .unwrap();
    assert!(
        listed[0].verified,
        "the line table resolves {source}:{TICK_LINE}"
    );
    let id = listed[0].id.unwrap();

    let stopped = wait_for_stop(&session);
    assert_eq!(stopped.state, SessionState::Stopped);
    let stop = stopped.stop.unwrap();
    assert_eq!(stop.reason, "breakpoint");
    assert_eq!(stop.hit_breakpoint_ids, [id]);

    let stack = session.stack_trace(None, 0, 4).unwrap();
    let frame = &stack.frames[0];
    assert!(frame.name.contains("tick"), "{}", frame.name);
    assert_eq!(frame.line, Some(TICK_LINE));
    let path = frame.source.as_ref().unwrap().path.as_deref().unwrap();
    assert!(path.ends_with("native.rs"), "{path}");

    let scopes = session.scopes(frame.id).unwrap();
    let mut visible = Vec::new();
    for scope in scopes.iter().filter(|scope| !scope.expensive) {
        visible.extend(
            session
                .variables(scope.variables_reference, 0, 20, None)
                .unwrap()
                .variables,
        );
    }
    let counter = visible
        .iter()
        .find(|variable| variable.name == "counter")
        .expect("the argument is visible in the frame");
    let first: u64 = counter.value.parse().expect("an integer value");
    assert!(first >= 1);
    let evaluated = session.evaluate("counter", Some(frame.id), None).unwrap();
    assert_eq!(evaluated.result, counter.value);

    // A log point reports the value and never leaves the debuggee stopped.
    session.clear_breakpoints(source, None).unwrap();
    session
        .set_source_breakpoint(source, TICK_LINE, None, Some("counter={counter}"))
        .unwrap();
    assert_ne!(session.resume().unwrap().state, SessionState::Stopped);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut logged = Vec::new();
    while logged.len() < 2 && std::time::Instant::now() < deadline {
        logged = session.output(0);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(logged.len() >= 2, "log point output arrived");
    let value: u64 = logged[0]
        .text
        .strip_prefix("counter=")
        .and_then(|text| text.parse().ok())
        .unwrap_or_else(|| panic!("unexpected log text {:?}", logged[0].text));
    assert!(value > first);
    assert_ne!(session.snapshot().state, SessionState::Stopped);
    assert!(session.output(logged.last().unwrap().sequence).len() <= 40);

    session.detach().unwrap();
}
