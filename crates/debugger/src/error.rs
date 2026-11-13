//! Errors the debugger reports instead of synthesizing state.
use std::fmt;

/// A debugger failure with a stable machine-readable code.
///
/// Every message carries the reason the adapter or process reported. Callers
/// surface the code and message to the client unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebuggerError {
    code: &'static str,
    message: String,
}

impl DebuggerError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// The adapter refused a request; its words are preserved.
    pub fn adapter(command: &str, reason: impl Into<String>) -> Self {
        Self::new(
            "adapter_error",
            format!("lldb-dap rejected {command}: {}", reason.into()),
        )
    }

    /// A bounded wait elapsed without the expected response.
    pub fn timeout(what: impl Into<String>, elapsed: std::time::Duration) -> Self {
        Self::new(
            "timeout",
            format!(
                "lldb-dap did not answer {} within {} ms.",
                what.into(),
                elapsed.as_millis()
            ),
        )
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::new("unavailable", reason)
    }
}

impl fmt::Display for DebuggerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DebuggerError {}

impl From<std::io::Error> for DebuggerError {
    fn from(error: std::io::Error) -> Self {
        Self::new("adapter_io", error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DebuggerError>;
