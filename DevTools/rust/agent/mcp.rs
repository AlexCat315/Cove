//! Transport-neutral MCP endpoint.
//!
//! Stdio, sockets and embedded hosts pass JSON bytes in and receive response
//! bytes out; the server owns no globals and no process, so several can coexist.
use crate::agent::skills::AgentSkillLibrary;
use crate::agent::tool::{AgentError, AgentTool, AgentToolResult, AgentToolbox};
use serde_json::{Value, json};

pub const MAX_REQUEST_BYTES: usize = 1_048_576;

/// Supported MCP protocol revisions, newest first.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub struct McpServer {
    pub name: String,
    pub version: String,
    toolbox: AgentToolbox,
    skills: AgentSkillLibrary,
    tools_may_change: bool,
}

impl McpServer {
    pub fn new(name: impl Into<String>, version: impl Into<String>, toolbox: AgentToolbox) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            toolbox,
            skills: AgentSkillLibrary::default(),
            tools_may_change: false,
        }
    }

    pub fn with_skills(mut self, skills: AgentSkillLibrary) -> Self {
        self.skills = skills;
        self
    }

    /// Returns `None` for notifications (a request without an `id`).
    pub fn respond(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() > MAX_REQUEST_BYTES {
            return Some(invalid_request(json!(null), "Request exceeds 1 MiB."));
        }
        let Ok(value) = serde_json::from_slice::<Value>(data) else {
            return Some(invalid_request(json!(null), "Invalid JSON."));
        };
        let Some(request) = value.as_object() else {
            return Some(invalid_request(json!(null), "Invalid JSON-RPC request."));
        };
        let raw_id = request.get("id").cloned().unwrap_or(Value::Null);
        // JSON-RPC ids are strings or numbers; anything else is rejected.
        let valid_id = raw_id.is_null() || raw_id.is_string()
            || raw_id.as_i64().is_some()
            || raw_id.as_u64().is_some()
            || raw_id.as_f64().is_some();
        let jsonrpc_ok = request.get("jsonrpc").and_then(Value::as_str) == Some("2.0");
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return Some(invalid_request(
                if valid_id { raw_id } else { json!(null) },
                "Invalid JSON-RPC request.",
            ));
        };
        if !jsonrpc_ok {
            return Some(invalid_request(
                if valid_id { raw_id } else { json!(null) },
                "Invalid JSON-RPC request.",
            ));
        }
        if !request.contains_key("id") {
            return None;
        }
        if !valid_id {
            return Some(invalid_request(json!(null), "Invalid request ID."));
        }
        let id = raw_id;
        let params = match request.get("params") {
            None | Some(Value::Null) => json!({}),
            Some(Value::Object(_)) => request["params"].clone(),
            Some(_) => return Some(error(&id, -32602, "Parameters must be an object.")),
        };
        Some(self.dispatch(method, &params, &id))
    }

    fn dispatch(&self, method: &str, params: &Value, id: &Value) -> Vec<u8> {
        match method {
            "initialize" => {
                let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
                let version = if PROTOCOL_VERSIONS.contains(&requested) { requested } else { PROTOCOL_VERSIONS[0] };
                response(id, json!({
                    "protocolVersion": version,
                    "serverInfo": { "name": self.name, "version": self.version },
                    "capabilities": {
                        "tools": { "listChanged": self.tools_may_change },
                        "prompts": {}
                    }
                }))
            }
            "ping" => response(id, json!({})),
            "tools/list" => {
                let tools: Vec<Value> = self.toolbox.tools.iter().map(|tool| tool.mcp_definition()).collect();
                response(id, json!({ "tools": tools }))
            }
            "tools/call" => {
                let (Some(name), true) = (
                    params.get("name").and_then(Value::as_str),
                    params.get("arguments").is_none_or(|value| value.is_object()),
                ) else {
                    return error(id, -32602, "tools/call requires a name and object arguments.");
                };
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match self.toolbox.execute(name, &arguments) {
                    Ok(result) => {
                        let text = String::from_utf8_lossy(&result.json).to_string();
                        let mut payload = json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": result.is_error
                        });
                        if let Some(value) = result.value()
                            && value.is_object()
                        {
                            payload["structuredContent"] = value;
                        }
                        response(id, payload)
                    }
                    Err(error) => {
                        let message = match &error {
                            AgentError::InvalidConfiguration(message) => message.clone(),
                            other => other.message(),
                        };
                        response(id, json!({
                            "content": [{ "type": "text", "text": truncate(&message) }],
                            "isError": true
                        }))
                    }
                }
            }
            "prompts/list" => {
                let prompts: Vec<Value> = self
                    .skills
                    .skills
                    .iter()
                    .map(|skill| json!({ "name": skill.name, "description": skill.description }))
                    .collect();
                response(id, json!({ "prompts": prompts }))
            }
            "prompts/get" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                match self.skills.read(name) {
                    Ok(skill) => response(id, json!({
                        "description": skill.description,
                        "messages": [{
                            "role": "user",
                            "content": { "type": "text", "text": skill.instructions }
                        }]
                    })),
                    Err(error) => error(id, -32602, &error.message()),
                }
            }
            other => error(id, -32601, &format!("Method not found: {other}")),
        }
    }

    /// Tools available on this server, for a host that wants to render them.
    pub fn tools(&self) -> &[AgentTool] {
        &self.toolbox.tools
    }
}

fn truncate(message: &str) -> String {
    message.chars().take(4096).collect()
}

fn response(id: &Value, result: Value) -> Vec<u8> {
    encode(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: &Value, code: i64, message: &str) -> Vec<u8> {
    encode(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }))
}

fn invalid_request(id: Value, message: &str) -> Vec<u8> {
    error(&id, -32600, message)
}

fn encode(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec())
}

/// Helper for hosts that want the text content of a tool result.
pub fn text_content(result: &AgentToolResult) -> String {
    String::from_utf8_lossy(&result.json).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::schema::JsonSchema;
    use std::rc::Rc;

    fn server() -> McpServer {
        let toolbox = AgentToolbox::new(
            vec![AgentTool::new("echo", "echo back", JsonSchema::any(), true)],
            Rc::new(|_name, input| Ok(AgentToolResult::encode(input, false))),
        )
        .unwrap();
        McpServer::new("test", "1.0", toolbox)
    }

    fn parse(bytes: &[u8]) -> Value {
        serde_json::from_slice(bytes).unwrap()
    }

    #[test]
    fn notifications_get_no_response() {
        assert!(server().respond(br#"{"jsonrpc":"2.0","method":"ping"}"#).is_none());
    }

    #[test]
    fn initialize_negotiates_a_known_version() {
        let response = parse(&server().respond(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        )
        .unwrap());
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        let unknown = parse(&server().respond(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"x"}}"#,
        )
        .unwrap());
        assert_eq!(unknown["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn malformed_and_unknown_requests_are_errors() {
        assert_eq!(parse(&server().respond(b"not json").unwrap())["error"]["code"], -32700);
        assert_eq!(
            parse(&server().respond(br#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#).unwrap())["error"]["code"],
            -32601
        );
        assert_eq!(
            parse(&server().respond(br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[]}"#).unwrap())
                ["error"]["code"],
            -32602
        );
    }

    #[test]
    fn tools_list_and_call_round_trip() {
        let listed = parse(&server().respond(br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap());
        assert_eq!(listed["result"]["tools"][0]["name"], "echo");
        let called = parse(
            &server()
                .respond(br#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"x":1}}}"#)
                .unwrap(),
        );
        assert_eq!(called["result"]["isError"], false);
        assert_eq!(called["result"]["structuredContent"]["x"], 1);
    }
}
