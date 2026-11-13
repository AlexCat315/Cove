//! Transport-neutral Cove diagnostics and DevTools messages.
//!
//! Swift's `JSONValue` maps to `JsonValue` below — deliberately a re-export of
//! `serde_json::Value`, not a second type: that enum already has exactly Swift's
//! variants (`Null`/`Bool`/`Number`/`String`/`Array`/`Object`) and its accessors
//! are Swift's `stringValue`/`objectValue`. Keeping one type means a payload can
//! never hold two incompatible JSON trees.
pub mod control;
mod session;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub use serde_json::Value as JsonValue;
pub use session::*;

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
    layout: Option<NodeLayoutInfo>, style: Option<NodeStyleInfo>, source: Option<SourceLocation>,
    #[serde(rename = "ownerScopeID")] owner_scope_id: Option<String>, recomposition: Option<Recomposition>,
});
payload!(TreeSnapshot {
    root: Option<NodeSummary>, invalidations: Option<Vec<InvalidationRecord>>,
    render_inventory: Option<RenderInventory>, input_inventory: Option<InputInventory>, inspection: Option<InspectionState>,
});
// One node inside a [`TreePage`].
//
// This is the wire summary's identity, geometry, roles, source and children
// without the summary's always-null detail slots: a page is read by an agent
// under a 512 KiB result budget, so absent values are omitted rather than
// serialised as `null`. Children cut by depth or by the node or byte budget
// are absent; the page's `truncated` flag says the subtree continued.
payload!(TreePageNode {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")] view_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] debug_name: Option<String>,
    frame: NodeFrame,
    absolute_frame: NodeFrame,
    #[serde(skip_serializing_if = "Option::is_none")] layout_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] semantic_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] source: Option<SourceLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] children: Vec<TreePageNode>,
});
// A bounded, real subtree of the live tree.
//
// `depth` counts node levels and includes the page root, so `depth = 1` is
// the root alone. `nodeCount` is the number of live nodes actually returned.
// `limit` is the cap the page applied: the requested limit unless the byte
// budget reduced it. `truncated` is true when the subtree continued past the
// depth, the node limit or the byte budget. No node is ever synthesised to
// stand in for a cut one.
payload!(TreePage {
    root: Option<TreePageNode>,
    depth: usize,
    limit: usize,
    node_count: usize,
    truncated: bool,
});
// The deepest page a client may request, counting the page root as level 1.
pub const MAX_TREE_PAGE_DEPTH: usize = 16;
/// The most nodes one page may name before the byte budget applies.
pub const MAX_TREE_PAGE_NODES: usize = 2000;
/// Serialised page size ceiling.
///
/// The agent toolbox rejects a JSON result above 512 KiB. Staying at three
/// quarters of that leaves room for the MCP envelope and for transports that
/// re-encode the result, while still carrying hundreds of real gallery nodes.
pub const MAX_TREE_PAGE_BYTES: usize = 384 * 1024;
payload!(InvalidationRecord {
    target: String,
    source: String,
    phase: String,
    timestamp: f64
});
payload!(RenderInventory { object_count: usize, layer_roots: Vec<String> });
payload!(InputInventory { node_count: usize, focusables: Vec<String>, hit_testables: Vec<String> });
payload!(WindowResize {
    width: f64,
    height: f64
});
// Where a runtime's content starts in the window-server coordinate space its
// host captures, and how that space maps onto runtime logical points.
//
// `origin` is the screen point of content `(0, 0)` in the same units the
// platform reports window bounds (logical points on Apple platforms, physical
// pixels on Windows and X11). `scale` is window-server units per content
// logical point, so `content = (server - origin) / scale`. `width`/`height`
// are the content viewport in logical points.
payload!(ContentGeometry {
    origin_x: f64,
    origin_y: f64,
    scale: f64,
    width: f64,
    height: f64
});
impl ContentGeometry {
    pub fn is_valid(&self) -> bool {
        [
            self.origin_x,
            self.origin_y,
            self.scale,
            self.width,
            self.height,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.scale > 0.0
            && self.width >= 0.0
            && self.height >= 0.0
    }
}
impl NodeSummary {
    /// Depth-first lookup by element ID in this subtree.
    ///
    /// Clients use it on the wire `tree` payload, whose entries keep identity
    /// and geometry but omit detailed layout and style.
    pub fn find(&self, id: &str) -> Option<&NodeSummary> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }
}
// The deepest hit-testable node at a content-space point.
//
// `node_id` is `None` when no node contains the point, which is the client's
// cue to clear a selection rather than an error.
payload!(NodeHit {
    #[serde(rename = "nodeID")] node_id: Option<String>,
    path: Vec<String>,
    local_point: Option<NodePoint>,
    absolute_frame: Option<NodeFrame>
});
impl WindowResize {
    pub fn is_valid(&self) -> bool {
        (1.0..=16384.0).contains(&self.width) && (1.0..=16384.0).contains(&self.height)
    }
}
payload!(TimingFrame {
    frame: u64, recomposition_ms: Option<f64>, layout_ms: f64, draw_ms: f64,
    render_thread_ms: Option<f64>, #[serde(default)] gpu_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pipeline_creation: Option<PipelineCreation>,
    submitted: bool, presented: Option<bool>,
    total_ms: f64, node_count: usize, batch_count: usize,
    // The CPU timeline window this frame owns: spans with a sequence in
    // `(timeline_start, timeline_end]` were produced while the frame was
    // prepared. Both are 0 when the host had no timeline window to report,
    // including frames recorded by probes older than this field.
    #[serde(default)] timeline_start: u64,
    #[serde(default)] timeline_end: u64,
    // Pass and layer attribution for this frame. Absent when the host records no
    // GPU frame, or when the probe that produced the frame predates this field.
    #[serde(default, skip_serializing_if = "Option::is_none")] gpu_breakdown: Option<GpuFrameBreakdown>,
});
payload!(PipelineCreation {
    count: u64,
    cpu_ms: f64
});
payload!(GpuPass {
    label: String,
    /// `render`, `compute`, `copy` or `accelerationStructure`.
    kind: String,
    draws: u64,
    indices: u64,
    vertices: u64,
    /// Indices into `GpuFrameBreakdown::layers` this pass rendered. Empty means
    /// the pass drew geometry no layer owns.
    #[serde(default)] layers: Vec<u32>,
    /// GPU time of this pass. Present only when the backend times passes
    /// individually; `timing_source` on the breakdown says which backends do.
    #[serde(default, skip_serializing_if = "Option::is_none")] gpu_ms: Option<f64>,
});
payload!(GpuLayer {
    /// The layer root's node id, when the census could name it.
    #[serde(rename = "nodeID", default, skip_serializing_if = "Option::is_none")] node_id: Option<String>,
    /// The layer's window in the frame index buffer.
    first_index: u64,
    index_count: u64,
    /// False when the frame reused the layer's cached draw list.
    recorded: bool,
    /// True when reuse only translated cached vertices.
    translated: bool,
    /// True for a layer whose cached geometry sat entirely outside the frame.
    skipped: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    /// Indices into `GpuFrameBreakdown::passes` that rendered this layer.
    #[serde(default)] passes: Vec<u32>,
});
impl GpuLayer {
    /// The layer's window as a `(first, count)` pair.
    pub fn index_range(&self) -> (u64, u64) {
        (self.first_index, self.index_count)
    }
}
payload!(GpuFrameBreakdown {
    /// Where pass times came from: `vulkan-timestamps` (one label per pass),
    /// `metal-submission` (one label per submission), `webgpu` (one number for
    /// the whole frame), or `cpu-only` when nothing was sampled.
    timing_source: String,
    #[serde(default)] passes: Vec<GpuPass>,
    #[serde(default)] layers: Vec<GpuLayer>,
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
// A DevTools extension a package or the host registered. Mirrors Flutter's
// DevTools extension contract: a discoverable panel with a hosted entry point the
// Inspector opens in a sandboxed frame. `entryPath` is a URL the client loads.
payload!(DevToolsExtension {
    id: String,
    name: String,
    description: String,
    version: String,
    homepage: Option<String>,
    #[serde(rename = "iconPath")] icon_path: Option<String>,
    #[serde(rename = "entryPath")] entry_path: String,
    enabled: bool,
});
// The full set of extensions the runtime currently advertises.
payload!(ExtensionList { extensions: Vec<DevToolsExtension> });
payload!(InspectionInsets {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64
});
// Authored length bounds in logical points.
payload!(NodeConstraints {
    min_width: Option<f64>,
    max_width: Option<f64>,
    min_height: Option<f64>,
    max_height: Option<f64>,
});
impl Default for NodeConstraints {
    fn default() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
        }
    }
}
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
    #[serde(default)]
    constraints: NodeConstraints,
    /// True when a child's frame spills outside this node's box while its
    /// `overflow` is `Visible` — the Cove equivalent of Flutter's
    /// "A RenderFlex overflowed by N pixels" warning.
    #[serde(default)]
    overflow_detected: bool,
});
payload!(NodeStyleInfo { background_color: Option<String>, foreground_color: Option<String>, overrides: Vec<String> });
payload!(LayoutAxisInput {
    known: Option<f32>,
    definite: Option<bool>,
    parent: Option<f32>,
    available: Option<f32>,
    available_kind: String,
});
payload!(LayoutProposal {
    width: LayoutAxisInput,
    height: LayoutAxisInput,
});
payload!(LayoutChainNode {
    id: Option<String>,
    name: Option<String>,
    incoming: Option<LayoutProposal>,
    solved: NodeFrame,
    resolved: Option<NodeFrame>,
    visible: Option<bool>,
});
payload!(LayoutChain {
    parent: Option<LayoutChainNode>,
    node: LayoutChainNode,
    children: Vec<LayoutChainNode>,
    child_count: usize,
    overflow: Option<InspectionInsets>,
    flex_basis: String,
    align_self: String,
    text_truncated: Option<bool>,
    text_overflow: Option<LayoutExtent>,
});
payload!(LayoutExtent {
    width: f32,
    height: f32
});
payload!(ComponentActivity {
    id: String,
    name: String,
    build_count: Option<u64>,
    layout_count: Option<u64>,
    paint_count: u64,
    reasons: Vec<RecompositionReason>,
});
payload!(RenderObjectInfo {
    #[serde(rename = "parentID")] parent_id: Option<String>,
    child_count: usize,
    layer_root: bool,
    cache_invalid: bool,
    cached_batches: Option<usize>,
    paint_bounds: Option<NodeFrame>,
    clip_count: usize,
    paint_count: u64,
});
payload!(InspectionState {
    #[serde(rename = "selectedID")] selected_id: Option<String>,
    can_undo: bool, can_redo: bool, override_count: usize,
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
// State of one sampled CPU profile. `idle` means no run has started in this
// probe session, `running` means the sampler is still following the UI thread
// and `ready` means a bounded, symbolized profile is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    Idle,
    Running,
    Ready,
}
// One symbolized stack frame. `function` and `file_path` stay absent when
// neither DWARF nor the dynamic symbol table resolves the sampled address; the
// label then carries the instruction address instead of inventing a name.
payload!(ProfileFrame {
    label: String,
    function: Option<String>,
    file_path: Option<String>,
    line: Option<u32>,
});
payload!(ProfileStack { samples: u64, percent: f64, frames: Vec<ProfileFrame> });
payload!(ProfileHotspot {
    label: String,
    function: Option<String>,
    file_path: Option<String>,
    line: Option<u32>,
    self_samples: u64,
    inclusive_samples: u64,
    self_ms: f64,
    inclusive_ms: f64,
    self_percent: f64,
    inclusive_percent: f64,
});
// Where a profile's numbers come from. `sampler` counts periodic stack samples
// taken by suspending the UI thread; `instrumented` accumulates the durations
// the runtime measured around its own frame phases, so `samples` count phase
// occurrences and every millisecond is measured, never synthesized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSource {
    Sampler,
    Instrumented,
}
// A bounded answer to `profile_read`. Stacks are aggregated by identical frame
// sequence, not one record per sample; `stacks_truncated` reports the bound.
payload!(UiProfile {
    status: ProfileStatus,
    source: ProfileSource,
    thread: String,
    hz: u32,
    duration_ms: f64,
    sample_interval_ms: f64,
    samples_taken: u64,
    samples_dropped: u64,
    retained_samples: u64,
    samples_per_second: Option<f64>,
    stacks_truncated: bool,
    stacks: Vec<ProfileStack>,
});
// A bounded answer to `profile_hotspots`. Self time is exclusive; inclusive
// time counts a frame once per sample, so child time does not inflate it.
payload!(UiHotspots {
    status: ProfileStatus,
    source: ProfileSource,
    thread: String,
    hz: u32,
    duration_ms: f64,
    sample_interval_ms: f64,
    samples_taken: u64,
    samples_dropped: u64,
    retained_samples: u64,
    truncated: bool,
    hotspots: Vec<ProfileHotspot>,
});
/// Most sites one allocation-site answer carries after symbolization.
///
/// The bound applies before symbols are resolved, so the work per answer is
/// `MAX_ALLOCATION_SITES * MAX_ALLOCATION_SITE_FRAMES` lookups at most.
pub const MAX_ALLOCATION_SITES: usize = 64;
/// Highest 1-in-N allocation sampling rate a request may ask for.
pub const MAX_ALLOCATION_RATE: u32 = 1_000_000;
// State of allocation-site sampling. `idle` means this probe session never
// sampled, `running` means the sampler is still recording, and `ready` means a
// stopped report is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationSiteStatus {
    Idle,
    Running,
    Ready,
}
// One symbolized frame of an allocation site.
//
// `address` is always the real sampled return address. `function`, `file_path`
// and `line` stay absent when the platform cannot symbolize addresses (Linux
// has no DWARF symbolizer yet) or when the address has no debug information.
payload!(AllocationSiteFrame {
    address: u64,
    function: Option<String>,
    file_path: Option<String>,
    line: Option<u32>,
});
// One allocation site: the caller stack that allocated and what was sampled
// through it. `bytes` and `blocks` are cumulative sampled allocations, not the
// live heap: a block freed after it was sampled still counts here.
payload!(AllocationSite {
    bytes: u64,
    blocks: u64,
    // The captured stack reached the capture depth limit, so deeper callers
    // were not recorded.
    frames_truncated: bool,
    frames: Vec<AllocationSiteFrame>,
});
// A bounded answer to `allocation_stop` or `allocation_sites`.
//
// Counters are cumulative for the sampling window that produced them. They are
// sampled allocation totals, never live-heap or ownership data.
payload!(UiAllocationSites {
    status: AllocationSiteStatus,
    rate: u32,
    sampled_bytes: u64,
    sampled_blocks: u64,
    samples_attempted: u64,
    samples_retained: u64,
    samples_dropped: u64,
    capture_failures: u64,
    // Sites existed beyond the response bound or beyond `rate`'s ring.
    sites_truncated: bool,
    // Distinct sampled stacks aggregated in this window, before the response
    // bound was applied.
    site_count: usize,
    // Why addresses carry no function or file, when the platform cannot
    // symbolize; `None` when the symbolizer resolved what it could.
    symbolization_reason: Option<String>,
    sites: Vec<AllocationSite>,
});
/// Decode an envelope payload into a concrete type. Returns `None` for an
/// absent or malformed payload, matching Swift's `decodePayload(_:from:)`.
pub fn decode_payload<T: serde::de::DeserializeOwned>(payload: &Option<JsonValue>) -> Option<T> {
    serde_json::from_value(payload.clone()?).ok()
}

/// Serialisation helpers, matching Swift's `DevToolsCodec`.
pub struct DevToolsCodec;

impl DevToolsCodec {
    /// Host-produced protocol values must be finite and JSON-encodable. A value
    /// that is not yields `Null` rather than panicking the way Swift's `try!`
    /// would.
    pub fn json<T: Serialize>(value: &T) -> JsonValue {
        serde_json::to_value(value).unwrap_or(JsonValue::Null)
    }

    /// Decode a payload slot into a concrete type.
    pub fn decode<T: DeserializeOwned>(payload: &Option<JsonValue>) -> Option<T> {
        decode_payload(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_uses_the_swift_field_names() {
        let envelope = Envelope::new("inspect.style.undo", Some(3), None);
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["type"], "inspect.style.undo");
        assert_eq!(value["id"], 3);
        assert!(value.get("payload").is_some());
    }

    #[test]
    fn a_frame_without_a_breakdown_still_reads() {
        // Probes older than the breakdown field omit it entirely.
        let json = serde_json::json!({
            "frame": 3,
            "layoutMs": 0.5,
            "drawMs": 0.25,
            "submitted": true,
            "totalMs": 1.0,
            "nodeCount": 2,
            "batchCount": 1
        });
        let frame: TimingFrame = serde_json::from_value(json).expect("old frame shape");
        assert!(frame.gpu_breakdown.is_none());
    }

    #[test]
    fn a_breakdown_names_passes_and_their_layers() {
        let breakdown = GpuFrameBreakdown {
            timing_source: "vulkan-timestamps".to_owned(),
            passes: vec![GpuPass {
                label: "ui-frame".to_owned(),
                kind: "render".to_owned(),
                draws: 3,
                indices: 120,
                vertices: 0,
                layers: vec![0, 1],
                gpu_ms: Some(0.42),
            }],
            layers: vec![GpuLayer {
                node_id: Some("7".to_owned()),
                first_index: 0,
                index_count: 120,
                recorded: true,
                translated: false,
                skipped: false,
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 40.0,
                passes: vec![0],
            }],
        };
        let value = serde_json::to_value(&breakdown).expect("serialize");
        assert_eq!(value["timingSource"], "vulkan-timestamps");
        assert_eq!(value["passes"][0]["layers"][1], 1);
        assert_eq!(value["passes"][0]["gpuMs"], 0.42);
        assert_eq!(value["layers"][0]["nodeID"], "7");
        assert_eq!(value["layers"][0]["firstIndex"], 0);
        let read: GpuFrameBreakdown = serde_json::from_value(value).expect("round trip");
        assert_eq!(read, breakdown);
    }

    #[test]
    fn payloads_round_trip_through_the_codec() {
        let frame = TimingFrame {
            gpu_breakdown: None,
            frame: 1,
            recomposition_ms: None,
            layout_ms: 0.5,
            draw_ms: 0.25,
            render_thread_ms: Some(0.125),
            gpu_ms: None,
            pipeline_creation: None,
            submitted: true,
            presented: Some(true),
            total_ms: 1.0,
            node_count: 4,
            batch_count: 2,
            timeline_start: 10,
            timeline_end: 14,
        };
        let payload = Some(DevToolsCodec::json(&frame));
        assert_eq!(DevToolsCodec::decode::<TimingFrame>(&payload), Some(frame));
        let value = payload.as_ref().expect("encoded payload");
        assert!(value.get("recompositionMs").is_some());
        assert!(value.get("presented").is_some());
        assert_eq!(value["timelineStart"], 10);
        assert_eq!(value["timelineEnd"], 14);
    }

    #[test]
    fn timing_frames_without_a_window_still_decode() {
        let legacy = serde_json::json!({
            "frame": 3,
            "recompositionMs": null,
            "layoutMs": 0.5,
            "drawMs": 0.25,
            "renderThreadMs": null,
            "submitted": true,
            "presented": null,
            "totalMs": 1.0,
            "nodeCount": 4,
            "batchCount": 2,
        });
        let frame = DevToolsCodec::decode::<TimingFrame>(&Some(legacy))
            .expect("a probe that predates trace windows still decodes");
        assert_eq!(frame.timeline_start, 0);
        assert_eq!(frame.timeline_end, 0);
    }

    #[test]
    fn source_location_keeps_the_capitalised_id_keys() {
        let location = SourceLocation {
            file_id: "f".into(),
            file_path: "/tmp/a.swift".into(),
            line: 1,
            column: 2,
        };
        let value = DevToolsCodec::json(&location);
        assert_eq!(value["fileID"], "f");
        assert_eq!(value["filePath"], "/tmp/a.swift");
    }

    #[test]
    fn node_layout_info_carries_constraints_and_overflow() {
        let info = NodeLayoutInfo {
            padding: InspectionInsets {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            margin: InspectionInsets {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            border: InspectionInsets {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            content_width: 100.0,
            content_height: 40.0,
            direction: "Ltr".into(),
            flex_direction: "Row".into(),
            justify_content: "FlexStart".into(),
            align_items: "Stretch".into(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            constraints: NodeConstraints {
                min_width: Some(80.0),
                max_width: None,
                min_height: Some(20.0),
                max_height: Some(200.0),
            },
            overflow_detected: true,
        };
        let value = DevToolsCodec::json(&info);
        assert_eq!(value["constraints"]["minWidth"], 80.0);
        assert_eq!(value["constraints"]["maxHeight"], 200.0);
        assert_eq!(value["overflowDetected"], true);
        let decoded = DevToolsCodec::decode::<NodeLayoutInfo>(&Some(value))
            .expect("a full layout block decodes");
        assert_eq!(decoded.constraints.min_width, Some(80.0));
        assert!(decoded.overflow_detected);
    }

    #[test]
    fn node_layout_info_accepts_legacy_messages_without_constraints() {
        // Clients built before constraints/overflow existed must still decode.
        let legacy = serde_json::json!({
            "padding": { "top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0 },
            "margin": { "top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0 },
            "border": { "top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0 },
            "contentWidth": 100.0,
            "contentHeight": 40.0,
            "direction": "Ltr",
            "flexDirection": "Row",
            "justifyContent": "FlexStart",
            "alignItems": "Stretch",
            "flexGrow": 0.0,
            "flexShrink": 1.0
        });
        let decoded = DevToolsCodec::decode::<NodeLayoutInfo>(&Some(legacy))
            .expect("a legacy layout block decodes");
        assert_eq!(decoded.constraints, NodeConstraints::default());
        assert!(!decoded.overflow_detected);
    }

    #[test]
    fn profile_commands_round_trip_with_the_snake_case_method_tags() {
        use crate::control::ProbeCommand;

        let commands = [
            ProbeCommand::ProfileStart {
                hz: 500,
                duration_ms: 250,
            },
            ProbeCommand::ProfileStop,
            ProbeCommand::ProfileRead,
            ProbeCommand::ProfileHotspots { limit: 12 },
        ];
        let wire: Vec<JsonValue> = commands
            .iter()
            .map(|command| serde_json::to_value(command).unwrap())
            .collect();
        assert_eq!(
            wire[0],
            serde_json::json!({
                "method": "profile_start",
                "params": {"hz": 500, "duration_ms": 250},
            })
        );
        assert_eq!(wire[1], serde_json::json!({"method": "profile_stop"}));
        assert_eq!(wire[2], serde_json::json!({"method": "profile_read"}));
        assert_eq!(
            wire[3],
            serde_json::json!({"method": "profile_hotspots", "params": {"limit": 12}})
        );
        for (command, value) in commands.into_iter().zip(wire) {
            assert_eq!(
                serde_json::from_value::<ProbeCommand>(value).unwrap(),
                command
            );
        }
    }

    #[test]
    fn clearing_logs_round_trips_as_a_snake_case_method() {
        use crate::control::ProbeCommand;

        let wire = serde_json::to_value(ProbeCommand::ClearLogs).unwrap();
        assert_eq!(wire, serde_json::json!({"method": "clear_logs"}));
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            ProbeCommand::ClearLogs
        );
    }

    #[test]
    fn debug_paint_round_trips_as_a_snake_case_method() {
        use crate::control::ProbeCommand;

        let command = ProbeCommand::DebugPaint { guidelines: true };
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "method": "debug_paint",
                "params": {"guidelines": true},
            })
        );
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            command
        );
        let off = serde_json::json!({"method": "debug_paint", "params": {"guidelines": false}});
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(off).unwrap(),
            ProbeCommand::DebugPaint { guidelines: false }
        );
    }

    #[test]
    fn baselines_round_trips_as_a_snake_case_method() {
        use crate::control::ProbeCommand;

        let command = ProbeCommand::Baselines { enabled: true };
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "method": "baselines",
                "params": {"enabled": true},
            })
        );
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            command
        );
        let off = serde_json::json!({"method": "baselines", "params": {"enabled": false}});
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(off).unwrap(),
            ProbeCommand::Baselines { enabled: false }
        );
    }

    #[test]
    fn oversized_images_round_trips_as_a_snake_case_method() {
        use crate::control::ProbeCommand;

        let command = ProbeCommand::OversizedImages { enabled: true };
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "method": "oversized_images",
                "params": {"enabled": true},
            })
        );
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            command
        );
        let off = serde_json::json!({"method": "oversized_images", "params": {"enabled": false}});
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(off).unwrap(),
            ProbeCommand::OversizedImages { enabled: false }
        );
    }

    #[test]
    fn repaint_highlight_round_trips_as_a_snake_case_method() {
        use crate::control::ProbeCommand;

        let command = ProbeCommand::RepaintHighlight { enabled: true };
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "method": "repaint_highlight",
                "params": {"enabled": true},
            })
        );
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            command
        );
        let off = serde_json::json!({"method": "repaint_highlight", "params": {"enabled": false}});
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(off).unwrap(),
            ProbeCommand::RepaintHighlight { enabled: false }
        );
    }

    #[test]
    fn animation_scale_round_trips_as_a_snake_case_method() {
        use crate::control::ProbeCommand;

        let command = ProbeCommand::AnimationScale { scale: 0.25 };
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "method": "animation_scale",
                "params": {"scale": 0.25},
            })
        );
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            command
        );
    }

    #[test]
    fn network_control_round_trips_as_snake_case_methods() {
        use crate::control::ProbeCommand;

        let command = ProbeCommand::NetworkRecording { enabled: false };
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "method": "network_recording",
                "params": {"enabled": false},
            })
        );
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            command
        );
        let on = serde_json::json!({"method": "network_recording", "params": {"enabled": true}});
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(on).unwrap(),
            ProbeCommand::NetworkRecording { enabled: true }
        );

        let wire = serde_json::to_value(ProbeCommand::ClearNetwork).unwrap();
        assert_eq!(wire, serde_json::json!({"method": "clear_network"}));
        assert_eq!(
            serde_json::from_value::<ProbeCommand>(wire).unwrap(),
            ProbeCommand::ClearNetwork
        );
    }

    #[test]
    fn allocation_site_commands_round_trip_and_keep_the_snake_case_tags() {
        use crate::control::ProbeCommand;

        let commands = [
            ProbeCommand::AllocationStart { rate: 64 },
            ProbeCommand::AllocationStop,
            ProbeCommand::AllocationSites { limit: 12 },
        ];
        let wire: Vec<JsonValue> = commands
            .iter()
            .map(|command| serde_json::to_value(command).unwrap())
            .collect();
        assert_eq!(
            wire[0],
            serde_json::json!({
                "method": "allocation_start",
                "params": {"rate": 64},
            })
        );
        assert_eq!(wire[1], serde_json::json!({"method": "allocation_stop"}));
        assert_eq!(
            wire[2],
            serde_json::json!({"method": "allocation_sites", "params": {"limit": 12}})
        );
        for (command, value) in commands.into_iter().zip(wire) {
            assert_eq!(
                serde_json::from_value::<ProbeCommand>(value).unwrap(),
                command
            );
        }
    }

    #[test]
    fn allocation_site_payloads_name_real_addresses_and_sampled_totals() {
        let site = AllocationSite {
            bytes: 4096,
            blocks: 32,
            frames_truncated: true,
            frames: vec![AllocationSiteFrame {
                address: 0x1_0000_2000,
                function: Some("gallery::paint".into()),
                file_path: Some("src/paint.rs".into()),
                line: Some(42),
            }],
        };
        let sites = UiAllocationSites {
            status: AllocationSiteStatus::Ready,
            rate: 64,
            sampled_bytes: 4096,
            sampled_blocks: 32,
            samples_attempted: 4,
            samples_retained: 4,
            samples_dropped: 0,
            capture_failures: 0,
            sites_truncated: false,
            site_count: 1,
            symbolization_reason: None,
            sites: vec![site],
        };
        let value = DevToolsCodec::json(&sites);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["sampledBytes"], 4096);
        assert_eq!(value["captureFailures"], 0);
        assert_eq!(value["siteCount"], 1);
        assert_eq!(value["sites"][0]["framesTruncated"], true);
        assert_eq!(value["sites"][0]["frames"][0]["address"], 0x1_0000_2000u64);
        assert_eq!(value["sites"][0]["frames"][0]["filePath"], "src/paint.rs");
        assert_eq!(
            DevToolsCodec::decode::<UiAllocationSites>(&Some(value)),
            Some(sites)
        );
        assert_eq!(
            serde_json::to_value(AllocationSiteStatus::Idle).unwrap(),
            JsonValue::String("idle".into())
        );
    }

    #[test]
    fn profile_payloads_keep_the_hotspot_and_stack_shape() {
        let frame = ProfileFrame {
            label: "app::run src/main.rs:12".into(),
            function: Some("app::run".into()),
            file_path: Some("src/main.rs".into()),
            line: Some(12),
        };
        let profile = UiProfile {
            status: ProfileStatus::Ready,
            source: ProfileSource::Sampler,
            thread: "main".into(),
            hz: 1000,
            duration_ms: 300.0,
            sample_interval_ms: 1.0,
            samples_taken: 300,
            samples_dropped: 0,
            retained_samples: 300,
            samples_per_second: Some(1000.0),
            stacks_truncated: false,
            stacks: vec![ProfileStack {
                samples: 300,
                percent: 100.0,
                frames: vec![frame.clone()],
            }],
        };
        let value = DevToolsCodec::json(&profile);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["sampleIntervalMs"], 1.0);
        assert_eq!(value["samplesTaken"], 300);
        assert_eq!(value["stacks"][0]["frames"][0]["filePath"], "src/main.rs");
        assert_eq!(
            DevToolsCodec::decode::<UiProfile>(&Some(value)),
            Some(profile)
        );

        let hotspots = UiHotspots {
            status: ProfileStatus::Ready,
            source: ProfileSource::Sampler,
            thread: "main".into(),
            hz: 1000,
            duration_ms: 300.0,
            sample_interval_ms: 1.0,
            samples_taken: 300,
            samples_dropped: 0,
            retained_samples: 300,
            truncated: false,
            hotspots: vec![ProfileHotspot {
                label: frame.label.clone(),
                function: frame.function.clone(),
                file_path: frame.file_path.clone(),
                line: frame.line,
                self_samples: 120,
                inclusive_samples: 300,
                self_ms: 120.0,
                inclusive_ms: 300.0,
                self_percent: 40.0,
                inclusive_percent: 100.0,
            }],
        };
        let value = DevToolsCodec::json(&hotspots);
        assert_eq!(value["hotspots"][0]["selfSamples"], 120);
        assert_eq!(value["hotspots"][0]["inclusiveMs"], 300.0);
        assert_eq!(value["hotspots"][0]["line"], 12);
        assert_eq!(
            DevToolsCodec::decode::<UiHotspots>(&Some(value)),
            Some(hotspots)
        );

        assert_eq!(
            serde_json::to_value(ProfileStatus::Idle).unwrap(),
            JsonValue::String("idle".into())
        );
        assert_eq!(
            serde_json::to_value(ProfileStatus::Running).unwrap(),
            JsonValue::String("running".into())
        );
    }
}
