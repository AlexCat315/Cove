//! Translation of `System/Sources/CoveProcess/Command.swift`.
//!
//! Executable lookup and authority belong to the caller's policy boundary; the
//! runner never invokes a shell or searches `PATH`.
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessCommand {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub directory: PathBuf,
    pub environment: BTreeMap<String, String>,
}

impl ProcessCommand {
    pub fn new(executable: PathBuf, directory: PathBuf) -> Self {
        Self {
            executable,
            arguments: Vec::new(),
            directory,
            environment: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessBudget {
    pub output_bytes: usize,
    pub pending_input_bytes: usize,
    pub lifetime_milliseconds: Option<u64>,
}

impl ProcessBudget {
    pub const MAXIMUM_BYTES: usize = 536_870_916;

    pub fn new() -> Self {
        Self {
            output_bytes: 1_048_576,
            pending_input_bytes: 4_194_304,
            lifetime_milliseconds: Some(30_000),
        }
    }
}

impl Default for ProcessBudget {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessEvent {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Exited(i32),
    StdoutClosed,
    StderrClosed,
}

/// Whether spawning an external process is permitted on this target.
/// Sandboxed mobile hosts cannot fork/exec, so every public launcher must
/// report [`ProcessError::Unavailable`] instead of surfacing an opaque
/// `io::Error` from `std::process::Command`.
pub const fn process_execution_available() -> bool {
    !cfg!(any(
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "android"
    ))
}

/// Shared guard for every public launcher (`ProcessSession::new`,
/// `ProcessSession::spawn`, `ProcessRunner::run*`, `NativeProcess::start`).
pub(crate) fn require_process_execution() -> Result<(), ProcessError> {
    if process_execution_available() {
        Ok(())
    } else {
        Err(ProcessError::Unavailable)
    }
}

impl ProcessEvent {
    /// Bytes charged against the event buffer budget.
    pub fn size(&self) -> usize {
        match self {
            Self::Stdout(data) | Self::Stderr(data) => data.len(),
            Self::Exited(_) | Self::StdoutClosed | Self::StderrClosed => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProcessError {
    #[error("Process execution is unavailable on this platform.")]
    Unavailable,
    #[error("Invalid process command.")]
    InvalidCommand,
    #[error("Invalid process resource limits.")]
    InvalidBudget,
    #[error("Process output exceeded its byte limit.")]
    OutputLimit,
    #[error("Pending process input exceeded its byte limit.")]
    InputLimit,
    #[error("Process exceeded its deadline.")]
    TimedOut,
    #[error("Process input is closed.")]
    Stopped,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

impl ProcessOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_owned()
    }

    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    pub fn append(&mut self, event: &ProcessEvent) {
        match event {
            ProcessEvent::Stdout(data) => self.stdout.extend_from_slice(data),
            ProcessEvent::Stderr(data) => self.stderr.extend_from_slice(data),
            ProcessEvent::Exited(status) => self.status = *status,
            ProcessEvent::StdoutClosed | ProcessEvent::StderrClosed => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_targets_are_the_only_ones_without_process_execution() {
        let mobile = cfg!(any(
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "android"
        ));
        assert_eq!(process_execution_available(), !mobile);
        assert_eq!(require_process_execution().is_ok(), !mobile);
        assert_eq!(
            require_process_execution().err(),
            mobile.then_some(ProcessError::Unavailable)
        );
    }

    #[test]
    fn unavailable_is_reported_with_a_stable_message() {
        assert_eq!(
            ProcessError::Unavailable.to_string(),
            "Process execution is unavailable on this platform."
        );
    }
}
