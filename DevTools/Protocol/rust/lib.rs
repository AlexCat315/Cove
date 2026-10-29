//! Transport-neutral DevTools schema. Rust input recordings use protocol 0.2.
mod recording;
mod session;
pub use recording::*;
use serde::{Deserialize, Serialize};
pub use serde_json::Value as JsonValue;
pub use session::*;

pub const PROTOCOL_VERSION: &str = "cove-devtools/0.2";

macro_rules! payload {
    ($name:ident { $($(#[$attr:meta])* $field:ident : $kind:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name { $($(#[$attr])* pub $field: $kind),* }
    }
}

payload!(Envelope { r#type: String, id: Option<i64>, payload: Option<JsonValue> });
impl Envelope {
    pub fn new(kind: impl Into<String>, id: Option<i64>, payload: Option<JsonValue>) -> Self {
        Self {
            r#type: kind.into(),
            id,
            payload,
        }
    }
    pub fn ok(&self) -> Option<Self> {
        self.id
            .map(|id| Self::new(format!("{}.ok", self.r#type), Some(id), None))
    }
    pub fn error(&self, code: &str, message: &str) -> Self {
        Self::new(
            format!("{}.err", self.r#type),
            self.id,
            Some(serde_json::json!({ "code": code, "message": message })),
        )
    }
}
payload!(HelloHostInfo {
    pid: u32,
    app_title: String,
    platform: String
});
payload!(HelloPayload { protocol: String, host: HelloHostInfo, capabilities: Vec<String> });
impl HelloPayload {
    pub fn new(host: HelloHostInfo, capabilities: Vec<String>) -> Self {
        Self {
            protocol: PROTOCOL_VERSION.into(),
            host,
            capabilities,
        }
    }
}
payload!(NodeFrame {
    x: f64,
    y: f64,
    w: f64,
    h: f64
});
payload!(NodePoint { x: f64, y: f64 });
payload!(NodeFlags {
    hit_testable: bool,
    focusable: bool,
    clips_to_bounds: bool,
    has_background: bool,
    has_border: bool
});
payload!(NodeSummary {
    id: String, view_tag: Option<String>, debug_name: Option<String>, frame: NodeFrame,
    absolute_frame: Option<NodeFrame>, content_offset: Option<NodePoint>, z_index: Option<f64>, opacity: Option<f64>,
    layout_role: Option<String>, semantic_role: Option<String>, flags: NodeFlags, children: Vec<NodeSummary>,
    #[serde(rename = "elementID")] element_id: Option<String>,
    layout: Option<NodeLayoutInfo>, style: Option<NodeStyleInfo>, source: Option<SourceLocation>,
    #[serde(rename = "ownerScopeID")] owner_scope_id: Option<String>, recomposition: Option<Recomposition>,
});
payload!(TreeSnapshot {
    root: Option<NodeSummary>, invalidations: Option<Vec<InvalidationRecord>>,
    render_inventory: Option<RenderInventory>, input_inventory: Option<InputInventory>, inspection: Option<InspectionState>,
});
payload!(InvalidationRecord {
    target: String,
    source: String,
    phase: String,
    timestamp: f64
});
payload!(RenderInventory { object_count: usize, layer_roots: Vec<String> });
payload!(InputInventory { node_count: usize, focusables: Vec<String>, hit_testables: Vec<String> });
payload!(SelectNode { id: String });
payload!(ErrorPayload {
    code: String,
    message: String
});
payload!(MirrorStart { fps: Option<f64>, quality: Option<f64>, scale: Option<f64> });
payload!(WindowResize {
    width: f64,
    height: f64
});
impl WindowResize {
    pub fn is_valid(&self) -> bool {
        (1.0..=16384.0).contains(&self.width) && (1.0..=16384.0).contains(&self.height)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MirrorAction {
    #[default]
    Activate,
    Increment,
    Decrement,
    Reveal,
}
payload!(MirrorActivate {
    #[serde(rename = "node_id")]
    node_id: String,
    #[serde(default)]
    action: MirrorAction,
});
payload!(MirrorStopped { reason: String });
payload!(MirrorFrame {
    seq: u64,
    width: u32,
    height: u32,
    logical_width: f64,
    logical_height: f64,
    jpeg_base64: String
});
payload!(MirrorInput {
    kind: String, x: Option<f32>, y: Option<f32>, delta_x: Option<f32>, delta_y: Option<f32>,
    button: Option<i64>, key: Option<String>, key_code: Option<i64>, text: Option<String>,
    modifiers: Option<i64>, click_count: Option<i64>, is_repeat: Option<bool>, scroll: Option<cove_platform::ScrollInput>,
});
payload!(TimingFrame {
    frame: u64, recomposition_ms: Option<f64>, layout_ms: f64, draw_ms: f64,
    present_ms: f64, presented: Option<bool>, total_ms: f64, node_count: usize, batch_count: usize,
});
payload!(RegisteredState {
    id: String, name: String, value_type: String, #[serde(rename = "scopeID")] scope_id: Option<String>,
});
payload!(StateWatch { ids: Vec<String> });
impl StateWatch {
    pub fn is_valid(&self) -> bool {
        self.ids.len() <= 128
            && self
                .ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.ids.len()
            && self.ids.iter().all(|id| !id.is_empty() && id.len() <= 256)
    }
}
payload!(ObservedState {
    id: String,
    summary: String,
    truncated: bool
});
payload!(StateObservation { registered: Vec<RegisteredState>, values: Vec<ObservedState> });
payload!(TimelineReason { kind: String, detail: Option<String> });
payload!(TimelineEvent {
    sequence: u64, phase: String, name: String, #[serde(rename = "scopeID")] scope_id: Option<String>,
    start_ms: f64, duration_ms: f64, #[serde(default)] reasons: Vec<TimelineReason>,
});
payload!(TimelineSnapshot { events: Vec<TimelineEvent>, #[serde(default)] dropped: usize });
payload!(InspectionInsets {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64
});
payload!(NodeLayoutInfo {
    padding: InspectionInsets,
    margin: InspectionInsets,
    border: InspectionInsets,
    content_width: f64,
    content_height: f64,
    direction: String,
    flex_direction: String,
    justify_content: String,
    align_items: String,
    flex_grow: f64,
    flex_shrink: f64,
});
payload!(NodeStyleInfo { background_color: Option<String>, foreground_color: Option<String>, overrides: Vec<String> });
payload!(InspectionState {
    #[serde(rename = "selectedID")] selected_id: Option<String>,
    #[serde(rename = "hoveredID")] hovered_id: Option<String>,
    picking: bool, can_undo: bool, can_redo: bool, override_count: usize,
});
payload!(SourceLocation {
    #[serde(rename = "fileID")]
    file_id: String,
    file_path: String,
    line: u64,
    column: u64
});
payload!(RecompositionReason {
    kind: String, detail: Option<String>, #[serde(rename = "originScopeID")] origin_scope_id: Option<String>,
});
payload!(Recomposition { count: u64, initial_ms: f64, last_ms: f64, total_ms: f64, max_ms: f64, reasons: Vec<RecompositionReason> });
payload!(DevToolsLaunch {
    id: String,
    token: String,
    descriptor_path: String,
    app_title: String
});
payload!(DevToolsDiscovery {
    id: String,
    pid: u32,
    endpoint: String
});
