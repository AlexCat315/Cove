//! Translation of `System/Sources/CoveProcess/Run.swift`.
//!
//! The Swift API is asynchronous; without a workspace async runtime the Rust
//! entry points block the calling thread while preserving deadlines, limits and
//! cancellation. `run_with_cancel` exposes the cancellation flag that
//! `Task.isCancelled` supplies on Apple platforms.
use super::command::{ProcessBudget, ProcessCommand, ProcessError, ProcessOutput};
use super::event_buffer::ProcessFailure;
use super::session::ProcessSession;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum ProcessRunner {}

impl ProcessRunner {
    /// For synchronous host discovery hooks. Interactive work should use `run`
    /// from a dedicated thread so waiting does not occupy the caller's executor.
    pub fn run_synchronously(
        command: &ProcessCommand,
        budget: ProcessBudget,
    ) -> Result<ProcessOutput, ProcessFailure> {
        if budget.lifetime_milliseconds.is_none() {
            return Err(ProcessError::InvalidBudget.into());
        }
        let session = ProcessSession::new(command, budget.clone())?;
        session.close_input();
        let mut result = ProcessOutput::new();
        loop {
            match session.next_event(None) {
                Ok(Some(event)) => {
                    result.append(&event);
                    if result.stdout.len().saturating_add(result.stderr.len())
                        > budget.output_bytes
                    {
                        session.cancel();
                        return Err(ProcessFailure::Process(ProcessError::OutputLimit));
                    }
                }
                Ok(None) => return Ok(result),
                Err(error) => {
                    session.cancel();
                    return Err(shared_failure(error));
                }
            }
        }
    }

    /// Runs with the default never-cancelled token.
    pub fn run(
        command: &ProcessCommand,
        input: &[u8],
        budget: ProcessBudget,
    ) -> Result<ProcessOutput, ProcessFailure> {
        Self::run_with_cancel(command, input, budget, Arc::new(AtomicBool::new(false)))
    }

    pub fn run_with_cancel(
        command: &ProcessCommand,
        input: &[u8],
        budget: ProcessBudget,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ProcessOutput, ProcessFailure> {
        let session = ProcessSession::new(command, budget.clone())?;
        let check = || {
            if cancelled.load(Ordering::Acquire) {
                session.cancel();
                Err(ProcessFailure::Cancelled)
            } else {
                Ok(())
            }
        };
        check()?;
        if !input.is_empty() {
            if let Err(error) = session.write(input) {
                session.cancel();
                return Err(error);
            }
        }
        session.close_input();
        let mut result = ProcessOutput::new();
        for event in session.events() {
            check()?;
            let event = match event {
                Ok(event) => event,
                Err(error) => {
                    session.cancel();
                    return Err(shared_failure(error));
                }
            };
            result.append(&event);
            if result.stdout.len().saturating_add(result.stderr.len()) > budget.output_bytes {
                session.cancel();
                return Err(ProcessFailure::Process(ProcessError::OutputLimit));
            }
        }
        Ok(result)
    }
}

/// The event buffer shares one failure between consumers through an `Arc`; the
/// runner reports an owned copy so callers get a plain `ProcessFailure`.
fn shared_failure(failure: std::sync::Arc<ProcessFailure>) -> ProcessFailure {
    match failure.as_ref() {
        ProcessFailure::Process(error) => ProcessFailure::Process(error.clone()),
        ProcessFailure::Io(error) => ProcessFailure::Io(std::io::Error::new(error.kind(), error.to_string())),
        ProcessFailure::Cancelled => ProcessFailure::Cancelled,
        ProcessFailure::Buffer(error) => ProcessFailure::Buffer(*error),
    }
}
