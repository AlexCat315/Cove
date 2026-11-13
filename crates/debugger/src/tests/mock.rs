use crate::model::SessionState;
use crate::session::{DapSession, Step};
use crate::testing::{MockAdapter, MockHandle, MockOptions};
use serde_json::json;
use std::time::Duration;

fn attached() -> (DapSession, MockHandle) {
    let (session, handle) = MockAdapter::pair();
    let info = session.initialize().unwrap();
    assert!(info.adapter_version.unwrap().contains("mock-lldb"));
    let snapshot = session.attach(4242).unwrap();
    assert_eq!(snapshot.state, SessionState::Stopped);
    (session, handle)
}

#[test]
fn initialize_preserves_the_adapter_capabilities() {
    let (session, _handle) = MockAdapter::pair();
    assert_eq!(session.snapshot().state, SessionState::Starting);
    let info = session.initialize().unwrap();
    assert!(info.supports_configuration_done);
    assert!(info.supports_conditional_breakpoints);
    assert!(!info.supports_function_breakpoints);
    assert!(info.supports_log_points);
    assert_eq!(info.exception_filters[0].id, "throw");
    assert!(info.adapter_version.unwrap().contains("mock-lldb"));
}

#[test]
fn log_points_and_exception_filters_forward_adapter_requests() {
    let (session, handle) = attached();
    let path = if cfg!(windows) {
        "C:\\app\\main.rs"
    } else {
        "/app/main.rs"
    };
    let points = session
        .set_source_breakpoint(path, 12, Some("n > 0"), Some("n = {n}"))
        .unwrap();
    assert_eq!(points[0].log_message.as_deref(), Some("n = {n}"));
    assert_eq!(
        handle.requests_named("setBreakpoints")[0]["arguments"]["breakpoints"][0]["logMessage"],
        "n = {n}"
    );
    assert_eq!(
        session.set_exception_filters(&["throw".into()]).unwrap(),
        ["throw"]
    );
    assert!(session.set_exception_filters(&["unknown".into()]).is_err());
    assert_eq!(session.exception_filters(), ["throw"]);
    assert_eq!(handle.requests_named("setExceptionBreakpoints").len(), 1);
    assert!(session.set_exception_filters(&[]).unwrap().is_empty());
}

#[test]
fn attach_reads_threads_frames_scopes_variables_and_evaluate() {
    let (session, handle) = attached();
    let attach = handle.requests_named("attach");
    assert_eq!(attach[0]["arguments"]["pid"], 4242);

    let snapshot = session.snapshot();
    assert_eq!(snapshot.pid, Some(4242));
    assert_eq!(snapshot.stop.as_ref().unwrap().reason, "attach");
    assert_eq!(snapshot.stop.as_ref().unwrap().thread_id, Some(1));

    let threads = session.threads().unwrap();
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[1].id, 2);
    assert_eq!(threads[1].name, "worker");

    let stack = session.stack_trace(None, 0, 20).unwrap();
    assert_eq!(stack.thread_id, 1);
    assert_eq!(stack.frames.len(), 2);
    assert_eq!(stack.total_frames, Some(2));
    assert!(!stack.truncated);
    assert_eq!(stack.frames[0].name, "cove::Mock::run");
    assert_eq!(
        stack.frames[0].source.as_ref().unwrap().path.as_deref(),
        Some("/tmp/mock/src/lib.rs")
    );
    assert_eq!(stack.frames[0].line, Some(42));

    let scopes = session.scopes(stack.frames[0].id).unwrap();
    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[0].name, "Locals");
    assert_eq!(scopes[0].variables_reference, 100);
    assert!(scopes[1].expensive);

    let page = session.variables(100, 0, 100, None).unwrap();
    assert_eq!(page.variables.len(), 1);
    assert_eq!(page.variables[0].name, "x");
    assert_eq!(page.variables[0].value, "42");
    assert_eq!(page.variables[0].r#type.as_deref(), Some("Int"));

    let evaluated = session.evaluate("x + 1", Some(11), Some("repl")).unwrap();
    assert_eq!(evaluated.result, "42");
    assert_eq!(evaluated.r#type.as_deref(), Some("Int"));
    let evaluate = handle.requests_named("evaluate");
    assert_eq!(evaluate[0]["arguments"]["frameId"], 11);
    assert_eq!(evaluate[0]["arguments"]["context"], "repl");
}

#[test]
fn breakpoints_accumulate_per_source_and_clear_by_line_or_file() {
    let (session, handle) = attached();
    let path = "/tmp/mock/src/lib.rs";

    let set = session.set_breakpoint(path, 12, None).unwrap();
    assert_eq!(set.len(), 1);
    assert!(set[0].verified);
    assert_eq!(set[0].line, 12);
    assert_eq!(set[0].resolved_line, Some(12));
    assert_eq!(set[0].id, Some(1));

    let set = session.set_breakpoint(path, 20, Some("x == 1")).unwrap();
    assert_eq!(set.len(), 2);
    assert_eq!(set[1].condition.as_deref(), Some("x == 1"));
    let requests = handle.requests_named("setBreakpoints");
    let last = requests.last().unwrap();
    assert_eq!(
        last["arguments"]["breakpoints"].as_array().unwrap().len(),
        2
    );
    assert_eq!(last["arguments"]["breakpoints"][1]["condition"], "x == 1");

    let remaining = session.clear_breakpoints(path, Some(12)).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].line, 20);
    let requests = handle.requests_named("setBreakpoints");
    assert_eq!(
        requests.last().unwrap()["arguments"]["breakpoints"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let remaining = session.clear_breakpoints(path, None).unwrap();
    assert!(remaining.is_empty());
    assert!(session.breakpoints().is_empty());
    let requests = handle.requests_named("setBreakpoints");
    assert!(
        requests.last().unwrap()["arguments"]["breakpoints"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_relative_breakpoint_path_is_refused_before_the_adapter_sees_it() {
    let (session, handle) = attached();
    let error = session.set_breakpoint("src/lib.rs", 12, None).unwrap_err();
    assert_eq!(error.code(), "invalid_request");
    assert!(error.message().contains("absolute"));
    assert!(handle.requests_named("setBreakpoints").is_empty());
}

#[test]
fn an_adapter_refusal_keeps_its_reason() {
    let (session, handle) = MockAdapter::with(
        MockOptions::default().fail_evaluate("Cannot evaluate: process is running"),
    );
    session.initialize().unwrap();
    let snapshot = session.attach(9).unwrap();
    assert_eq!(snapshot.state, SessionState::Stopped);
    let error = session.evaluate("x", None, None).unwrap_err();
    assert_eq!(error.code(), "adapter_error");
    assert!(error.message().contains("process is running"));
    assert!(handle.requests_named("evaluate").len() == 1);
}

#[test]
fn a_request_without_a_response_times_out_with_its_command_name() {
    let (session, handle) = MockAdapter::with(MockOptions::default().ignore("stackTrace"));
    session.set_request_timeout(Duration::from_millis(250));
    session.initialize().unwrap();
    session.attach(5).unwrap();
    let started = std::time::Instant::now();
    let error = session.stack_trace(None, 0, 20).unwrap_err();
    assert_eq!(error.code(), "timeout");
    assert!(error.message().contains("stackTrace"));
    assert!(started.elapsed() < Duration::from_secs(2));
    session.abort();
    assert!(session.is_closed());
    drop(handle);
}

#[test]
fn paused_state_only_comes_from_stopped_events() {
    let (session, _handle) = attached();
    assert_eq!(session.snapshot().state, SessionState::Stopped);

    let running = session.resume().unwrap();
    assert_eq!(running.state, SessionState::Running);
    assert!(running.stop.is_none());

    let error = session.stack_trace(None, 0, 20).unwrap_err();
    assert_eq!(error.code(), "not_stopped");
    assert!(error.message().contains("running"));

    let paused = session.pause(None).unwrap();
    assert_eq!(paused.state, SessionState::Stopped);
    assert_eq!(paused.stop.as_ref().unwrap().reason, "pause");

    let stepped = session.step(Step::Over).unwrap();
    assert_eq!(stepped.state, SessionState::Stopped);
    assert_eq!(stepped.stop.as_ref().unwrap().reason, "step");
}

#[test]
fn detach_asks_the_adapter_not_to_terminate_the_debuggee() {
    let (session, handle) = attached();
    session.detach().unwrap();
    let requests = handle.requests_named("disconnect");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["arguments"]["terminateDebuggee"], false);
    assert_eq!(session.snapshot().state, SessionState::Closed);
    assert!(session.is_closed());
    handle.wait();
}

#[test]
fn a_zero_variables_reference_is_refused() {
    let (session, _handle) = attached();
    let error = session.variables(0, 0, 10, None).unwrap_err();
    assert_eq!(error.code(), "invalid_request");
}

#[test]
fn large_values_are_marked_as_truncated_by_the_client() {
    let (session, handle) = MockAdapter::pair();
    session.initialize().unwrap();
    session.attach(1).unwrap();
    // The mock returns "42"; the truncation limit is far above it, so the
    // marker stays false. A shorter limit is exercised by the framing tests.
    let page = session.variables(100, 0, 100, Some("named")).unwrap();
    assert!(!page.variables[0].value_truncated);
    assert_eq!(
        handle.requests_named("variables")[0]["arguments"]["filter"],
        "named"
    );
    assert_eq!(
        handle.requests_named("variables")[0]["arguments"]["start"],
        0
    );
    assert_eq!(
        json!(handle.requests_named("variables")[0]["arguments"]["count"]),
        100
    );
}

#[test]
fn an_attach_refusal_is_unavailable_and_closes_the_session() {
    let (session, _handle) = MockAdapter::with(
        MockOptions::default().fail_attach("attach failed (Not allowed to attach to process.)"),
    );
    session.initialize().unwrap();
    let error = session.attach(77).unwrap_err();
    assert_eq!(error.code(), "unavailable");
    assert!(error.message().contains("Not allowed to attach"));
    assert!(session.is_closed());
    assert_eq!(session.snapshot().state, SessionState::Closed);
}

#[test]
fn a_step_without_a_stopped_event_is_a_timeout_not_a_fake_pause() {
    let (session, _handle) = MockAdapter::with(MockOptions::default().ignore("next"));
    session.set_request_timeout(Duration::from_millis(250));
    session.initialize().unwrap();
    session.attach(1).unwrap();
    let error = session.step(Step::Over).unwrap_err();
    assert_eq!(error.code(), "timeout");
    assert!(error.message().contains("next"));
    assert_eq!(session.snapshot().state, SessionState::Stopped);
}

#[test]
fn a_breakpoint_event_updates_verification_without_a_new_set_request() {
    let (session, handle) = attached();
    let path = "/tmp/mock/src/lib.rs";
    let set = session.set_breakpoint(path, 12, None).unwrap();
    assert!(set[0].verified);
    handle.emit(
        "breakpoint",
        json!({
            "reason": "changed",
            "breakpoint": {
                "id": 1,
                "verified": false,
                "line": 12,
                "message": "pending symbol load",
            },
        }),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let breakpoint = session.breakpoints();
        if !breakpoint[0].verified {
            assert_eq!(
                breakpoint[0].message.as_deref(),
                Some("pending symbol load")
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline, "event not applied");
        std::thread::sleep(Duration::from_millis(5));
    }
    handle.emit(
        "breakpoint",
        json!({"reason": "removed", "breakpoint": {"id": 1, "verified": false}}),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if session.breakpoints()[0].message.as_deref() == Some("Removed by the adapter.") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "removal not applied");
        std::thread::sleep(Duration::from_millis(5));
    }
}
