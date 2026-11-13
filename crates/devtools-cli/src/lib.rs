//! Cove DevTools controller and its MCP client.
mod app_size;
mod controller;
mod debugger;
mod deep_links;
mod http;
mod registry;
mod tools;

pub use http::{RunningController, inspector_url, serve};
pub use registry::random_id;
pub use tools::definitions;

use base64::{Engine, engine::general_purpose::STANDARD};
use cove_agent::{AgentError, AgentToolResult, AgentToolbox, McpServer};
use cove_devtools_protocol::control::ConnectionInfo;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

fn failure(message: impl Into<String>) -> AgentError {
    AgentError::InvalidConfiguration(message.into())
}

fn local_http(endpoint: &str) -> bool {
    let Ok(uri) = endpoint.parse::<ureq::http::Uri>() else {
        return false;
    };
    uri.scheme_str() == Some("http")
        && matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]"))
        && uri
            .authority()
            .is_some_and(|authority| !authority.as_str().contains('@'))
        && uri.query().is_none()
}

pub struct Client {
    endpoint: Option<String>,
    token: Option<String>,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(endpoint: Option<String>, token: Option<String>) -> Result<Self, AgentError> {
        if endpoint
            .as_ref()
            .is_some_and(|endpoint| !local_http(endpoint))
        {
            return Err(failure(
                "Use the local HTTP endpoint and token printed by cove-devtools serve.",
            ));
        }
        Ok(Self {
            endpoint,
            token,
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(30)))
                .proxy(None)
                .max_redirects(0)
                .build()
                .into(),
        })
    }

    fn connection(&self) -> Result<ConnectionInfo, AgentError> {
        let info = ConnectionInfo::resolve(self.endpoint.as_deref(), self.token.as_deref())
            .map_err(|error| failure(error.message))?;
        if !local_http(&info.endpoint) {
            return Err(failure(
                "The controller address must be a local HTTP endpoint.",
            ));
        }
        Ok(info)
    }

    pub fn call(&self, name: &str, args: &Value) -> Result<AgentToolResult, AgentError> {
        let info = self.connection()?;
        let mut response = self.agent.post(&format!("{}/rpc", info.endpoint.trim_end_matches('/')))
            .header("Authorization", &format!("Bearer {}", info.token))
            .send_json(json!({"jsonrpc":"2.0","id":1,"method":name,"params":args}))
            .map_err(|_| failure("Cannot call the DevTools controller. Check cove-devtools serve and its token. An interrupted action may already have run."))?;
        let value: Value = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_json()
            .map_err(|_| failure("Invalid controller response."))?;
        if let Some(error) = value.get("error") {
            return Ok(AgentToolResult::encode(error, true));
        }
        let mut data = value
            .get("result")
            .cloned()
            .ok_or_else(|| failure("Missing controller result."))?;
        let mut image = None;
        if matches!(name, "capture_ui" | "capture_ui_node") {
            let resource = data["resource"].as_str().unwrap_or_default();
            match self.image(resource).and_then(|bytes| attachment(&bytes)) {
                Ok((bytes, mime, width, height)) => {
                    data["attachment"] =
                        json!({"width": width, "height": height, "mime_type": mime});
                    image = Some((bytes, mime));
                }
                Err(error) => data["attachment_error"] = json!(error.to_string()),
            }
        }
        let result = AgentToolResult::encode(&data, false);
        Ok(match image {
            Some((bytes, mime)) => result.with_image(STANDARD.encode(bytes), mime),
            None => result,
        })
    }

    pub fn image(&self, resource: &str) -> Result<Vec<u8>, AgentError> {
        let id = resource.strip_prefix("/captures/").unwrap_or_default();
        if id.len() != 48 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(failure("Invalid capture resource."));
        }
        let info = self.connection()?;
        self.agent
            .get(&format!(
                "{}/captures/{id}",
                info.endpoint.trim_end_matches('/')
            ))
            .header("Authorization", &format!("Bearer {}", info.token))
            .call()
            .map_err(|_| failure("The capture resource is unavailable."))?
            .body_mut()
            .with_config()
            .limit(32 * 1024 * 1024)
            .read_to_vec()
            .map_err(|_| failure("Cannot read the capture resource."))
    }
}

fn attachment(png: &[u8]) -> Result<(Vec<u8>, &'static str, u32, u32), AgentError> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|_| failure("Invalid PNG from controller."))?;
    let limit = cove_agent::MAX_TOOL_IMAGE_BYTES / 4 * 3;
    if png.len() <= limit {
        return Ok((png.to_vec(), "image/png", image.width(), image.height()));
    }
    let mut image = image.thumbnail(1920, 1920);
    loop {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 82)
            .encode_image(&image)
            .map_err(|_| failure("Cannot encode a capture attachment."))?;
        if bytes.len() <= limit {
            return Ok((bytes, "image/jpeg", image.width(), image.height()));
        }
        image = image.thumbnail(
            (image.width() * 3 / 4).max(1),
            (image.height() * 3 / 4).max(1),
        );
    }
}

pub fn toolbox(client: Arc<Client>) -> Result<AgentToolbox, AgentError> {
    AgentToolbox::new(
        definitions(),
        Arc::new(move |name, args| client.call(name, args)),
    )
}

pub fn mcp_server(client: Arc<Client>) -> Result<McpServer, AgentError> {
    Ok(
        McpServer::new("cove-devtools", env!("CARGO_PKG_VERSION"), toolbox(client)?)
            .with_skills(cove_agent::ui_tools::Tools::skills()),
    )
}
