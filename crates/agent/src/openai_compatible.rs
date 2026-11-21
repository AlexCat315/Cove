//! Blocking adapter for OpenAI-compatible Chat Completions servers.
//! Construct it with the server's API base, e.g. `http://127.0.0.1:11434/v1`.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use cove_http::{Client, ClientConfig, Request, Url};
use serde_json::{Value, json};

use crate::{
    AgentError, AgentMessage, AgentModel, AgentReply, AgentRole, AgentTool, AgentToolCall,
    SessionApiFormat,
};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// A configured model. Credentials remain in memory and are excluded from
/// Cove network recordings. Run the adapter off the UI thread.
pub struct ChatCompletionsModel {
    client: Client,
    endpoint: String,
    model: String,
    api_key: Option<String>,
}

impl ChatCompletionsModel {
    pub fn new(base_url: &str, model: &str, api_key: Option<&str>) -> Result<Self, AgentError> {
        let base_url = api_base(base_url)?;
        let model = model.trim();
        if model.is_empty() || model.len() > 512 || model.chars().any(char::is_control) {
            return Err(AgentError::InvalidConfiguration(
                "Enter a model name.".to_owned(),
            ));
        }
        Ok(Self {
            client: client(Duration::from_secs(120)),
            endpoint: format!("{base_url}/chat/completions"),
            model: model.to_owned(),
            api_key: validated_key(api_key)?.map(str::to_owned),
        })
    }

    /// Lists model IDs advertised by an OpenAI-compatible server.
    /// Some servers omit this endpoint. Callers can still configure a model manually.
    pub fn list_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<String>, AgentError> {
        let base = api_base(base_url)?;
        let body = fetch_json(
            &client(Duration::from_secs(20)),
            Request::get(format!("{base}/models")),
            validated_key(api_key)?,
        )?;
        let data = body["data"].as_array().ok_or(AgentError::InvalidResult)?;
        let models = data
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .filter(|id| {
                !id.trim().is_empty() && id.len() <= 512 && !id.chars().any(char::is_control)
            })
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        Ok(models.into_iter().collect())
    }

    pub fn into_agent_model(self) -> AgentModel {
        Arc::new(move |messages, tools| self.complete(messages, tools))
    }

    pub fn complete(
        &self,
        messages: &[AgentMessage],
        tools: &[AgentTool],
    ) -> Result<AgentReply, AgentError> {
        let messages: Vec<Value> = messages.iter().map(encode_message).collect();
        let mut body = json!({ "model": self.model, "messages": messages, "stream": false });
        if !tools.is_empty() {
            body["tools"] = Value::Array(
                tools
                    .iter()
                    .map(|tool| tool.provider_definition(SessionApiFormat::OpenAiCompatible))
                    .collect(),
            );
        }
        let request = Request::post(
            &self.endpoint,
            serde_json::to_vec(&body).map_err(|_| AgentError::InvalidResult)?,
        )
        .header("Content-Type", "application/json");
        decode_reply(&fetch_json(&self.client, request, self.api_key.as_deref())?)
    }
}

/// Validates the API base and accepts a complete Chat Completions URL as input.
/// The resulting base also identifies the scope of credentials for this API.
pub fn api_base(base_url: &str) -> Result<&str, AgentError> {
    let base_url = base_url.trim().trim_end_matches('/');
    let invalid_url = || {
        AgentError::InvalidConfiguration(
            "Enter an HTTP(S) API base URL without credentials, query, or fragment.".to_owned(),
        )
    };
    let url = Url::parse(base_url).map_err(|_| invalid_url())?;
    if base_url.contains(['?', '#', '\\']) {
        return Err(invalid_url());
    }
    if !url.secure
        && url.host != "localhost"
        && !url.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
    {
        return Err(AgentError::InvalidConfiguration(
            "Use HTTPS for model servers outside this device.".to_owned(),
        ));
    }
    Ok(if url.path.ends_with("/chat/completions") {
        base_url
            .strip_suffix("/chat/completions")
            .unwrap_or(base_url)
    } else {
        base_url
    })
}

fn validated_key(api_key: Option<&str>) -> Result<Option<&str>, AgentError> {
    let api_key = api_key.map(str::trim).filter(|key| !key.is_empty());
    if api_key
        .is_some_and(|key| key.len() > 8192 || !key.bytes().all(|byte| byte.is_ascii_graphic()))
    {
        return Err(AgentError::InvalidConfiguration(
            "The API key contains invalid characters.".to_owned(),
        ));
    }
    Ok(api_key)
}

fn client(timeout: Duration) -> Client {
    Client::new(ClientConfig {
        timeout,
        max_redirects: 0,
        user_agent: "Cove-Agent/1".to_owned(),
    })
}

fn fetch_json(
    client: &Client,
    request: Request,
    api_key: Option<&str>,
) -> Result<Value, AgentError> {
    let mut request = request.header("Accept", "application/json");
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {key}"));
    }
    let response = client
        .send_unrecorded(request)
        .map_err(|error| AgentError::Model(format!("Could not reach the model server: {error}")))?;
    if !response.is_success() {
        // Some servers echo input and headers in error bodies.
        return Err(AgentError::HttpStatus(response.status));
    }
    let bytes = response.bytes(MAX_RESPONSE_BYTES).map_err(|error| {
        AgentError::Model(format!("Could not read the model response: {error}"))
    })?;
    serde_json::from_slice(&bytes).map_err(|_| AgentError::InvalidResult)
}

fn encode_message(message: &AgentMessage) -> Value {
    let mut value = match message.role {
        AgentRole::System => json!({ "role": "system", "content": message.text }),
        AgentRole::User => json!({ "role": "user", "content": message.text }),
        AgentRole::Assistant if !message.calls.is_empty() => {
            let calls: Vec<Value> = message
                .calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id,
                        "type": "function",
                        "function": { "name": call.name, "arguments": call.arguments.to_string() }
                    })
                })
                .collect();
            json!({ "role": "assistant", "content": if message.text.is_empty() { None } else { Some(&message.text) }, "tool_calls": calls })
        }
        AgentRole::Assistant => json!({ "role": "assistant", "content": message.text }),
        AgentRole::Tool => {
            json!({ "role": "tool", "tool_call_id": message.result_for, "content": message.text })
        }
    };
    if message.role == AgentRole::Assistant
        && let Some(reasoning) = &message.reasoning_content
    {
        value["reasoning_content"] = json!(reasoning);
    }
    value
}

fn decode_reply(body: &Value) -> Result<AgentReply, AgentError> {
    let choice = body["choices"]
        .as_array()
        .and_then(|choices| choices.first())
        .ok_or(AgentError::InvalidResult)?;
    let message = &choice["message"];
    if choice["finish_reason"].as_str() == Some("length") {
        return Err(AgentError::Model(
            "The model stopped before completing its response.".to_owned(),
        ));
    }
    let mut assistant = AgentMessage::text(
        AgentRole::Assistant,
        message["content"].as_str().unwrap_or_default(),
    );
    assistant.reasoning_content = message["reasoning_content"].as_str().map(str::to_owned);
    if let Some(calls) = message["tool_calls"]
        .as_array()
        .filter(|calls| !calls.is_empty())
    {
        assistant.calls = calls
            .iter()
            .map(|call| {
                let function = &call["function"];
                let id = call["id"].as_str().ok_or(AgentError::InvalidResult)?;
                let name = function["name"].as_str().ok_or(AgentError::InvalidResult)?;
                let arguments = function["arguments"]
                    .as_str()
                    .ok_or(AgentError::InvalidResult)?;
                let arguments =
                    serde_json::from_str(arguments).map_err(|_| AgentError::InvalidResult)?;
                Ok(AgentToolCall {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    arguments,
                })
            })
            .collect::<Result<Vec<_>, AgentError>>()?;
    }
    if assistant.calls.is_empty() && assistant.text.trim().is_empty() {
        return Err(AgentError::InvalidResult);
    }
    Ok(AgentReply::Assistant(assistant))
}
