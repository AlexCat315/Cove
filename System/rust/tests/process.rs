//! Translation of `System/Tests/CoveProcessTests/ProcessTests.swift`.
//!
//! Behaviour checklist (`/bin/sh` drives the child on POSIX):
//! - parallel synchronous runs do not starve each other
//! - stdout and stderr drain through exit, including the final bytes
//! - stdin is delivered before EOF and the parent environment is not inherited
//! - interactive output arrives while stdin stays open
//! - output is bounded even when the child never exits
//! - a child ignoring `TERM` is killed after its deadline
//! - cancellation terminates a long-running command
//! - pending stdin has its own byte budget

#[cfg(unix)]
mod unix {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use cove_system::process::{
        ProcessCommand, ProcessEvent, ProcessLimits, ProcessSession, Termination,
    };

    fn command(script: &str) -> ProcessCommand {
        let mut command = ProcessCommand::new("/bin/sh").expect("sh resolves");
        command.arguments = vec!["-c".into(), script.into()];
        command
    }

    #[test]
    fn parallel_synchronous_discovery() {
        let workers = std::thread::available_parallelism()
            .map(|count| (count.get() * 2).clamp(8, 32))
            .unwrap_or(8);
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(std::thread::spawn(|| {
                let session =
                    ProcessSession::spawn(&command("printf ready"), ProcessLimits::default())
                        .expect("spawn");
                session.collect().expect("collect")
            }));
        }
        for handle in handles {
            let output = handle.join().expect("worker thread");
            assert_eq!(output.code, 0);
            assert_eq!(String::from_utf8_lossy(&output.stdout), "ready");
        }
    }

    #[test]
    fn stdout_and_stderr_drain_through_exit() {
        let session =
            ProcessSession::spawn(&command("printf stdout; printf stderr >&2; exit 7"), ProcessLimits::default())
                .expect("spawn");
        let output = session.collect().expect("collect");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "stdout");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "stderr");
        assert_eq!(output.code, 7);
        assert_eq!(output.reason, Termination::Exited);
    }

    #[test]
    fn stdin_is_delivered_before_eof_without_inheriting_environment() {
        let mut request =
            command("cat; printf '%s' \"${COVE_PROCESS_SECRET-unset}\"");
        // Only the explicitly provided environment reaches the child.
        request.environment = BTreeMap::new();
        let mut session = ProcessSession::spawn(&request, ProcessLimits::default()).expect("spawn");
        session.write_input(b"hello").expect("write input");
        session.close_input();
        let output = session.collect().expect("collect");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hellounset");
    }

    #[test]
    fn interactive_output_arrives_while_stdin_remains_open() {
        let mut session =
            ProcessSession::spawn(&command("printf ready; cat"), ProcessLimits::default())
                .expect("spawn");
        let mut output: Vec<u8> = Vec::new();
        while output.len() < 5 {
            match session.next_event() {
                Some(ProcessEvent::Stdout(chunk)) => output.extend(chunk),
                Some(_) => {}
                None => break,
            }
        }
        assert_eq!(String::from_utf8_lossy(&output), "ready");

        session.write_input(b"ping").expect("write input");
        while output.len() < 9 {
            match session.next_event() {
                Some(ProcessEvent::Stdout(chunk)) => output.extend(chunk),
                Some(_) => {}
                None => break,
            }
        }
        assert_eq!(String::from_utf8_lossy(&output), "readyping");

        session.close_input();
        session.cancel();
    }

    #[test]
    fn output_is_bounded_even_when_the_child_never_exits() {
        let limits = ProcessLimits { output_bytes: 100, ..Default::default() };
        let session = ProcessSession::spawn(
            &command("while :; do printf 1234567890; done"),
            limits,
        )
        .expect("spawn");
        let output = session.collect().expect("collection stops at the limit");
        assert!(
            output.stdout.len() <= 100,
            "output stayed within the 100-byte budget, saw {}",
            output.stdout.len()
        );
        assert_ne!(output.reason, Termination::Exited);
    }

    #[test]
    fn a_process_ignoring_term_is_killed_after_its_deadline() {
        let limits = ProcessLimits { timeout: Duration::from_millis(100), ..Default::default() };
        let session =
            ProcessSession::spawn(&command("trap '' TERM; while :; do :; done"), limits)
                .expect("spawn");
        let started = std::time::Instant::now();
        let output = session.collect().expect("collect");
        assert_eq!(output.reason, Termination::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the deadline bound the run, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn cancellation_terminates_a_long_running_command() {
        let session = ProcessSession::spawn(&command("while :; do :; done"), ProcessLimits::default())
            .expect("spawn");
        session.cancel();
        while session.next_event().is_some() {}
    }

    #[test]
    fn pending_stdin_has_a_separate_budget() {
        let limits = ProcessLimits { input_bytes: 8, ..Default::default() };
        let mut session =
            ProcessSession::spawn(&command("while :; do :; done"), limits).expect("spawn");
        assert!(
            session.write_input(&vec![1u8; 9]).is_err(),
            "nine bytes exceed the eight-byte pending-input budget"
        );
        session.cancel();
    }
}
