//! Native debugger sessions for Cove DevTools.
//!
//! The controller owns one [`DebugSession`] per attached runtime process. Two
//! backends implement it:
//!
//! - [`NativeSession`] loads liblldb into the controller and drives the SB API
//!   directly. It is preferred whenever liblldb can be loaded.
//! - [`DapSession`] spawns `lldb-dap`, frames DAP messages, correlates
//!   responses and tears down the adapter's process tree when it closes. It is
//!   the fallback when liblldb is missing or `COVE_DEBUGGER_BACKEND=dap`.
//!
//! Both derive paused state only from debugger events.
//!
//! # Real data only
//!
//! Stack frames, scopes, variables and evaluation results are copied from LLDB
//! or DAP responses. A running debuggee returns [`DebuggerError`] code `not_stopped`
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
mod backend;
mod error;
mod ffi;
mod framing;
mod model;
mod native;
mod process;
mod session;
pub mod testing;

pub use adapter::{Adapter, Availability};
pub use backend::{BackendKind, DebugSession, NativeBackend};
pub use error::{DebuggerError, Result};
pub use model::{
    Breakpoint, DebuggerOutput, EvaluateResult, ExceptionFilter, InitializeInfo, Scope,
    SessionSnapshot, SessionState, SourceInfo, StackFrame, StackTrace, StopInfo, ThreadInfo,
    Variable, VariablePage,
};
pub use native::NativeSession;
pub use session::{DapSession, Step};

#[cfg(test)]
mod tests;
