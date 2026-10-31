//! Translation of `System/Tests/CoveProcessTests/BufferTests.swift`.
//!
//! Behaviour checklist:
//! - fragmented output is charged against a byte budget, and the append that
//!   would exceed it is rejected without losing earlier events
//! - terminal events are always accepted, even at the budget limit
//! - `finish()` ends iteration after the caller drains the accepted events

use cove_system::process::{ProcessEvent, ProcessEventBuffer};

#[test]
fn fragmented_output_uses_byte_budget() {
    let buffer = ProcessEventBuffer::new(128);
    for value in 0u8..128 {
        assert!(
            buffer.append(ProcessEvent::Stdout(vec![value])),
            "byte {value} fits the 128-byte budget"
        );
    }
    assert!(
        !buffer.append(ProcessEvent::Stdout(vec![128])),
        "the 129th byte exceeds the budget"
    );
    // Terminal events are never charged against the budget.
    assert!(buffer.append(ProcessEvent::Exit {
        code: 0,
        reason: cove_system::process::Termination::Exited
    }));
    buffer.finish();

    let mut output: Vec<u8> = Vec::new();
    let mut exit: Option<i32> = None;
    while let Ok(Some(event)) = buffer.next() {
        match event {
            ProcessEvent::Stdout(bytes) => output.extend(bytes),
            ProcessEvent::Exit { code, .. } => exit = Some(code),
            ProcessEvent::Stderr(_) => {}
        }
    }
    assert_eq!(output, (0u8..128).collect::<Vec<_>>());
    assert_eq!(exit, Some(0));
}

#[test]
fn finish_releases_an_idle_reader() {
    let buffer: ProcessEventBuffer = ProcessEventBuffer::new(128);
    buffer.finish();
    assert!(
        matches!(buffer.next(), Ok(None)),
        "a finished buffer without events reports end of stream"
    );
}
