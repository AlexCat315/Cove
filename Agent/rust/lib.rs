//! Agent-facing contracts, tool loops, skills and MCP.
mod mcp;
mod schema;
mod skills;
mod tool;

pub use mcp::{McpServer, PROTOCOL_VERSIONS};
pub use schema::{
    JsonSchema, JsonSchemaScalar, JsonSchemaType, JsonSchemaValidator, JsonSchemaViolation,
};
pub use skills::{AgentSkill, AgentSkillLibrary};
pub use tool::{
    AgentError, AgentMessage, AgentReply, AgentRole, AgentRun, AgentSession, AgentTool,
    AgentToolCall, AgentToolResult, AgentToolbox, SessionApiFormat, ToolExecutor,
};

// Convenience aliases so call sites can use either the `JSON`-prefixed spelling
// from the Swift originals or the Rust-style one.
pub type JSONSchema = JsonSchema;
pub type JSONSchemaType = JsonSchemaType;
pub type JSONSchemaScalar = JsonSchemaScalar;
pub type JSONSchemaViolation = JsonSchemaViolation;
pub type JSONSchemaValidator = JsonSchemaValidator;
