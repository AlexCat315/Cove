//! Tool metadata, validation and the provider-neutral tool loop.
use crate::schema::{JsonSchema, JsonSchemaValidator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Wire formats supported by the shared tool contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionApiFormat {
    Anthropic,
    OpenAiResponses,
    OpenAiCompatible,
}

/// Plain tool metadata. Execution stays with the application that owns the data.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub schema: JsonSchema,
    pub read_only: bool,
}

impl AgentTool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: JsonSchema,
        read_only: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            schema,
            read_only,
        }
    }

    pub fn mcp_definition(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.schema.canonical_json(),
            "annotations": { "readOnlyHint": self.read_only, "openWorldHint": false }
        })
    }

    pub fn provider_definition(&self, format: SessionApiFormat) -> Value {
        match format {
            SessionApiFormat::Anthropic => serde_json::json!({
                "name": self.name,
                "description": self.description,
                "input_schema": self.schema.canonical_json()
            }),
            SessionApiFormat::OpenAiCompatible => serde_json::json!({
                "type": "function",
                "function": {
                    "name": self.name,
                    "description": self.description,
                    "parameters": self.schema.canonical_json()
                }
            }),
            SessionApiFormat::OpenAiResponses => serde_json::json!({
                "type": "function",
                "name": self.name,
                "description": self.description,
                "parameters": self.schema.canonical_json(),
                "strict": false
            }),
        }
    }
}

/// One tool result: bounded JSON plus an error flag.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentToolResult {
    pub json: Vec<u8>,
    pub is_error: bool,
    pub images: Vec<AgentImage>,
}

/// Image content is kept out of the JSON/text result so MCP clients can see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentImage {
    pub data: String,
    pub mime_type: String,
}

impl AgentToolResult {
    pub fn new(json: Vec<u8>, is_error: bool) -> Self {
        Self {
            json,
            is_error,
            images: Vec::new(),
        }
    }

    pub fn encode(value: &Value, is_error: bool) -> Self {
        Self {
            json: serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec()),
            is_error,
            images: Vec::new(),
        }
    }

    pub fn with_image(mut self, data: String, mime_type: impl Into<String>) -> Self {
        self.images.push(AgentImage {
            data,
            mime_type: mime_type.into(),
        });
        self
    }

    pub fn value(&self) -> Option<Value> {
        serde_json::from_slice(&self.json).ok()
    }
}

/// Errors shared by tool execution, MCP and the agent session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentError {
    InvalidConfiguration(String),
    UnknownTool(String),
    InvalidResult,
    StepLimit,
}

impl AgentError {
    pub fn message(&self) -> String {
        match self {
            Self::InvalidConfiguration(message) => message.clone(),
            Self::UnknownTool(name) => format!("Unknown tool: {name}"),
            Self::InvalidResult => "Tool returned invalid JSON/media or exceeded the result limit (JSON: 512 KiB; images: 1 MiB encoded).".into(),
            Self::StepLimit => "Agent reached its tool round limit.".into(),
        }
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for AgentError {}

/// Upper bounds shared by every entry point.
pub const MAX_TOOLS: usize = 256;
pub const MAX_TOOL_INPUT_BYTES: usize = 262_144;
pub const MAX_TOOL_RESULT_BYTES: usize = 524_288;
pub const MAX_TOOL_IMAGE_BYTES: usize = 1_048_576;
pub const MAX_TOOL_NAME_BYTES: usize = 128;
pub const MAX_TOOL_DESCRIPTION_BYTES: usize = 16_384;

/// Executor signature. Synchronous by design: diagnostic reads run inside a
/// native host's frame inbox, which cannot yield. `Send + Sync` so a toolbox can
/// move to a stdio or socket server thread.
pub type ToolExecutor =
    Arc<dyn Fn(&str, &Value) -> Result<AgentToolResult, AgentError> + Send + Sync>;

/// Model adapter for one turn of [`AgentSession`].
pub type AgentModel =
    Arc<dyn Fn(&[AgentMessage], &[AgentTool]) -> Result<AgentReply, AgentError> + Send + Sync>;

/// One validated boundary shared by model sessions and MCP clients.
#[derive(Clone)]
pub struct AgentToolbox {
    pub tools: Vec<AgentTool>,
    executor: ToolExecutor,
}

impl AgentToolbox {
    pub fn new(tools: Vec<AgentTool>, executor: ToolExecutor) -> Result<Self, AgentError> {
        let mut names = std::collections::BTreeSet::new();
        if tools.len() > MAX_TOOLS {
            return Err(AgentError::InvalidConfiguration(
                "Tool names must be unique and bounded; at most 256 tools are supported.".into(),
            ));
        }
        for tool in &tools {
            if tool.name.is_empty()
                || tool.name.len() > MAX_TOOL_NAME_BYTES
                || tool.description.len() > MAX_TOOL_DESCRIPTION_BYTES
                || !names.insert(tool.name.clone())
            {
                return Err(AgentError::InvalidConfiguration(
                    "Tool names must be unique and bounded; at most 256 tools are supported."
                        .into(),
                ));
            }
        }
        Ok(Self { tools, executor })
    }

    pub fn execute(&self, name: &str, input: &Value) -> Result<AgentToolResult, AgentError> {
        let Some(tool) = self.tools.iter().find(|tool| tool.name == name) else {
            return Err(AgentError::UnknownTool(name.to_string()));
        };
        let input_bytes = serde_json::to_vec(input).map_err(|_| AgentError::InvalidResult)?;
        if input_bytes.len() > MAX_TOOL_INPUT_BYTES {
            return Err(AgentError::InvalidConfiguration(
                "Tool input exceeds 256 KiB.".into(),
            ));
        }
        JsonSchemaValidator::validate(input, &tool.schema)
            .map_err(|violation| AgentError::InvalidConfiguration(violation.to_string()))?;
        let result = (self.executor)(name, input)?;
        if result.json.len() > MAX_TOOL_RESULT_BYTES
            || serde_json::from_slice::<Value>(&result.json).is_err()
            || result.images.len() > 4
            || result
                .images
                .iter()
                .map(|image| image.data.len())
                .sum::<usize>()
                > MAX_TOOL_IMAGE_BYTES
            || result.images.iter().any(|image| {
                image.data.is_empty()
                    || !matches!(
                        image.mime_type.as_str(),
                        "image/jpeg" | "image/png" | "image/webp"
                    )
                    || image
                        .data
                        .bytes()
                        .any(|byte| !byte.is_ascii_alphanumeric() && !b"+/=".contains(&byte))
            })
        {
            return Err(AgentError::InvalidResult);
        }
        Ok(result)
    }
}

impl std::fmt::Debug for AgentToolbox {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentToolbox")
            .field("tools", &self.tools.len())
            .finish()
    }
}

/// One tool invocation requested by the model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: AgentRole,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<AgentToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_for: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl AgentMessage {
    pub fn new(role: AgentRole) -> Self {
        Self {
            role,
            text: String::new(),
            calls: Vec::new(),
            result_for: None,
            is_error: false,
        }
    }
    pub fn text(role: AgentRole, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::new(role)
        }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self::text(AgentRole::User, text)
    }
}

/// What the model adapter returned for one turn.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentReply {
    Answer(String),
    Tools(Vec<AgentToolCall>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentRun {
    pub answer: String,
    pub messages: Vec<AgentMessage>,
    pub tool_rounds: usize,
}

/// Provider-neutral tool loop.
pub struct AgentSession {
    toolbox: AgentToolbox,
    instructions: String,
    maximum_tool_rounds: usize,
    model: AgentModel,
}

impl AgentSession {
    pub fn new(
        toolbox: AgentToolbox,
        skills: &crate::skills::AgentSkillLibrary,
        enabled_skills: &[String],
        maximum_tool_rounds: usize,
        model: AgentModel,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            instructions: skills.instructions(enabled_skills)?,
            maximum_tool_rounds: maximum_tool_rounds.clamp(1, 32),
            toolbox,
            model,
        })
    }

    pub fn run(&self, input: Vec<AgentMessage>) -> Result<AgentRun, AgentError> {
        let mut messages = input;
        if !self.instructions.is_empty() {
            messages.insert(
                0,
                AgentMessage::text(AgentRole::System, self.instructions.clone()),
            );
        }
        for round in 0..=self.maximum_tool_rounds {
            if messages.len() > 256
                || messages
                    .iter()
                    .map(|message| message.text.len())
                    .sum::<usize>()
                    > 1_048_576
            {
                return Err(AgentError::InvalidConfiguration(
                    "Conversation exceeds the message or byte limit.".into(),
                ));
            }
            let reply = (self.model)(&messages, &self.toolbox.tools)?;
            match reply {
                AgentReply::Answer(text) => {
                    if text.len() > 262_144 {
                        return Err(AgentError::InvalidResult);
                    }
                    messages.push(AgentMessage::text(AgentRole::Assistant, text.clone()));
                    return Ok(AgentRun {
                        answer: text,
                        messages,
                        tool_rounds: round,
                    });
                }
                AgentReply::Tools(calls) => {
                    if round == self.maximum_tool_rounds {
                        return Err(AgentError::StepLimit);
                    }
                    let mut ids = std::collections::BTreeSet::new();
                    if calls.is_empty()
                        || calls.len() > 16
                        || !calls
                            .iter()
                            .all(|call| !call.id.is_empty() && call.id.len() <= 128)
                        || !calls.iter().all(|call| ids.insert(call.id.clone()))
                    {
                        return Err(AgentError::InvalidConfiguration(
                            "Model returned invalid tool calls.".into(),
                        ));
                    }
                    let mut assistant = AgentMessage::new(AgentRole::Assistant);
                    assistant.calls = calls.clone();
                    messages.push(assistant);
                    for call in calls {
                        let result = self.toolbox.execute(&call.name, &call.arguments);
                        let (text, is_error) = match result {
                            Ok(result) => (
                                String::from_utf8_lossy(&result.json).to_string(),
                                result.is_error,
                            ),
                            Err(error) => (
                                serde_json::json!({ "error": error.message() }).to_string(),
                                true,
                            ),
                        };
                        let mut message = AgentMessage::text(AgentRole::Tool, text);
                        message.result_for = Some(call.id.clone());
                        message.is_error = is_error;
                        messages.push(message);
                    }
                }
            }
        }
        Err(AgentError::StepLimit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toolbox() -> AgentToolbox {
        let mut properties = std::collections::BTreeMap::new();
        properties.insert(
            "limit".to_string(),
            JsonSchema::integer(Some(1.0), Some(10.0)),
        );
        let tools = vec![AgentTool::new(
            "query",
            "a query",
            JsonSchema::object(properties, Vec::new()),
            true,
        )];
        AgentToolbox::new(
            tools,
            Arc::new(|_name, _input| {
                Ok(AgentToolResult::encode(
                    &serde_json::json!({"ok": true}),
                    false,
                ))
            }),
        )
        .unwrap()
    }

    #[test]
    fn duplicate_tool_names_are_rejected() {
        let tool = AgentTool::new("a", "d", JsonSchema::any(), true);
        let result = AgentToolbox::new(
            vec![tool.clone(), tool],
            Arc::new(|_, _| Ok(AgentToolResult::encode(&serde_json::json!(null), false))),
        );
        assert!(result.is_err());
    }

    #[test]
    fn tool_input_is_validated_before_execution() {
        let toolbox = toolbox();
        assert!(
            toolbox
                .execute("query", &serde_json::json!({"limit": 5}))
                .is_ok()
        );
        // Out of range.
        assert!(
            toolbox
                .execute("query", &serde_json::json!({"limit": 50}))
                .is_err()
        );
        // Unknown property.
        assert!(
            toolbox
                .execute("query", &serde_json::json!({"nope": 1}))
                .is_err()
        );
        // Unknown tool.
        assert!(matches!(
            toolbox.execute("nope", &serde_json::json!({})),
            Err(AgentError::UnknownTool(_))
        ));
    }

    #[test]
    fn session_rejects_duplicate_call_ids() {
        let session = AgentSession::new(
            toolbox(),
            &crate::skills::AgentSkillLibrary::default(),
            &[],
            4,
            Arc::new(|_messages, _tools| {
                Ok(AgentReply::Tools(vec![
                    AgentToolCall {
                        id: "a".into(),
                        name: "query".into(),
                        arguments: serde_json::json!({}),
                    },
                    AgentToolCall {
                        id: "a".into(),
                        name: "query".into(),
                        arguments: serde_json::json!({}),
                    },
                ]))
            }),
        )
        .unwrap();
        assert!(session.run(Vec::new()).is_err());
    }

    #[test]
    fn step_limit_is_enforced() {
        let session = AgentSession::new(
            toolbox(),
            &crate::skills::AgentSkillLibrary::default(),
            &[],
            1,
            Arc::new(|_messages, _tools| {
                Ok(AgentReply::Tools(vec![AgentToolCall {
                    id: "a".into(),
                    name: "query".into(),
                    arguments: serde_json::json!({}),
                }]))
            }),
        )
        .unwrap();
        assert_eq!(session.run(Vec::new()), Err(AgentError::StepLimit));
    }
}
