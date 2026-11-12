//! The controller and runtime probe share these transport-independent messages.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 2 * 1024 * 1024;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub endpoint: String,
    pub token: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl ConnectionInfo {
    pub fn path() -> Option<std::path::PathBuf> {
        use std::{env, path::PathBuf};
        if let Some(path) = env::var_os("COVE_DEVTOOLS_SESSION_FILE") {
            return Some(path.into());
        }
        #[cfg(windows)]
        let root = PathBuf::from(env::var_os("LOCALAPPDATA")?).join("Cove");
        #[cfg(target_os = "macos")]
        let root = PathBuf::from(env::var_os("HOME")?).join("Library/Application Support/Cove");
        #[cfg(not(any(windows, target_os = "macos")))]
        let root = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?
            .join("cove");
        Some(root.join("devtools-controller.json"))
    }

    pub fn local() -> Option<Self> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(Self::path()?)
            .ok()?
            .take(4097)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 4096 {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    }

    pub fn resolve(endpoint: Option<&str>, token: Option<&str>) -> Result<Self, Fault> {
        let local = Self::local();
        let endpoint = endpoint
            .map(str::to_owned)
            .or_else(|| std::env::var("COVE_DEVTOOLS_ENDPOINT").ok())
            .or_else(|| local.as_ref().map(|info| info.endpoint.clone()))
            .unwrap_or_else(|| "http://127.0.0.1:9229".to_owned());
        let token = token
            .map(str::to_owned)
            .or_else(|| std::env::var("COVE_DEVTOOLS_TOKEN").ok())
            .or_else(|| {
                local
                    .filter(|info| info.endpoint == endpoint)
                    .map(|info| info.token)
            })
            .filter(|token| !token.is_empty() && token.len() <= 256)
            .ok_or_else(|| {
                Fault::new(
                    "controller_unavailable",
                    "Start cove-devtools serve or configure the controller address and token.",
                )
            })?;
        Ok(Self { endpoint, token })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    pub version: u32,
    pub title: String,
    pub platform: String,
    pub pid: Option<u32>,
    /// Exact platform window identifier, when the application host has one.
    pub window: Option<u64>,
    /// A WebDriver session or page can be bound explicitly by the controller.
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub sequence: u64,
    pub command: ProbeCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub sequence: u64,
    pub result: Result<Value, Fault>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub lease: Lease,
    pub completion: Option<Completion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum ProbeCommand {
    Tree,
    Select {
        node_id: Option<String>,
    },
    Read {
        name: String,
        arguments: Value,
    },
    Edit {
        name: String,
        arguments: Value,
    },
    Resize {
        width: f64,
        height: f64,
    },
    Environment(Value),
    TraceStart,
    TraceStop,
    TraceRead {
        id: String,
        offset: usize,
        limit: usize,
    },
    TraceCompare {
        baseline: String,
        candidate: String,
    },
    Timing {
        after: u64,
    },
    Logs {
        after: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct Fault {
    pub code: String,
    pub message: String,
}

impl Fault {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
