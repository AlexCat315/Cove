//! Translation of `Terminal/Tests/CoveUITerminalTests/WindowsSessionTests.swift`.
//!
//! Behaviour checklist:
//! - ConPTY runs a program and reports its exit code
//! - ConPTY delivers program output
//! - ConPTY forwards cmd switches and their command
//!
//! Gated on `cfg(windows)` for the same reason as the Swift `#if os(Windows)`:
//! these need `powershell.exe`/`cmd.exe` and a real ConPTY pseudo console. The
//! whole suite compiles on every platform — the gate only removes the bodies —
//! so `cargo test --no-run` still type-checks the port on macOS.
//!
//! Named difference (`GAP:`): the Swift helper ran on a `@MainActor` and applied
//! output through `DispatchQueue.main.async`. The Rust session hands output to
//! its host through `cove_core::scene::LocalInvalidationHandle`, which queues
//! invalidations for the UI thread rather than running them, so the helper calls
//! `TerminalSession::pump()` in its wait loop exactly as a host's invalidation
//! wake does.

#![cfg(windows)]

use cove_terminal::process::TerminalCommand;
use cove_terminal::session::{SessionStatus, TerminalSession};
use std::time::{Duration, Instant};

/// Runs `executable` on a ConPTY and reports where it ended up.
fn run(executable: &str, arguments: &[&str], wait_for_output: bool) -> (SessionStatus, String) {
    let session = TerminalSession::new(
        TerminalCommand::new(executable)
            .with_arguments(arguments.iter().map(|value| (*value).to_owned()).collect()),
        100,
    );
    session.start().expect("the program starts on ConPTY");

    let exit_deadline = Instant::now() + Duration::from_secs(20);
    while session.status() == SessionStatus::Running {
        session.pump();
        if Instant::now() >= exit_deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut screen = String::new();
    let output_deadline = Instant::now() + Duration::from_secs(20);
    while wait_for_output && Instant::now() < output_deadline {
        session.pump();
        screen = read_screen(&session);
        if !screen.trim().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !wait_for_output {
        session.pump();
        screen = read_screen(&session);
    }

    (session.status(), screen)
}

fn read_screen(session: &TerminalSession) -> String {
    let frame = session.frame();
    (0..frame.rows)
        .map(|row| frame.text_of_row(row))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn conpty_runs_a_program_and_reports_its_exit_code() {
    let (status, _) = run(
        "powershell.exe",
        &["-NoProfile", "-Command", "exit 5"],
        false,
    );
    assert_eq!(status, SessionStatus::Exited(5), "ConPTY reports the exit code");
}

#[test]
fn conpty_delivers_program_output() {
    let (status, screen) = run(
        "powershell.exe",
        &["-NoProfile", "-Command", "Write-Output hello-cove"],
        true,
    );
    assert_eq!(status, SessionStatus::Exited(0), "the program exited cleanly");
    assert!(
        screen.contains("hello-cove"),
        "ConPTY delivered the program's output; screen:\n{screen}"
    );
}

#[test]
fn conpty_forwards_cmd_switches_and_their_command() {
    let (status, screen) = run("cmd.exe", &["/d", "/c", "echo", "hello-cove"], true);
    assert_eq!(status, SessionStatus::Exited(0), "the program exited cleanly");
    assert!(
        screen.contains("hello-cove"),
        "cmd's switches and command reached the child; screen:\n{screen}"
    );
}
