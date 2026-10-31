//! Translation of `System/Tests/CoveProcessTests/Contracts.swift`.
//!
//! Behaviour checklist:
//! - concurrent synchronous runs each see both streams and a zero exit
//! - short output arrives before EOF and round-trips through stdin
//! - the environment reaches the child
//! - native argument quoting preserves empty, spaced, non-ASCII, quoted and
//!   trailing-backslash arguments
//! - cancelling an idle session wakes its reader with a cancellation

use std::path::PathBuf;
use std::time::Duration;

use cove_system::process::{
    ProcessCommand, ProcessEvent, ProcessLimits, ProcessSession, Termination,
};

/// Locates the fixture binary Cargo builds next to the test executable.
fn fixture() -> PathBuf {
    // `current_exe()` is `.../<profile>/deps/contracts-<hash>`. The fixture is a
    // `[[bin]]`, which Cargo writes to `.../<profile>/`, so step up twice.
    let mut directory = std::env::current_exe().expect("test executable path");
    directory.pop();
    if directory.file_name().is_some_and(|name| name == "deps") {
        directory.pop();
    }
    #[cfg(windows)]
    let name = "cove_process_fixture.exe";
    #[cfg(not(windows))]
    let name = "cove_process_fixture";
    directory.join(name)
}

fn command(mode: &str) -> ProcessCommand {
    let mut command = ProcessCommand::new(fixture()).expect("fixture resolves on PATH");
    command.arguments = vec![mode.into()];
    command
}

fn limits() -> ProcessLimits {
    ProcessLimits {
        timeout: Duration::from_secs(10),
        ..Default::default()
    }
}

#[test]
fn concurrent_synchronous_discovery() {
    let workers = std::thread::available_parallelism()
        .map(|count| (count.get() * 2).clamp(8, 32))
        .unwrap_or(8);
    let mut handles = Vec::new();
    for _ in 0..workers {
        handles.push(std::thread::spawn(|| {
            let session = ProcessSession::spawn(&command("output"), limits()).expect("spawn");
            session.collect().expect("collect")
        }));
    }
    for handle in handles {
        let output = handle.join().expect("worker thread");
        assert_eq!(output.code, 0);
        assert_eq!(output.reason, Termination::Exited);
        assert_eq!(String::from_utf8_lossy(&output.stdout), "stdout");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "stderr");
    }
}

#[test]
fn short_output_is_delivered_before_eof() {
    let mut session = ProcessSession::spawn(&command("interactive"), limits()).expect("spawn");
    let mut bytes: Vec<u8> = Vec::new();
    while bytes.len() < 5 {
        match session.next_event() {
            Some(ProcessEvent::Stdout(chunk)) => bytes.extend(chunk),
            Some(_) => {}
            None => break,
        }
    }
    assert_eq!(String::from_utf8_lossy(&bytes), "ready");

    session.write_input(b"ping").expect("write input");
    while bytes.len() < 9 {
        match session.next_event() {
            Some(ProcessEvent::Stdout(chunk)) => bytes.extend(chunk),
            Some(_) => {}
            None => break,
        }
    }
    assert_eq!(String::from_utf8_lossy(&bytes), "readyping");
    session.close_input();
    session.cancel();
}

#[test]
fn both_streams_and_environment_survive_launch() {
    let session = ProcessSession::spawn(&command("output"), limits()).expect("spawn");
    let output = session.collect().expect("collect");
    assert_eq!(output.code, 0);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "stdout");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "stderr");

    let mut request = command("environment");
    request
        .environment
        .insert("COVE_FIXTURE_VALUE".into(), "value".into());
    let session = ProcessSession::spawn(&request, limits()).expect("spawn");
    let output = session.collect().expect("collect");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "value");
}

#[test]
fn cancellation_wakes_an_idle_reader() {
    let session = ProcessSession::spawn(&command("sleep"), limits()).expect("spawn");
    session.cancel();
    // Draining after cancellation must terminate instead of blocking forever.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while session.next_event().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "reader released promptly"
        );
    }
}

#[test]
fn native_arguments_keep_quotes_whitespace_and_backslashes() {
    let mut request = command("arguments");
    let arguments = [
        "",
        "space value",
        "🥭 中文",
        "quoted\"value",
        "trailing\\",
        "&|%PATH%",
    ];
    for argument in arguments {
        request.arguments.push(argument.into());
    }
    let session = ProcessSession::spawn(&request, limits()).expect("spawn");
    let output = session.collect().expect("collect");
    // The fixture prints one JSON string per argument, in an array.
    let decoded = String::from_utf8_lossy(&output.stdout);
    for argument in arguments {
        let escaped = argument.replace('\\', "\\\\").replace('"', "\\\"");
        assert!(
            decoded.contains(&format!("\"{escaped}\"")),
            "argument {argument:?} survives the native quoting layer in {decoded}"
        );
    }
}
