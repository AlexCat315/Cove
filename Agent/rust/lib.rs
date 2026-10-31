//! UI-free agent foundation: JSON Schema subset, tools, the provider-neutral
//! tool loop, skills, MCP and its line transports.
//!
//! Depends only on serde; UI hosts add their handlers on top (`cove-devtools`).
mod mcp;
mod schema;
mod skills;
mod tool;
pub mod transport;
pub mod ui_tools;

pub use mcp::{MAX_REQUEST_BYTES, McpServer, PROTOCOL_VERSIONS, ToolProvider, notification};
pub use schema::{
    JsonSchema, JsonSchemaScalar, JsonSchemaType, JsonSchemaValidator, JsonSchemaViolation,
};
pub use skills::{AgentSkill, AgentSkillLibrary};
pub use tool::{
    AgentError, AgentMessage, AgentModel, AgentReply, AgentRole, AgentRun, AgentSession, AgentTool,
    AgentToolCall, AgentToolResult, AgentToolbox, MAX_TOOL_DESCRIPTION_BYTES, MAX_TOOL_INPUT_BYTES,
    MAX_TOOL_NAME_BYTES, MAX_TOOL_RESULT_BYTES, MAX_TOOLS, SessionApiFormat, ToolExecutor,
};

// Convenience aliases so call sites can use either the `JSON`-prefixed spelling
// from the Swift originals or the Rust-style one.
pub type JSONSchema = JsonSchema;
pub type JSONSchemaType = JsonSchemaType;
pub type JSONSchemaScalar = JsonSchemaScalar;
pub type JSONSchemaViolation = JsonSchemaViolation;
pub type JSONSchemaValidator = JsonSchemaValidator;
