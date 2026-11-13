//! Debug Adapter Protocol client for `lldb-dap`.
//!
//! The controller owns one [`DapSession`] per attached runtime process. The
//! session spawns the adapter, frames DAP messages, correlates responses,
//! derives paused state only from adapter events and tears down the adapter's
//! process tree when it closes.
//!
//! # Real data only
//!
//! Stack frames, scopes, variables and evaluation results are copied from DAP
//! responses. A running debuggee returns [`DebuggerError`] code `not_stopped`
//! rather than an empty or remembered stack. An attach refusal keeps the
//! adapter's own reason (`Not allowed to attach to process`, a missing PID and
//! so on) and the session is closed instead of remaining half-attached.
//!
//! # Process rules
//!
//! Discovery, version checks and cleanup commands run with hard timeouts. The
//! adapter is started in its own process group; cleanup also walks the process
//! table because `lldb-dap` starts `debugserver` in a separate group. No session
//! ever launches a target: attach is the only entry point, and detach asks the
//! adapter not to terminate the debuggee.

mod adapter;
mod error;
mod framing;
mod model;
mod process;
mod session;
pub mod testing;

pub use adapter::{Adapter, Availability};
pub use error::{DebuggerError, Result};
pub use model::{
    Breakpoint, EvaluateResult, InitializeInfo, Scope, SessionSnapshot, SessionState, SourceInfo,
    StackFrame, StackTrace, StopInfo, ThreadInfo, Variable, VariablePage,
};
pub use session::{DapSession, Step};

#[cfg(test)]
mod tests;
