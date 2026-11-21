//! UI-free agent foundation: JSON Schema subset, tools, the provider-neutral
//! tool loop, skills, MCP and its line transports.
//!
//! The optional `openai-compatible` feature adds a model adapter over Cove HTTP.
//! UI hosts add their own tools on top.
pub mod checkpoint;
mod mcp;
pub mod memory;
#[cfg(feature = "openai-compatible")]
pub mod openai_compatible;
pub mod repetition;
pub mod retrieval;
mod schema;
mod skills;
mod snapshot_store;
mod tool;
pub mod transport;
pub mod ui_tools;
pub mod workflow;

pub use mcp::{MAX_REQUEST_BYTES, McpServer, PROTOCOL_VERSIONS, ToolProvider, notification};
pub use schema::{
    JsonSchema, JsonSchemaScalar, JsonSchemaType, JsonSchemaValidator, JsonSchemaViolation,
};
pub use skills::{AgentSkill, AgentSkillLibrary};
pub use tool::{
    AgentError, AgentImage, AgentMessage, AgentModel, AgentReply, AgentRole, AgentRun,
    AgentSession, AgentTool, AgentToolCall, AgentToolResult, AgentToolbox, CheckpointToolKey,
    ContextualToolExecutor, MAX_TOOL_DESCRIPTION_BYTES, MAX_TOOL_IMAGE_BYTES, MAX_TOOL_INPUT_BYTES,
    MAX_TOOL_NAME_BYTES, MAX_TOOL_RESULT_BYTES, MAX_TOOLS, SessionApiFormat, ToolExecutionContext,
    ToolExecutor,
};

// Convenience aliases so call sites can use either the `JSON`-prefixed spelling
// from the Swift originals or the Rust-style one.
pub type JSONSchema = JsonSchema;
pub type JSONSchemaType = JsonSchemaType;
pub type JSONSchemaScalar = JsonSchemaScalar;
pub type JSONSchemaViolation = JsonSchemaViolation;
pub type JSONSchemaValidator = JsonSchemaValidator;

fn portable_storage_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}
