//! Translation of `Terminal/Tests/CoveUITerminalTests/SessionTests.swift`.
//!
//! Behaviour checklist:
//! - program output reaches the screen and the exit code is reported
//! - typed input is echoed by the line discipline and hang-up ends the program
//! - the program sees the fitted grid size and later resizes
//! - the child environment identifies the emulator and honours overrides
//! - missing executables fail before forking and say so on screen
//! - user input returns the viewport to the live screen
//!

#![cfg(unix)]

use cove_terminal::process::{TerminalCommand, TerminalError};
use cove_terminal::session::{SessionEvent, SessionStatus, TerminalSession};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn session(command: TerminalCommand) -> TerminalSession {
    TerminalSession::new(command, 100)
}

fn screen(session: &TerminalSession) -> String {
    let frame = session.frame();
    (0..frame.rows)
        .map(|row| frame.text_of_row(row))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Applies pending output on this thread, as the host's invalidation wake does.
fn pump_until(session: &TerminalSession, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        session.pump();
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn program_output_reaches_the_screen_and_the_exit_code_is_reported() {
    let command = TerminalCommand::new("/bin/sh").with_arguments(vec![
        "-c".into(),
        "printf 'hello\\033[1mworld'; exit 3".into(),
    ]);
    let session = session(command);
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = events.clone();
    session.set_on_event(Some(std::rc::Rc::new(move |event: &SessionEvent| {
        sink.borrow_mut().push(event.clone());
    })));

    session.start().expect("the shell starts");
    assert_eq!(
        session.status(),
        SessionStatus::Running,
        "the session is running"
    );

    let settled = pump_until(&session, || session.status() != SessionStatus::Running);
    assert!(settled, "the program exited; screen:\n{}", screen(&session));

    assert_eq!(
        session.status(),
        SessionStatus::Exited(3),
        "the child's exit code is reported"
    );
    assert_eq!(
        events.borrow().last(),
        Some(&SessionEvent::Exited(3)),
        "the last event is the exit"
    );
    let text = screen(&session);
    assert!(
        text.starts_with("helloworld"),
        "the SGR bold did not stop the text landing; screen:\n{text}"
    );
    assert!(
        text.contains("[Process exited with code 3]"),
        "the exit is also shown in the terminal; screen:\n{text}"
    );
    assert_eq!(
        session.start(),
        Err(TerminalError::AlreadyStarted),
        "a session runs one program once"
    );
}

#[test]
fn typed_input_is_echoed_by_the_line_discipline_and_hang_up_ends_the_program() {
    let session = session(TerminalCommand::new("cat"));
    session.start().expect("cat starts");
    session.send("ping\r");

    // The Swift suite asserted `components(separatedBy: "ping").count >= 3`,
    // i.e. the word appearing at least twice: the line discipline echoes it and
    // `cat` writes it back.
    let echoed = pump_until(&session, || screen(&session).matches("ping").count() >= 2);
    assert!(echoed, "cat echoed the line; screen:\n{}", screen(&session));

    session.terminate();
    let ended = pump_until(&session, || session.status() != SessionStatus::Running);
    assert!(ended, "hang-up ended cat; screen:\n{}", screen(&session));
    assert_eq!(
        session.status(),
        SessionStatus::Exited(128 + libc::SIGHUP),
        "the reported code is the shell-style hang-up status"
    );
}

#[test]
fn the_program_sees_the_fitted_grid_size_and_later_resizes() {
    let command = TerminalCommand::new("/bin/sh")
        .with_arguments(vec!["-c".into(), "stty size; read _; stty size".into()]);
    let session = session(command);
    session.fit(100, 30, 8, 16);
    session.start().expect("the shell starts");

    let saw_initial = pump_until(&session, || screen(&session).contains("30 100"));
    assert!(
        saw_initial,
        "the child saw the fitted 30x100 grid; screen:\n{}",
        screen(&session)
    );

    session.fit(60, 20, 8, 16);
    session.send("\r");
    let saw_resize = pump_until(&session, || screen(&session).contains("20 60"));
    assert!(
        saw_resize,
        "the child saw the resized 20x60 grid; screen:\n{}",
        screen(&session)
    );
}

#[test]
fn child_environment_identifies_the_emulator_and_honours_overrides() {
    let mut environment = BTreeMap::new();
    environment.insert("COVE_TEST".to_owned(), "on".to_owned());
    let command = TerminalCommand::new("/bin/sh")
        .with_arguments(vec!["-c".into(), "printf \"$TERM:$COVE_TEST\"".into()])
        .with_environment(environment)
        .with_working_directory("/");
    let session = session(command);

    session.start().expect("the shell starts");
    let finished = pump_until(&session, || session.status() != SessionStatus::Running);
    assert!(
        finished,
        "the program exited; screen:\n{}",
        screen(&session)
    );

    assert!(
        screen(&session).starts_with("xterm-256color:on"),
        "TERM is the emulator identity and the override reached the child; screen:\n{}",
        screen(&session)
    );
}

#[test]
fn missing_executables_fail_before_forking_and_say_so_on_screen() {
    let expected = TerminalError::ExecutableNotFound("definitely-not-a-real-binary-cove".into());
    let session = session(TerminalCommand::new("definitely-not-a-real-binary-cove"));

    assert_eq!(
        session.start(),
        Err(expected.clone()),
        "resolving the executable fails before any fork"
    );
    assert_eq!(
        session.status(),
        SessionStatus::Failed(expected),
        "the failure is the session status"
    );
    assert!(
        screen(&session).starts_with("[Executable not found: definitely-not-a-real-binary-cove]"),
        "the terminal says why it did not start; screen:\n{}",
        screen(&session)
    );
    assert_eq!(
        session.start(),
        Err(TerminalError::AlreadyStarted),
        "a failed session cannot be restarted either"
    );
}

#[test]
fn user_input_returns_the_viewport_to_the_live_screen() {
    // A display session shows handed-in output without running a program, which
    // is the reachable equivalent of `.loginShell` in this suite: the scroll
    // behaviour under test belongs to `write`, not to the child process.
    let session = TerminalSession::display(100);
    session.fit(10, 2, 8, 16);
    session.feed_bytes(b"1\r\n2\r\n3\r\n4");
    session.scroll(-2);
    assert!(
        !session.frame().scroll.is_at_bottom(),
        "scrolling back leaves the live screen"
    );

    session.send("x");
    assert!(
        session.frame().scroll.is_at_bottom(),
        "user input returns the viewport to the live screen"
    );
}

#[test]
fn display_session_translates_bare_newlines_like_a_pipe() {
    let session = TerminalSession::display(100);
    session.fit(20, 4, 8, 16);
    session.show(b"one\ntwo\n");
    let frame = session.frame();

    assert_eq!(
        frame.text_of_row(0),
        "one",
        "a bare newline starts at the margin"
    );
    assert_eq!(frame.text_of_row(1), "two", "and the next line does too");

    session.show_exit(0);
    assert_eq!(
        session.status(),
        SessionStatus::Exited(0),
        "a display session can report the producing program's status"
    );
}
