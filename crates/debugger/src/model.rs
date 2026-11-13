//! Wire-shaped session data. Every field comes from a DAP response or event.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Starting,
    Attached,
    Running,
    Stopped,
    Exited,
    Terminated,
    Closed,
}

/// The stop event that paused the target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct StopInfo {
    pub reason: String,
    pub description: Option<String>,
    pub thread_id: Option<u64>,
    pub all_threads_stopped: bool,
    pub text: Option<String>,
    pub hit_breakpoint_ids: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ThreadInfo {
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceInfo {
    pub path: Option<String>,
    pub name: Option<String>,
    pub source_reference: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub source: Option<SourceInfo>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub instruction_pointer: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StackTrace {
    pub thread_id: u64,
    pub frames: Vec<StackFrame>,
    pub total_frames: Option<u64>,
    pub start_frame: u64,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Scope {
    pub name: String,
    pub variables_reference: u64,
    pub expensive: bool,
    pub presentation_hint: Option<String>,
    pub source: Option<SourceInfo>,
    pub line: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub value_truncated: bool,
    pub r#type: Option<String>,
    pub variables_reference: u64,
    pub evaluate_name: Option<String>,
    pub named_variables: Option<u64>,
    pub indexed_variables: Option<u64>,
    pub memory_reference: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VariablePage {
    pub variables: Vec<Variable>,
    pub start: u64,
    pub count: u64,
    pub total: Option<u64>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EvaluateResult {
    pub result: String,
    pub result_truncated: bool,
    pub r#type: Option<String>,
    pub variables_reference: u64,
    pub named_variables: Option<u64>,
    pub indexed_variables: Option<u64>,
    pub memory_reference: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Breakpoint {
    pub path: String,
    pub line: u32,
    pub resolved_line: Option<u32>,
    pub condition: Option<String>,
    pub log_message: Option<String>,
    pub id: Option<u64>,
    pub verified: bool,
    pub message: Option<String>,
}

/// A snapshot of session state without any synthesized frames or variables.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionSnapshot {
    pub state: SessionState,
    pub stop: Option<StopInfo>,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub adapter_version: Option<String>,
    pub exit_code: Option<i64>,
    pub events_dropped: u64,
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InitializeInfo {
    pub adapter_version: Option<String>,
    pub supports_configuration_done: bool,
    pub supports_function_breakpoints: bool,
    pub supports_conditional_breakpoints: bool,
    pub supports_log_points: bool,
    pub exception_filters: Vec<ExceptionFilter>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExceptionFilter {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DebuggerOutput {
    pub sequence: u64,
    pub category: String,
    pub text: String,
}
