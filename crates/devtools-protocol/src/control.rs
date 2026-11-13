//! The controller and runtime probe share these transport-independent messages.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 2 * 1024 * 1024;
/// Leaves room for Base64 and the probe reply inside MAX_MESSAGE.
pub const MAX_CAPTURE_BYTES: usize = 1_500_000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
}

impl Viewport {
    pub fn contains(self, x: f32, y: f32) -> bool {
        (0.0..self.width).contains(&x) && (0.0..self.height).contains(&y)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCapture {
    pub viewport: Viewport,
    pub png: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeInput {
    Click {
        x: f32,
        y: f32,
        button: u8,
    },
    Drag {
        x: f32,
        y: f32,
        to_x: f32,
        to_y: f32,
    },
    Scroll {
        x: f32,
        y: f32,
        dx: i32,
        dy: i32,
    },
    Key {
        key: String,
        modifiers: Vec<String>,
    },
    Text {
        text: String,
    },
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum ProbeCommand {
    Tree,
    /// A bounded subtree of the live tree.
    ///
    /// `node_id` roots the page at a live element ID (the scene root when
    /// absent), `depth` counts node levels including that root, and `limit`
    /// caps the returned nodes. Children or nodes beyond either bound are left
    /// out and reported through `truncated`; the probe never synthesises a
    /// node. Unknown IDs answer `stale_node`; out-of-range bounds answer
    /// `invalid_request`.
    TreePage {
        node_id: Option<String>,
        depth: usize,
        limit: usize,
    },
    Select {
        node_id: Option<String>,
    },
    /// Hit-test the live tree at a content-space logical point without
    /// dispatching platform input.
    NodeAt {
        x: f64,
        y: f64,
    },
    /// The runtime content origin in the platform's window-server coordinate
    /// space and the scale between that space and runtime logical points.
    ContentGeometry,
    /// Returns one PNG from the runtime's own render surface.
    Capture,
    /// Sends an action to the selected runtime's scene dispatcher.
    Input {
        viewport: Option<Viewport>,
        action: RuntimeInput,
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
    /// Classify a recorded frame's dominant CPU phase from the trace spans
    /// retained for its timeline window.
    AnalyzeUiFrame {
        frame: u64,
    },
    Logs {
        after: Option<u64>,
    },
    /// Empty the runtime's retained log history and reset the loss count for
    /// the cleared window. The reply carries the number of removed records,
    /// the reset loss count and the first available sequence. Sequences keep
    /// increasing; records logged after the command are retained normally.
    ClearLogs,
    /// Start sampling the runtime's UI thread with the in-process CPU sampler.
    ///
    /// `hz` is the requested rate and `duration_ms` the bound after which the
    /// probe stops by itself. Runtimes without a real sampler answer
    /// `unavailable` with the reason instead of returning samples.
    ProfileStart {
        hz: u32,
        duration_ms: u32,
    },
    /// Finish the active profile, join the sampler and return it symbolized.
    ProfileStop,
    /// The active or most recent profile: a bounded set of aggregated stacks.
    ProfileRead,
    /// Aggregated self and inclusive hotspots of the retained profile.
    ProfileHotspots {
        limit: usize,
    },
    /// Start sampling allocation sites in the runtime's process.
    ///
    /// `rate` is the 1-in-N divisor: every `rate`-th successful fresh
    /// allocation captures a bounded raw stack. A start replaces any prior
    /// session, so the next stop describes only allocations after this call.
    /// Runtimes without real stack capture answer `unavailable` with the
    /// reason instead of returning sites.
    AllocationStart {
        rate: u32,
    },
    /// Finish the active sampling session and return its symbolized sites.
    AllocationStop,
    /// The sites collected so far, or the last stopped report.
    ///
    /// `limit` bounds the returned sites; sites beyond it set
    /// `sites_truncated`.
    AllocationSites {
        limit: usize,
    },
    /// Toggle debug paint guidelines: one-pixel outlines around every visible
    /// live node frame. The state reports only what it draws. Baselines
    /// (`Baselines`), repaint highlighting (`RepaintHighlight`) and slow
    /// animations (`AnimationScale`) are separate layers, not part of this
    /// state.
    DebugPaint {
        guidelines: bool,
    },
    /// Toggle text baselines: one-pixel lines at the real baseline of every
    /// measured text line on visible nodes, clipped by the root viewport and
    /// each node's clipping ancestors. The reply carries the applied state.
    Baselines {
        enabled: bool,
    },
    /// Toggle oversized-image highlighting: a two-pixel outline around every
    /// visible image whose decoded source area exceeds its painted display
    /// area by four times or more. Vector sources rasterized at the draw size,
    /// images without a known decoded size, and invalid or zero-size display
    /// rects are skipped. The reply carries the applied state.
    OversizedImages {
        enabled: bool,
    },
    /// Toggle repaint highlighting: a two-pixel outline around every visible
    /// node that has really painted, colored by that node's paint generation.
    /// The generation only advances when a renderer emits the node's own box
    /// or content, so a layer retained in the cache keeps its color while a
    /// repainting node cycles. Nodes that have never painted are skipped. The
    /// reply carries the applied state.
    RepaintHighlight {
        enabled: bool,
    },
    /// Scale every animation's delta globally: `1.0` is real time, `0.5` half
    /// speed. The scale is clamped to a safe range and the reply carries the
    /// applied value. Frame pacing, input and diagnostics keep wall-clock
    /// time.
    AnimationScale {
        scale: f64,
    },
    /// Pause or resume the runtime's in-process HTTP recorder.
    ///
    /// Disabling stops new requests from being recorded and keeps the retained
    /// history; enabling records new requests again. The reply carries the
    /// state the recorder actually holds.
    NetworkRecording {
        enabled: bool,
    },
    /// Empty the runtime's retained network records and report how many were
    /// removed. The recorder's enabled state and its 256-record bound are
    /// unchanged, so requests after the clear are retained normally.
    ClearNetwork,
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
