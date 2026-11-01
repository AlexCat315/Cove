//! Transport-neutral MCP endpoint.
//!
//! Stdio, sockets and embedded hosts pass JSON bytes in and receive response
//! bytes out; the server owns no globals and no process, so several can coexist.
use crate::skills::AgentSkillLibrary;
use crate::tool::{AgentError, AgentTool, AgentToolbox, ToolExecutor};
use serde_json::{Value, json};
use std::sync::Arc;

pub const MAX_REQUEST_BYTES: usize = 1_048_576;

/// Supported MCP protocol revisions, newest first.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Tool list re-read on every `tools/list` and `tools/call`.
pub type ToolProvider = Arc<dyn Fn() -> Result<Vec<AgentTool>, AgentError> + Send + Sync>;

enum Tools {
    Fixed(AgentToolbox),
    Provided {
        list: ToolProvider,
        execute: ToolExecutor,
    },
}

pub struct McpServer {
    pub name: String,
    pub version: String,
    tools: Tools,
    skills: AgentSkillLibrary,
}

impl McpServer {
    pub fn new(name: impl Into<String>, version: impl Into<String>, toolbox: AgentToolbox) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tools: Tools::Fixed(toolbox),
            skills: AgentSkillLibrary::default(),
        }
    }

    /// A server whose tools may change; it advertises `listChanged` and the
    /// host emits [`notification`]`("notifications/tools/list_changed")`.
    pub fn with_provider(
        name: impl Into<String>,
        version: impl Into<String>,
        skills: AgentSkillLibrary,
        tools: impl Fn() -> Result<Vec<AgentTool>, AgentError> + Send + Sync + 'static,
        execute: impl Fn(&str, &Value) -> Result<crate::tool::AgentToolResult, AgentError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tools: Tools::Provided {
                list: Arc::new(tools),
                execute: Arc::new(execute),
            },
            skills,
        }
    }

    pub fn with_skills(mut self, skills: AgentSkillLibrary) -> Self {
        self.skills = skills;
        self
    }

    pub fn tools_may_change(&self) -> bool {
        matches!(self.tools, Tools::Provided { .. })
    }

    /// The current toolbox: the fixed one, or the provider's list validated.
    pub fn toolbox(&self) -> Result<AgentToolbox, AgentError> {
        match &self.tools {
            Tools::Fixed(toolbox) => Ok(toolbox.clone()),
            Tools::Provided { list, execute } => AgentToolbox::new(list()?, execute.clone()),
        }
    }

    /// Returns `None` for notifications (a request without an `id`).
    pub fn respond(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() > MAX_REQUEST_BYTES {
            return Some(invalid_request(json!(null), "Request exceeds 1 MiB."));
        }
        let Ok(value) = serde_json::from_slice::<Value>(data) else {
            // JSON-RPC 2.0 reserves -32700 for a body that is not JSON at all;
            // -32600 means "valid JSON, but not a valid Request object".
            return Some(error(&json!(null), -32700, "Parse error."));
        };
        let Some(request) = value.as_object() else {
            return Some(invalid_request(json!(null), "Invalid JSON-RPC request."));
        };
        let raw_id = request.get("id").cloned().unwrap_or(Value::Null);
        // JSON-RPC ids are strings or numbers; anything else is rejected.
        let valid_id = raw_id.is_null()
            || raw_id.is_string()
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
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let version = if PROTOCOL_VERSIONS.contains(&requested) {
                    requested
                } else {
                    PROTOCOL_VERSIONS[0]
                };
                response(
                    id,
                    json!({
                        "protocolVersion": version,
                        "serverInfo": { "name": self.name, "version": self.version },
                        "capabilities": {
                            "tools": { "listChanged": self.tools_may_change() },
                            "prompts": {}
                        }
                    }),
                )
            }
            "ping" => response(id, json!({})),
            "tools/list" => match self.toolbox() {
                Ok(toolbox) => {
                    let tools: Vec<Value> = toolbox
                        .tools
                        .iter()
                        .map(AgentTool::mcp_definition)
                        .collect();
                    response(id, json!({ "tools": tools }))
                }
                Err(failure) => error(id, -32602, &truncate(&failure.to_string())),
            },
            "tools/call" => {
                let (Some(name), true) = (
                    params.get("name").and_then(Value::as_str),
                    params
                        .get("arguments")
                        .is_none_or(|value| value.is_object()),
                ) else {
                    return error(
                        id,
                        -32602,
                        "tools/call requires a name and object arguments.",
                    );
                };
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let toolbox = match self.toolbox() {
                    Ok(toolbox) => toolbox,
                    Err(failure) => return error(id, -32602, &truncate(&failure.to_string())),
                };
                match toolbox.execute(name, &arguments) {
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
                        let message = error.to_string();
                        response(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": truncate(&message) }],
                                "isError": true
                            }),
                        )
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
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match self.skills.read(name) {
                    Ok(skill) => response(
                        id,
                        json!({
                            "description": skill.description,
                            "messages": [{
                                "role": "user",
                                "content": { "type": "text", "text": skill.instructions }
                            }]
                        }),
                    ),
                    Err(failure) => error(id, -32602, &failure.to_string()),
                }
            }
            other => error(id, -32601, &format!("Method not found: {other}")),
        }
    }

    /// Tools available on this server, for a host that wants to render them.
    pub fn tools(&self) -> Result<Vec<AgentTool>, AgentError> {
        self.toolbox().map(|toolbox| toolbox.tools)
    }
}

/// A JSON-RPC notification line, e.g. `notifications/tools/list_changed`.
pub fn notification(method: &str, params: Option<Value>) -> Vec<u8> {
    let mut value = json!({ "jsonrpc": "2.0", "method": method });
    if let Some(params) = params {
        value["params"] = params;
    }
    encode(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::JsonSchema;
    use crate::tool::AgentToolResult;
    use std::sync::Arc;

    fn server() -> McpServer {
        let toolbox = AgentToolbox::new(
            vec![AgentTool::new("echo", "echo back", JsonSchema::any(), true)],
            Arc::new(|_name, input| Ok(AgentToolResult::encode(input, false))),
        )
        .unwrap();
        McpServer::new("test", "1.0", toolbox)
    }

    fn parse(bytes: &[u8]) -> Value {
        serde_json::from_slice(bytes).unwrap()
    }

    #[test]
    fn notifications_get_no_response() {
        assert!(
            server()
                .respond(br#"{"jsonrpc":"2.0","method":"ping"}"#)
                .is_none()
        );
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
        assert_eq!(
            parse(&server().respond(b"not json").unwrap())["error"]["code"],
            -32700
        );
        assert_eq!(
            parse(
                &server()
                    .respond(br#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#)
                    .unwrap()
            )["error"]["code"],
            -32601
        );
        assert_eq!(
            parse(
                &server()
                    .respond(br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[]}"#)
                    .unwrap()
            )["error"]["code"],
            -32602
        );
    }

    #[test]
    fn provider_tools_are_reread_and_advertise_list_changed() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let count = Arc::new(AtomicUsize::new(1));
        let listed = count.clone();
        let server = McpServer::with_provider(
            "dynamic",
            "1",
            AgentSkillLibrary::default(),
            move || {
                Ok((0..listed.load(Ordering::SeqCst))
                    .map(|index| AgentTool::new(format!("t{index}"), "d", JsonSchema::any(), true))
                    .collect())
            },
            |name, _| Ok(AgentToolResult::encode(&json!({ "name": name }), false)),
        );
        let initialized = parse(
            &server
                .respond(br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
                .unwrap(),
        );
        assert_eq!(
            initialized["result"]["capabilities"]["tools"]["listChanged"],
            true
        );
        let list = br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        assert_eq!(
            parse(&server.respond(list).unwrap())["result"]["tools"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        count.store(3, Ordering::SeqCst);
        assert_eq!(
            parse(&server.respond(list).unwrap())["result"]["tools"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let called = parse(
            &server
                .respond(
                    br#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"t2"}}"#,
                )
                .unwrap(),
        );
        assert_eq!(called["result"]["structuredContent"]["name"], "t2");
        assert_eq!(
            parse(&notification("notifications/tools/list_changed", None)),
            json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
        );
    }

    #[test]
    fn invalid_provider_tools_are_parameter_errors() {
        let server = McpServer::with_provider(
            "dynamic",
            "1",
            AgentSkillLibrary::default(),
            || Ok(vec![AgentTool::new("", "d", JsonSchema::any(), true)]),
            |_, input| Ok(AgentToolResult::encode(input, false)),
        );
        let listed = parse(
            &server
                .respond(br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
                .unwrap(),
        );
        assert_eq!(listed["error"]["code"], -32602);
    }

    #[test]
    fn server_moves_to_another_thread() {
        let server = server();
        let response = std::thread::spawn(move || {
            server.respond(br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
        })
        .join()
        .unwrap();
        assert!(response.is_some());
    }

    #[test]
    fn tools_list_and_call_round_trip() {
        let listed = parse(
            &server()
                .respond(br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
                .unwrap(),
        );
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
