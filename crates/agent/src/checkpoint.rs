//! Durable, provider-neutral execution of an agent run.

mod storage;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool::{tool_message, validate_tool_result};
use crate::{
    AgentError, AgentMessage, AgentRole, AgentRun, AgentSession, AgentToolCall, AgentToolResult,
    CheckpointToolKey,
};

#[cfg(feature = "checkpoint-files")]
pub use storage::FileCheckpointStore;
pub use storage::MemoryCheckpointStore;

const FORMAT_VERSION: u32 = 1;
const MAX_CHECKPOINT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub enum CheckpointError {
    Agent(AgentError),
    Storage(std::io::Error),
    Invalid(&'static str),
    Exists,
    Missing,
    Stale,
    UnexpectedDecision,
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(error) => error.fmt(output),
            Self::Storage(error) => write!(output, "Checkpoint storage: {error}"),
            Self::Invalid(reason) => write!(output, "Invalid checkpoint: {reason}"),
            Self::Exists => output.write_str("A checkpoint with this ID already exists."),
            Self::Missing => output.write_str("No checkpoint exists for this ID."),
            Self::Stale => output
                .write_str("The checkpoint changed. Read its current revision before deciding."),
            Self::UnexpectedDecision => {
                output.write_str("Tool decision does not match the checkpoint state.")
            }
        }
    }
}

impl std::error::Error for CheckpointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Agent(error) => Some(error),
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AgentError> for CheckpointError {
    fn from(error: AgentError) -> Self {
        Self::Agent(error)
    }
}

impl From<std::io::Error> for CheckpointError {
    fn from(error: std::io::Error) -> Self {
        Self::Storage(error)
    }
}

/// Storage must serialize leases per run ID and publish each save atomically.
/// Loads outside a lease return the last published snapshot.
pub trait CheckpointStore: Send + Sync {
    fn load(&self, id: &str) -> Result<Option<AgentCheckpoint>, CheckpointError>;
    fn lease(&self, id: &str) -> Result<Box<dyn CheckpointLease + '_>, CheckpointError>;
}

/// A lease holds exclusive access to one run until it is dropped. Use a
/// lease's own load inside the lease to support backends with locked reads.
pub trait CheckpointLease {
    fn load(&self) -> Result<Option<AgentCheckpoint>, CheckpointError>;
    fn save(&mut self, checkpoint: &AgentCheckpoint) -> Result<(), CheckpointError>;
}

/// A host selects the execution policy when it creates a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPolicy {
    Automatic,
    ConfirmWrites,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentCheckpoint {
    version: u32,
    id: String,
    scope: String,
    revision: u64,
    policy: ToolPolicy,
    maximum_tool_rounds: usize,
    tool_rounds: usize,
    messages: Vec<AgentMessage>,
    phase: Phase,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Model,
    Tool(PendingTool),
    Complete,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PendingTool {
    assistant: usize,
    next: usize,
    state: ToolState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ToolState {
    Queued,
    Approval(Value),
    Approved(Value),
    InFlight { read_only: bool },
}

/// A snapshot of the next boundary in the run. An in-flight checkpoint can
/// be visible while its owner is executing. After the lease is released, a
/// read can resume automatically; a write requires an authoritative result.
#[derive(Debug, PartialEq)]
pub enum CheckpointStatus<'a> {
    Model,
    ToolReady(&'a AgentToolCall),
    AwaitingApproval(&'a AgentToolCall),
    ToolInFlight {
        call: &'a AgentToolCall,
        read_only: bool,
    },
    Completed(&'a str),
}

impl AgentCheckpoint {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn messages(&self) -> &[AgentMessage] {
        &self.messages
    }

    pub fn status(&self) -> Result<CheckpointStatus<'_>, CheckpointError> {
        match &self.phase {
            Phase::Model => Ok(CheckpointStatus::Model),
            Phase::Tool(pending) => {
                let call = self.pending_call(pending)?;
                Ok(match pending.state {
                    ToolState::Queued | ToolState::Approved(_) => CheckpointStatus::ToolReady(call),
                    ToolState::Approval(_) => CheckpointStatus::AwaitingApproval(call),
                    ToolState::InFlight { read_only } => {
                        CheckpointStatus::ToolInFlight { call, read_only }
                    }
                })
            }
            Phase::Complete => {
                let answer = self
                    .messages
                    .last()
                    .filter(|last| last.role == AgentRole::Assistant && last.calls.is_empty())
                    .ok_or(CheckpointError::Invalid("completed run has no final reply"))?;
                Ok(CheckpointStatus::Completed(&answer.text))
            }
        }
    }

    /// The same key passed to a contextual executor, including on retry.
    pub fn tool_key(&self) -> Result<Option<CheckpointToolKey<'_>>, CheckpointError> {
        let Phase::Tool(pending) = &self.phase else {
            return Ok(None);
        };
        self.pending_call(pending)?;
        Ok(Some(CheckpointToolKey {
            scope: &self.scope,
            run_id: &self.id,
            assistant_message: pending.assistant,
            call_index: pending.next,
        }))
    }

    pub fn completed_run(&self) -> Result<Option<AgentRun>, CheckpointError> {
        match self.status()? {
            CheckpointStatus::Completed(answer) => Ok(Some(AgentRun {
                answer: answer.to_owned(),
                messages: self.messages.clone(),
                tool_rounds: self.tool_rounds,
            })),
            _ => Ok(None),
        }
    }

    fn pending_call(&self, pending: &PendingTool) -> Result<&AgentToolCall, CheckpointError> {
        let invalid = || CheckpointError::Invalid("tool cursor does not match the transcript");
        let assistant = self
            .messages
            .get(pending.assistant)
            .filter(|m| m.role == AgentRole::Assistant)
            .ok_or_else(invalid)?;
        let call = assistant.calls.get(pending.next).ok_or_else(invalid)?;
        let results = &self.messages[pending.assistant + 1..];
        if results.len() != pending.next
            || !results.iter().zip(&assistant.calls).all(|(message, call)| {
                message.role == AgentRole::Tool
                    && message.result_for.as_deref() == Some(call.id.as_str())
            })
        {
            return Err(invalid());
        }
        Ok(call)
    }

    fn validate(&self, id: &str, scope: &str) -> Result<(), CheckpointError> {
        if self.version != FORMAT_VERSION {
            return Err(CheckpointError::Invalid("unsupported format version"));
        }
        if self.id != id || self.scope != scope {
            return Err(CheckpointError::Invalid(
                "run ID or execution scope does not match",
            ));
        }
        if !(1..=32).contains(&self.maximum_tool_rounds)
            || self.tool_rounds > self.maximum_tool_rounds
        {
            return Err(CheckpointError::Invalid("invalid tool round count"));
        }
        if matches!(self.phase, Phase::Tool(_)) && self.tool_rounds == 0 {
            return Err(CheckpointError::Invalid("pending tool has no round"));
        }
        self.status()?;
        Ok(())
    }

    fn append_result(&mut self, message: AgentMessage) -> Result<(), CheckpointError> {
        let Phase::Tool(pending) = &mut self.phase else {
            return Err(CheckpointError::Invalid("tool result without pending tool"));
        };
        pending.next += 1;
        if pending.next == self.messages[pending.assistant].calls.len() {
            self.phase = Phase::Model;
        } else {
            pending.state = ToolState::Queued;
        }
        self.messages.push(message);
        Ok(())
    }

    fn set_tool_state(&mut self, state: ToolState) {
        if let Phase::Tool(pending) = &mut self.phase {
            pending.state = state;
        }
    }
}

/// Decisions are tied to the reviewed revision so an old UI cannot approve a
/// different pending call. For an interrupted write, record only an outcome
/// established by the host or its operator.
pub enum ToolDecision {
    Approve,
    Reject(String),
    RecordOutcome(AgentToolResult),
}

/// Drives an existing `AgentSession` one model or tool step at a time. The
/// scope is a host-owned identifier for the workspace and authority context.
pub struct AgentRunner<'a> {
    session: &'a AgentSession,
    store: &'a dyn CheckpointStore,
    scope: &'a str,
}

impl<'a> AgentRunner<'a> {
    pub fn new(
        session: &'a AgentSession,
        store: &'a dyn CheckpointStore,
        scope: &'a str,
    ) -> Result<Self, CheckpointError> {
        if scope.trim().is_empty() || scope.len() > 1024 {
            return Err(CheckpointError::Invalid(
                "execution scope must have 1–1024 bytes",
            ));
        }
        Ok(Self {
            session,
            store,
            scope,
        })
    }

    pub fn start(
        &self,
        id: &str,
        messages: Vec<AgentMessage>,
        policy: ToolPolicy,
    ) -> Result<AgentCheckpoint, CheckpointError> {
        validate_id(id)?;
        let mut lease = self.store.lease(id)?;
        if lease.load()?.is_some() {
            return Err(CheckpointError::Exists);
        }
        let checkpoint = AgentCheckpoint {
            version: FORMAT_VERSION,
            id: id.to_owned(),
            scope: self.scope.to_owned(),
            revision: 0,
            policy,
            maximum_tool_rounds: self.session.maximum_tool_rounds(),
            tool_rounds: 0,
            messages: self.session.prepare(messages),
            phase: Phase::Model,
        };
        lease.save(&checkpoint)?;
        Ok(checkpoint)
    }

    pub fn load(&self, id: &str) -> Result<AgentCheckpoint, CheckpointError> {
        validate_id(id)?;
        self.checked(id, self.store.load(id)?)
    }

    pub fn advance(&self, id: &str) -> Result<AgentCheckpoint, CheckpointError> {
        validate_id(id)?;
        let mut lease = self.store.lease(id)?;
        let mut checkpoint = self.checked(id, lease.load()?)?;
        match &checkpoint.phase {
            Phase::Complete => return Ok(checkpoint),
            Phase::Model => {
                let assistant = self.session.request_model(
                    &checkpoint.messages,
                    checkpoint.tool_rounds,
                    checkpoint
                        .maximum_tool_rounds
                        .min(self.session.maximum_tool_rounds()),
                )?;
                checkpoint.phase = if assistant.calls.is_empty() {
                    Phase::Complete
                } else {
                    checkpoint.tool_rounds += 1;
                    Phase::Tool(PendingTool {
                        assistant: checkpoint.messages.len(),
                        next: 0,
                        state: ToolState::Queued,
                    })
                };
                checkpoint.messages.push(assistant);
            }
            Phase::Tool(pending) => {
                let state = pending.state.clone();
                if matches!(
                    state,
                    ToolState::Approval(_) | ToolState::InFlight { read_only: false }
                ) {
                    return Ok(checkpoint);
                }
                let call = checkpoint.pending_call(pending)?.clone();
                self.advance_tool(&mut checkpoint, &mut *lease, call, state)?;
            }
        }
        commit(&mut *lease, &mut checkpoint)?;
        Ok(checkpoint)
    }

    /// Drive the run to completion, approval, or an unresolved write. The
    /// observer runs before each step with the published snapshot and can
    /// pause this worker by returning false.
    pub fn drive(
        &self,
        id: &str,
        mut should_continue: impl FnMut(&AgentCheckpoint) -> bool,
    ) -> Result<AgentCheckpoint, CheckpointError> {
        let mut checkpoint = self.load(id)?;
        while should_continue(&checkpoint) {
            if matches!(
                checkpoint.status()?,
                CheckpointStatus::AwaitingApproval(_)
                    | CheckpointStatus::Completed(_)
                    | CheckpointStatus::ToolInFlight {
                        read_only: false,
                        ..
                    }
            ) {
                break;
            }
            checkpoint = self.advance(id)?;
        }
        Ok(checkpoint)
    }

    fn advance_tool(
        &self,
        checkpoint: &mut AgentCheckpoint,
        lease: &mut dyn CheckpointLease,
        call: AgentToolCall,
        state: ToolState,
    ) -> Result<(), CheckpointError> {
        let tool = match self.session.validate_call(&call) {
            Ok(tool) => tool,
            Err(error) => return checkpoint.append_result(tool_message(call.id, Err(error))),
        };
        let contract = tool.mcp_definition();
        let requires_approval = match state {
            ToolState::Approved(prior) => prior != contract,
            ToolState::InFlight { read_only: true } => !tool.read_only,
            _ => !tool.read_only && checkpoint.policy == ToolPolicy::ConfirmWrites,
        };
        if requires_approval {
            checkpoint.set_tool_state(ToolState::Approval(contract));
            return Ok(());
        }

        checkpoint.set_tool_state(ToolState::InFlight {
            read_only: tool.read_only,
        });
        commit(lease, checkpoint)?;
        let result = self.session.execute_call(&call, checkpoint.tool_key()?);
        if !tool.read_only
            && let Err(error) = result
        {
            // The executor can fail after a side effect. Keep the committed
            // in-flight marker so a later worker cannot repeat the write.
            return Err(error.into());
        }
        checkpoint.append_result(tool_message(call.id, result))
    }

    pub fn resolve(
        &self,
        id: &str,
        revision: u64,
        decision: ToolDecision,
    ) -> Result<AgentCheckpoint, CheckpointError> {
        validate_id(id)?;
        let mut lease = self.store.lease(id)?;
        let mut checkpoint = self.checked(id, lease.load()?)?;
        if checkpoint.revision != revision {
            return Err(CheckpointError::Stale);
        }
        let Phase::Tool(pending) = &checkpoint.phase else {
            return Err(CheckpointError::UnexpectedDecision);
        };
        let call = checkpoint.pending_call(pending)?;
        match (&pending.state, decision) {
            (ToolState::Approval(contract), ToolDecision::Approve) => {
                checkpoint.set_tool_state(ToolState::Approved(contract.clone()));
            }
            (ToolState::Approval(_), ToolDecision::Reject(reason)) => {
                let reason = reason.chars().take(2048).collect::<String>();
                checkpoint.append_result(tool_message(
                    call.id.clone(),
                    Ok(AgentToolResult::encode(
                        &serde_json::json!({"error": reason}),
                        true,
                    )),
                ))?;
            }
            (ToolState::InFlight { .. }, ToolDecision::RecordOutcome(result)) => {
                validate_tool_result(&result)?;
                checkpoint.append_result(tool_message(call.id.clone(), Ok(result)))?;
            }
            _ => return Err(CheckpointError::UnexpectedDecision),
        }
        commit(&mut *lease, &mut checkpoint)?;
        Ok(checkpoint)
    }

    /// Append a new user turn to a completed run, retaining its transcript
    /// and execution policy. Tool round limits start over for the new turn.
    pub fn continue_with(
        &self,
        id: &str,
        revision: u64,
        input: impl Into<String>,
    ) -> Result<AgentCheckpoint, CheckpointError> {
        validate_id(id)?;
        let mut lease = self.store.lease(id)?;
        let mut checkpoint = self.checked(id, lease.load()?)?;
        if checkpoint.revision != revision {
            return Err(CheckpointError::Stale);
        }
        if !matches!(checkpoint.phase, Phase::Complete) {
            return Err(CheckpointError::Invalid(
                "finish this turn before appending another",
            ));
        }
        checkpoint.messages.push(AgentMessage::user(input));
        checkpoint.phase = Phase::Model;
        checkpoint.tool_rounds = 0;
        commit(&mut *lease, &mut checkpoint)?;
        Ok(checkpoint)
    }

    fn checked(
        &self,
        id: &str,
        checkpoint: Option<AgentCheckpoint>,
    ) -> Result<AgentCheckpoint, CheckpointError> {
        let checkpoint = checkpoint.ok_or(CheckpointError::Missing)?;
        checkpoint.validate(id, self.scope)?;
        Ok(checkpoint)
    }
}

fn commit(
    lease: &mut dyn CheckpointLease,
    checkpoint: &mut AgentCheckpoint,
) -> Result<(), CheckpointError> {
    checkpoint.revision = checkpoint
        .revision
        .checked_add(1)
        .ok_or(CheckpointError::Invalid("revision exhausted"))?;
    lease.save(checkpoint)
}

fn validate_id(id: &str) -> Result<(), CheckpointError> {
    if !crate::portable_storage_id(id) {
        return Err(CheckpointError::Invalid(
            "run ID must be 1–128 lowercase ASCII letters, digits, hyphens or underscores",
        ));
    }
    Ok(())
}
