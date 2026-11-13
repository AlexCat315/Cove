//! Read-only diagnostic facade over the scene.
//!
//! This is the surface an agent or MCP client reads. Live `Node`s never escape:
//! every query projects into a bounded, serialisable record so a model cannot
//! hold scene state across a step.
use crate::inspection::inspector::{DebugAttachmentKey, SceneInspector};
use cove_core::Rect;
use cove_core::scene::{
    AccessibilitySemantics, AccessibilityTree, Node, NodeTree, StateDescriptor, StateRegistry,
    StateValue,
};
use cove_devtools_protocol::{
    Envelope, NodeFrame, NodeSummary, SourceLocation, StateObservation, StateWatch,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Maximum items a single page returns, matching Swift's clamp.
pub const MAX_PAGE_ITEMS: usize = 100;

/// One page of results with the total and the next cursor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticPage<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub next_offset: Option<usize>,
}

impl<T> DiagnosticPage<T> {
    pub fn new(items: &[T], offset: usize, limit: usize) -> Self
    where
        T: Clone,
    {
        let start = items.len().min(offset);
        let end = items.len().min(start + limit.clamp(1, MAX_PAGE_ITEMS));
        Self {
            items: items[start..end].to_vec(),
            total: items.len(),
            next_offset: (end < items.len()).then_some(end),
        }
    }

    pub fn project<U>(self, transform: impl Fn(T) -> U) -> DiagnosticPage<U> {
        DiagnosticPage {
            items: self.items.into_iter().map(transform).collect(),
            total: self.total,
            next_offset: self.next_offset,
        }
    }
}

/// Memory counters the host chooses to report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticMemorySnapshot {
    pub counters: BTreeMap<String, i64>,
    pub windows: BTreeMap<String, BTreeMap<String, i64>>,
}

/// A node plus its semantics, ancestor link and computed presentation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticNode {
    pub node: NodeSummary,
    #[serde(rename = "parentID")]
    pub parent_id: Option<String>,
    pub semantics: Option<AccessibilitySemantics>,
    pub presentation: DiagnosticPresentation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_truncated: Option<bool>,
}

/// Current presentation, separate from authored node flags. Visible bounds
/// include ancestor clipping and scroll offsets; input includes ancestors and
/// the owning scene's active modal scope.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticPresentation {
    pub is_visible: bool,
    pub input_enabled: bool,
    pub visible_frame: Option<NodeFrame>,
}

/// One-line projection for AI clients (`compact: true`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCompactNode {
    pub id: String,
    #[serde(rename = "parentID")]
    pub parent_id: Option<String>,
    pub role: Option<String>,
    pub label: Option<String>,
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_truncated: Option<bool>,
    pub state: Option<Vec<String>>,
    pub frame: Option<NodeFrame>,
    pub input_enabled: bool,
    pub debug_name: Option<String>,
    /// `File.swift:line`.
    pub source: Option<String>,
}

impl DiagnosticCompactNode {
    pub fn from_node(item: &DiagnosticNode) -> Self {
        let semantics = item.semantics.as_ref();
        let mut state = Vec::new();
        if let Some(semantics) = semantics {
            if semantics.state.is_selected {
                state.push("selected".to_string());
            }
            if let Some(expanded) = semantics.state.is_expanded {
                state.push(if expanded { "expanded" } else { "collapsed" }.to_string());
            }
            if !semantics.state.is_enabled {
                state.push("disabled".to_string());
            }
            if semantics.state.is_invalid {
                state.push("invalid".to_string());
            }
            if semantics.state.is_read_only {
                state.push("readOnly".to_string());
            }
        }
        Self {
            id: item.node.id.clone(),
            parent_id: item.parent_id.clone(),
            role: semantics
                .map(|semantics| crate::inspection::inspector::role_name(semantics.role))
                .or_else(|| item.node.semantic_role.clone()),
            label: semantics
                .map(|semantics| semantics.label.clone())
                .filter(|label| !label.is_empty()),
            value: semantics
                .map(|semantics| semantics.value.clone())
                .filter(|value| !value.is_empty()),
            value_truncated: item.value_truncated,
            state: (!state.is_empty()).then_some(state),
            frame: item.presentation.visible_frame.clone(),
            input_enabled: item.presentation.input_enabled,
            debug_name: item.node.debug_name.clone(),
            source: item.node.source.as_ref().map(|source| {
                let name = source
                    .file_id
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&source.file_id);
                format!("{name}:{}", source.line)
            }),
        }
    }
}

/// Geometry defect found in an authored frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticGeometryIssue {
    #[serde(rename = "nodeID")]
    pub node_id: String,
    pub invalid_values: BTreeMap<String, String>,
    pub source: Option<SourceLocation>,
}

/// Content cut off below a clipping node that cannot scroll.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticOverflowIssue {
    #[serde(rename = "nodeID")]
    pub node_id: String,
    pub debug_name: Option<String>,
    /// Points of content below the node's bottom edge.
    pub hidden_height: f64,
    pub source: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLayoutSummary {
    pub total: usize,
    pub invalid_geometry: Vec<DiagnosticGeometryIssue>,
    pub clipped_overflow: Vec<DiagnosticOverflowIssue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticOverview {
    pub node_count: usize,
    pub component_count: usize,
    pub state_observation_available: bool,
    pub timeline_available: bool,
    pub has_root: bool,
    pub layout_issues: DiagnosticLayoutSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsStateQuery {
    pub metadata: DiagnosticPage<StateDescriptor>,
    pub values: Vec<StateValue>,
    #[serde(rename = "missingIDs")]
    pub missing_ids: Vec<String>,
}

/// Error type shared by every diagnostic read.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticError {
    InvalidConfiguration(String),
    MissingNode(String),
    UnknownTool(String),
    InvalidResult,
}

impl DiagnosticError {
    pub fn message(&self) -> String {
        match self {
            Self::InvalidConfiguration(message) => message.clone(),
            Self::MissingNode(id) => format!("Node is missing or unmounted: {id}"),
            Self::UnknownTool(name) => format!("Unknown tool: {name}"),
            Self::InvalidResult => "Tool returned invalid JSON or exceeded 512 KiB.".into(),
        }
    }
}

impl std::fmt::Display for DiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for DiagnosticError {}

impl From<DiagnosticError> for crate::agent::AgentError {
    fn from(error: DiagnosticError) -> Self {
        match error {
            DiagnosticError::InvalidConfiguration(message) => {
                crate::agent::AgentError::InvalidConfiguration(message)
            }
            DiagnosticError::UnknownTool(name) => crate::agent::AgentError::UnknownTool(name),
            DiagnosticError::MissingNode(id) => crate::agent::AgentError::InvalidConfiguration(
                format!("Node is missing or unmounted: {id}"),
            ),
            DiagnosticError::InvalidResult => crate::agent::AgentError::InvalidResult,
        }
    }
}

/// Where diagnostics read opted-in state. Evaluated on the scene thread.
pub trait DiagnosticStateSource {
    fn descriptors(&self) -> Vec<StateDescriptor>;
    fn values(&self, ids: &[String]) -> Vec<StateValue>;
}

impl DiagnosticStateSource for StateRegistry {
    fn descriptors(&self) -> Vec<StateDescriptor> {
        StateRegistry::descriptors(self)
    }
    fn values(&self, ids: &[String]) -> Vec<StateValue> {
        StateRegistry::values(self, ids)
    }
}

/// Read-only facade over the same scene data the inspector uses.
pub struct Diagnostics {
    tree: Rc<RefCell<NodeTree>>,
    inspector: SceneInspector,
    state: Option<Rc<dyn DiagnosticStateSource>>,
    traces: RefCell<TraceStore>,
    /// Optional memory provider the host installs.
    pub memory: RefCell<Option<Rc<dyn Fn() -> DiagnosticMemorySnapshot>>>,
}

impl Diagnostics {
    pub fn new(tree: Rc<RefCell<NodeTree>>, state: Option<StateRegistry>) -> Self {
        Self::with_state_source(
            tree,
            state.map(|registry| Rc::new(registry) as Rc<dyn DiagnosticStateSource>),
        )
    }

    pub fn with_state_source(
        tree: Rc<RefCell<NodeTree>>,
        state: Option<Rc<dyn DiagnosticStateSource>>,
    ) -> Self {
        Self {
            inspector: SceneInspector::new(tree.clone(), None, None),
            tree,
            state,
            traces: RefCell::new(TraceStore::default()),
            memory: RefCell::new(None),
        }
    }

    pub fn tree(&self) -> &Rc<RefCell<NodeTree>> {
        &self.tree
    }

    pub fn inspector(&self) -> &SceneInspector {
        &self.inspector
    }

    pub fn state_observation_available(&self) -> bool {
        self.state.is_some()
    }

    /// Flatten every live node with its semantics and presentation.
    fn nodes(&self, value_node_id: Option<&str>) -> Vec<DiagnosticNode> {
        let root = self.tree.borrow().root();
        let Some(root) = root else { return Vec::new() };
        let presentation = capture_presentation(&root);
        let mut meanings: BTreeMap<String, AccessibilitySemantics> = BTreeMap::new();
        let mut truncated: Option<bool> = None;
        let mut stack = vec![root.clone()];
        while let Some(node) = stack.pop() {
            let id = node.id().0.to_string();
            if let Some(mut semantics) = node.accessibility()
                && !semantics.is_hidden
            {
                semantics.label = AccessibilityTree::resolved_label(&node);
                if semantics.state.is_secure {
                    semantics.value = "<redacted>".into();
                }
                if Some(id.as_str()) == value_node_id {
                    // Secure values must not even invoke a lazy provider.
                    let value = if semantics.state.is_secure {
                        "<redacted>".to_string()
                    } else {
                        node.resolved_accessibility_value()
                    };
                    let preview: String = value.chars().take(4097).collect();
                    truncated = Some(preview.chars().count() > 4096);
                    semantics.value = preview.chars().take(4096).collect();
                } else if matches!(
                    semantics.role,
                    cove_core::scene::AccessibilityRole::TextField
                ) && !semantics.state.is_secure
                {
                    // A field's draft is private: the default record reports the
                    // meaning, not what the user has typed. Reading it takes an
                    // explicit `include_value` request. A secure field keeps its
                    // redaction marker so clients can tell it apart.
                    semantics.value.clear();
                }
                meanings.insert(id, semantics);
            }
            stack.extend(node.children());
        }
        let snapshot = self.inspector.snapshot();
        let Some(summary_root) = snapshot.root else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut stack = vec![(summary_root, None)];
        while let Some((mut summary, parent)) = stack.pop() {
            let children = std::mem::take(&mut summary.children);
            if let Some(entry) = presentation.get(&summary.id) {
                result.push(DiagnosticNode {
                    value_truncated: (Some(summary.id.as_str()) == value_node_id)
                        .then(|| truncated.unwrap_or(false)),
                    semantics: meanings.get(&summary.id).cloned(),
                    node: summary.clone(),
                    parent_id: parent.clone(),
                    presentation: entry.clone(),
                });
            }
            let id = summary.id.clone();
            // `stack` is LIFO, so push children in reverse to visit them in
            // document order.
            for child in children.into_iter().rev() {
                stack.push((child, Some(id.clone())));
            }
        }
        result
    }

    /// Query live nodes by label, value, help, role or source.
    #[allow(clippy::too_many_arguments)]
    pub fn query_nodes(
        &self,
        id: Option<&str>,
        text: Option<&str>,
        role: Option<&str>,
        source: Option<&str>,
        visible_only: bool,
        input_enabled_only: bool,
        include_value: bool,
        offset: usize,
        limit: usize,
    ) -> Result<DiagnosticPage<DiagnosticNode>, DiagnosticError> {
        if include_value && id.is_none() {
            return Err(DiagnosticError::InvalidConfiguration(
                "include_value requires one explicit node_id.".into(),
            ));
        }
        let all = self.nodes(include_value.then_some(id).flatten());
        if let Some(id) = id
            && !all.iter().any(|item| item.node.id == id)
        {
            return Err(DiagnosticError::MissingNode(id.into()));
        }
        let text = text.map(str::to_lowercase);
        let source = source.map(str::to_lowercase);
        let matches: Vec<DiagnosticNode> =
            all.into_iter()
                .filter(|item| {
                    let node = &item.node;
                    if id.is_some_and(|id| node.id != id) {
                        return false;
                    }
                    if visible_only && !item.presentation.is_visible {
                        return false;
                    }
                    if input_enabled_only && !item.presentation.input_enabled {
                        return false;
                    }
                    if let Some(text) = &text {
                        let candidates = [
                            node.debug_name.clone(),
                            node.view_tag.clone(),
                            item.semantics.as_ref().map(|s| s.label.clone()),
                            item.semantics.as_ref().map(|s| s.value.clone()),
                            item.semantics.as_ref().map(|s| s.help.clone()),
                        ];
                        if !candidates
                            .into_iter()
                            .flatten()
                            .any(|candidate| candidate.to_lowercase().contains(text))
                        {
                            return false;
                        }
                    }
                    if let Some(role) = role {
                        let matches = node
                            .semantic_role
                            .as_deref()
                            .is_some_and(|name| name.eq_ignore_ascii_case(role))
                            || node
                                .layout_role
                                .as_deref()
                                .is_some_and(|name| name.eq_ignore_ascii_case(role))
                            || item.semantics.as_ref().is_some_and(|semantics| {
                                format!("{:?}", semantics.role).eq_ignore_ascii_case(role)
                            });
                        if !matches {
                            return false;
                        }
                    }
                    if let Some(source) = &source {
                        let matches = node.source.as_ref().is_some_and(|location| {
                            location.file_path.to_lowercase().contains(source)
                        });
                        if !matches {
                            return false;
                        }
                    }
                    true
                })
                .collect();
        Ok(DiagnosticPage::new(&matches, offset, limit))
    }

    /// Nodes that own components, sorted by total inclusive CPU time.
    pub fn query_recomposition(
        &self,
        text: Option<&str>,
        minimum_count: u64,
        offset: usize,
        limit: usize,
    ) -> DiagnosticPage<DiagnosticNode> {
        let text = text.map(str::to_lowercase);
        let mut matches: Vec<DiagnosticNode> = self
            .nodes(None)
            .into_iter()
            .filter(|item| {
                let Some(metrics) = &item.node.recomposition else {
                    return false;
                };
                if metrics.count < minimum_count {
                    return false;
                }
                match &text {
                    None => true,
                    Some(text) => [item.node.debug_name.clone(), item.node.view_tag.clone()]
                        .into_iter()
                        .flatten()
                        .any(|candidate| candidate.to_lowercase().contains(text)),
                }
            })
            .collect();
        matches.sort_by(|left, right| {
            let left_metrics = left.node.recomposition.as_ref().expect("filtered");
            let right_metrics = right.node.recomposition.as_ref().expect("filtered");
            right_metrics
                .total_ms
                .partial_cmp(&left_metrics.total_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(right_metrics.count.cmp(&left_metrics.count))
                .then(left.node.id.cmp(&right.node.id))
        });
        DiagnosticPage::new(&matches, offset, limit)
    }

    /// List opted-in state metadata, reading summaries only for explicit ids.
    pub fn query_state(
        &self,
        ids: &[String],
        scope_id: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<DiagnosticsStateQuery, DiagnosticError> {
        let Some(state) = &self.state else {
            return Err(DiagnosticError::InvalidConfiguration(
                "State observation is unavailable in this host.".into(),
            ));
        };
        let watch = StateWatch { ids: ids.to_vec() };
        if !watch.is_valid() {
            return Err(DiagnosticError::InvalidConfiguration(
                "Invalid state IDs.".into(),
            ));
        }
        let registered: Vec<StateDescriptor> = state
            .descriptors()
            .into_iter()
            .filter(|descriptor| {
                scope_id.is_none_or(|scope| descriptor.scope_id.as_deref() == Some(scope))
            })
            .collect();
        let allowed: std::collections::BTreeSet<String> = registered
            .iter()
            .map(|descriptor| descriptor.id.clone())
            .collect();
        let requested: Vec<String> = ids
            .iter()
            .filter(|id| allowed.contains(*id))
            .cloned()
            .collect();
        let missing: Vec<String> = ids
            .iter()
            .filter(|id| !allowed.contains(*id))
            .cloned()
            .collect();
        Ok(DiagnosticsStateQuery {
            metadata: DiagnosticPage::new(&registered, offset, limit),
            values: state.values(&requested),
            missing_ids: missing,
        })
    }

    /// State observation for the DevTools wire, matching Swift's
    /// `StateRegistry.observation(ids:)`.
    pub fn state_observation(&self, ids: &[String]) -> StateObservation {
        let Some(state) = &self.state else {
            return StateObservation {
                registered: Vec::new(),
                values: Vec::new(),
            };
        };
        StateObservation {
            registered: state
                .descriptors()
                .into_iter()
                .map(|descriptor| cove_devtools_protocol::RegisteredState {
                    id: descriptor.id,
                    name: descriptor.name,
                    value_type: descriptor.value_type,
                    scope_id: descriptor.scope_id,
                })
                .collect(),
            values: state
                .values(ids)
                .into_iter()
                .map(|value| cove_devtools_protocol::ObservedState {
                    id: value.id,
                    summary: value.summary,
                    truncated: value.truncated,
                })
                .collect(),
        }
    }

    pub fn layout_issues(&self, limit: usize) -> DiagnosticLayoutSummary {
        let limit = limit.clamp(1, 100);
        let geometry: Vec<DiagnosticGeometryIssue> = self
            .nodes(None)
            .into_iter()
            .filter_map(|item| {
                let frame = item.node.frame.clone();
                let values = [
                    ("x", frame.x),
                    ("y", frame.y),
                    ("width", frame.w),
                    ("height", frame.h),
                ];
                let invalid: BTreeMap<String, String> = values
                    .into_iter()
                    .filter(|(name, value)| {
                        !value.is_finite() || (matches!(*name, "width" | "height") && *value < 0.0)
                    })
                    .map(|(name, value)| (name.to_string(), format!("{value}")))
                    .collect();
                (!invalid.is_empty()).then(|| DiagnosticGeometryIssue {
                    node_id: item.node.id.clone(),
                    invalid_values: invalid,
                    source: item.node.source.clone(),
                })
            })
            .collect();
        let overflow = self
            .tree
            .borrow()
            .root()
            .map(|root| collect_overflow(&root))
            .unwrap_or_default();
        DiagnosticLayoutSummary {
            total: geometry.len() + overflow.len(),
            invalid_geometry: geometry.into_iter().take(limit).collect(),
            clipped_overflow: overflow.into_iter().take(limit).collect(),
        }
    }

    pub fn overview(&self, limit: usize) -> DiagnosticOverview {
        let nodes = self
            .query_nodes(None, None, None, None, false, false, false, 0, 1)
            .unwrap_or(DiagnosticPage {
                items: Vec::new(),
                total: 0,
                next_offset: None,
            });
        DiagnosticOverview {
            node_count: nodes.total,
            component_count: self.query_recomposition(None, 0, 0, 1).total,
            state_observation_available: self.state.is_some(),
            timeline_available: true,
            has_root: nodes.total > 0,
            layout_issues: self.layout_issues(limit),
        }
    }

    /// Capture a bounded CPU timeline window.
    pub fn capture_trace(&self, duration_ms: u64) -> Result<TraceCapture, DiagnosticError> {
        self.traces.borrow_mut().capture(&self.tree, duration_ms)
    }

    /// Open a capture window without blocking. The host keeps running, emits
    /// spans, then calls [`Self::finish_capture`] within `duration_ms`.
    pub fn begin_capture(&self, duration_ms: u64) -> Result<(), DiagnosticError> {
        self.traces.borrow_mut().begin(&self.tree, duration_ms)
    }

    /// Close the window opened by [`Self::begin_capture`] and publish its spans.
    pub fn finish_capture(&self) -> Result<TraceCapture, DiagnosticError> {
        self.traces.borrow_mut().finish(&self.tree)
    }

    pub fn query_trace(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<TraceQuery, DiagnosticError> {
        self.traces.borrow().query(id, offset, limit)
    }

    pub fn compare_traces(
        &self,
        baseline: &str,
        candidate: &str,
    ) -> Result<TraceComparison, DiagnosticError> {
        self.traces.borrow().compare(baseline, candidate)
    }

    pub fn reset_captures(&self) {
        self.traces.borrow_mut().reset();
    }

    /// Handle a bounded, read-only DevTools request. Trace capture stays on the
    /// asynchronous toolbox.
    pub fn handle_read_request(&self, request: &Envelope) -> Envelope {
        match self.read_payload(request) {
            Ok(payload) => {
                // Swift validates every tool result at one boundary: it has to
                // be valid JSON within the result budget. Clients cannot hold a
                // payload the model could not have received.
                let oversized = serde_json::to_vec(&payload)
                    .map(|bytes| bytes.len() > crate::agent::MAX_TOOL_RESULT_BYTES)
                    .unwrap_or(true);
                if oversized {
                    return request.error("bad_request", &DiagnosticError::InvalidResult.message());
                }
                let mut response = request.ok().unwrap_or_else(|| {
                    Envelope::new(format!("{}.ok", request.r#type), request.id, None)
                });
                response.payload = Some(payload);
                response
            }
            Err(error) => {
                let kind = match error {
                    DiagnosticError::UnknownTool(_) => "unsupported_tool",
                    DiagnosticError::MissingNode(_) => "stale_node",
                    _ => "bad_request",
                };
                request.error(kind, &error.message())
            }
        }
    }

    fn read_payload(&self, request: &Envelope) -> Result<serde_json::Value, DiagnosticError> {
        match request.r#type.as_str() {
            "diagnostics.tools" => {
                let empty = request
                    .payload
                    .as_ref()
                    .map(|payload| payload.as_object().is_some_and(|object| object.is_empty()))
                    .unwrap_or(true);
                if !empty {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "diagnostics.tools takes no payload".into(),
                    ));
                }
                let tools: Vec<serde_json::Value> =
                    crate::inspection::tools::Tool::immediate_read_tools()
                        .into_iter()
                        .map(|tool| {
                            serde_json::to_value(tool.definition().mcp_definition())
                                .unwrap_or_default()
                        })
                        .collect();
                Ok(serde_json::json!({ "tools": tools }))
            }
            "diagnostics.call" => {
                let Some(object) = request
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.as_object())
                else {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "diagnostics.call requires a tool name and an arguments object".into(),
                    ));
                };
                if !object.keys().all(|key| key == "name" || key == "arguments") {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "diagnostics.call requires a tool name and an arguments object".into(),
                    ));
                }
                let Some(name) = object.get("name").and_then(|value| value.as_str()) else {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "diagnostics.call requires a tool name and an arguments object".into(),
                    ));
                };
                if name.is_empty() || name.len() > 128 {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "diagnostics.call requires a tool name and an arguments object".into(),
                    ));
                }
                let arguments = object
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                if !arguments.is_object() {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "diagnostics.call requires a tool name and an arguments object".into(),
                    ));
                }
                let Some(tool) = crate::inspection::tools::Tool::parse(name) else {
                    return Err(DiagnosticError::UnknownTool(name.to_string()));
                };
                if !tool.is_immediate_read() {
                    return Err(DiagnosticError::UnknownTool(name.to_string()));
                }
                self.execute_read_json(tool, &arguments)
            }
            _ => Err(DiagnosticError::InvalidConfiguration(
                "Unsupported diagnostic request".into(),
            )),
        }
    }

    /// Run an immediate-read tool, returning the JSON the wire carries.
    pub fn execute_read_json(
        &self,
        tool: crate::inspection::tools::Tool,
        arguments: &serde_json::Value,
    ) -> Result<serde_json::Value, DiagnosticError> {
        use crate::inspection::tools::Tool;
        crate::agent::JsonSchemaValidator::validate(arguments, &tool.definition().schema)
            .map_err(|error| DiagnosticError::InvalidConfiguration(error.to_string()))?;
        let data = match tool {
            Tool::QueryUiNetwork => network_log(),
            Tool::QueryUiMemory => {
                let memory = self.memory.borrow().clone();
                serde_json::json!({ "ok": true, "data": memory.map(|provider| provider()) })
            }
            Tool::GetDiagnostics => {
                let limit = arguments
                    .get("limit")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(50) as usize;
                serde_json::json!({ "ok": true, "data": self.overview(limit) })
            }
            Tool::QueryUiNodes => {
                let page = self.query_nodes(
                    arguments.get("node_id").and_then(|value| value.as_str()),
                    arguments.get("text").and_then(|value| value.as_str()),
                    arguments.get("role").and_then(|value| value.as_str()),
                    arguments.get("source").and_then(|value| value.as_str()),
                    arguments
                        .get("visible_only")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    arguments
                        .get("input_enabled_only")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    arguments
                        .get("include_value")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    arguments
                        .get("offset")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize,
                    arguments
                        .get("limit")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(50) as usize,
                )?;
                self.validate_geometry(&page)?;
                if arguments
                    .get("compact")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
                {
                    let compact = page.project(|item| DiagnosticCompactNode::from_node(&item));
                    serde_json::json!({ "ok": true, "data": compact })
                } else {
                    serde_json::json!({ "ok": true, "data": page })
                }
            }
            Tool::QueryUiRecomposition => {
                let page = self.query_recomposition(
                    arguments.get("text").and_then(|value| value.as_str()),
                    arguments
                        .get("minimum_count")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0),
                    arguments
                        .get("offset")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize,
                    arguments
                        .get("limit")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(50) as usize,
                );
                self.validate_geometry(&page)?;
                serde_json::json!({ "ok": true, "data": page })
            }
            Tool::QueryUiState => {
                let ids: Vec<String> = arguments
                    .get("ids")
                    .and_then(|value| value.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                let query = self.query_state(
                    &ids,
                    arguments.get("scope_id").and_then(|value| value.as_str()),
                    arguments
                        .get("offset")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize,
                    arguments
                        .get("limit")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(50) as usize,
                )?;
                serde_json::json!({ "ok": true, "data": query })
            }
            Tool::GetUiTrace => {
                let Some(id) = arguments.get("capture_id").and_then(|value| value.as_str()) else {
                    return Err(DiagnosticError::InvalidConfiguration(
                        "capture_id is required".into(),
                    ));
                };
                let query = self.query_trace(
                    id,
                    arguments
                        .get("offset")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize,
                    arguments
                        .get("limit")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(50) as usize,
                )?;
                serde_json::json!({ "ok": true, "data": query })
            }
            Tool::CompareUiTraces => {
                let baseline = arguments
                    .get("baseline_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let candidate = arguments
                    .get("candidate_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let comparison = self.compare_traces(baseline, candidate)?;
                serde_json::json!({ "ok": true, "data": comparison })
            }
            Tool::ListSkills => {
                let library = crate::inspection::tools::default_skill_library();
                let items: Vec<serde_json::Value> = library
                    .skills
                    .iter()
                    .map(|skill| serde_json::json!({ "name": skill.name, "description": skill.description }))
                    .collect();
                serde_json::json!({ "ok": true, "data": items })
            }
            Tool::ReadSkill => {
                let name = arguments
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let library = crate::inspection::tools::default_skill_library();
                let skill = library
                    .read(name)
                    .map_err(|error| DiagnosticError::InvalidConfiguration(error.to_string()))?;
                serde_json::json!({ "ok": true, "data": skill })
            }
            Tool::CaptureUiTrace => {
                return Err(DiagnosticError::InvalidConfiguration(
                    "capture_ui_trace requires the asynchronous toolbox.".into(),
                ));
            }
            Tool::StartUiProfile
            | Tool::StopUiProfile
            | Tool::GetUiProfile
            | Tool::QueryUiHotspots
            | Tool::StartUiAllocationProfile
            | Tool::StopUiAllocationProfile
            | Tool::QueryUiAllocationSites => {
                return Err(DiagnosticError::InvalidConfiguration(
                    "UI profile commands require the asynchronous toolbox.".into(),
                ));
            }
        };
        Ok(data)
    }

    /// Reject a page that would hand a client a node with unrepresentable
    /// geometry, pointing at the layout summary instead.
    fn validate_geometry(
        &self,
        page: &DiagnosticPage<DiagnosticNode>,
    ) -> Result<(), DiagnosticError> {
        let invalid = page.items.iter().find_map(|item| {
            let frame = item.node.frame.clone();
            let invalid: Vec<String> = [
                ("x", frame.x),
                ("y", frame.y),
                ("width", frame.w),
                ("height", frame.h),
            ]
            .into_iter()
            .filter(|(name, value)| {
                !value.is_finite() || (matches!(*name, "width" | "height") && *value < 0.0)
            })
            .map(|(name, _)| name.to_string())
            .collect();
            (!invalid.is_empty()).then(|| (item.node.id.clone(), invalid))
        });
        match invalid {
            Some((node_id, _)) => Err(DiagnosticError::InvalidConfiguration(format!(
                "Node {node_id} has invalid layout geometry. Use get_diagnostics.layoutIssues for values and source."
            ))),
            None => Ok(()),
        }
    }
}

fn collect_overflow(root: &Node) -> Vec<DiagnosticOverflowIssue> {
    fn content_bottom(node: &Node) -> Option<f32> {
        if !node.is_layout_visible() {
            return None;
        }
        let frame = node.absolute_frame();
        let mut bottom =
            (frame.width > 0.0 && frame.height > 0.0).then_some(frame.y + frame.height);
        if node.clips_to_bounds() {
            return bottom;
        }
        for child in node.children() {
            if let Some(child_bottom) = content_bottom(&child) {
                bottom = Some(bottom.map_or(child_bottom, |current| current.max(child_bottom)));
            }
        }
        bottom
    }
    let mut issues = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if !node.is_layout_visible() {
            continue;
        }
        let frame = node.absolute_frame();
        if node.clips_to_bounds()
            && !node.is_scroll_viewport()
            && frame.width >= 1.0
            && frame.height >= 1.0
        {
            let bottom = node
                .children()
                .iter()
                .filter_map(content_bottom)
                .fold(f32::NEG_INFINITY, f32::max);
            if bottom.is_finite() && bottom - (frame.y + frame.height) > 1.0 {
                // The nearest authored ancestor supplies the source location.
                let mut authored = Some(node.clone());
                let source = loop {
                    let Some(current) = authored else { break None };
                    if let Some(location) = current.source_location() {
                        break Some(SourceLocation {
                            file_id: location.file_id,
                            file_path: location.file_path,
                            line: location.line,
                            column: location.column,
                        });
                    }
                    authored = current.parent();
                };
                issues.push(DiagnosticOverflowIssue {
                    node_id: node.id().0.to_string(),
                    debug_name: node.attachment::<String>(DebugAttachmentKey::DEBUG_NAME),
                    hidden_height: (bottom - (frame.y + frame.height)) as f64,
                    source,
                });
            }
        }
        // LIFO stack: push in reverse so siblings keep document order.
        for child in node.children().into_iter().rev() {
            stack.push(child);
        }
    }
    issues
}

/// Compute visible bounds and input availability for every node.
fn capture_presentation(root: &Node) -> BTreeMap<String, DiagnosticPresentation> {
    let mut result = BTreeMap::new();
    // The focus chain of the owning scene's active modal scope, when one is
    // mounted. A linear walk avoids repeatedly descending the same deep
    // composition subtrees the way a lazy recursive search would.
    fn modal_input(root: &Node) -> Option<std::rc::Rc<cove_core::scene::FocusChain>> {
        let mut pending = vec![root.clone()];
        while let Some(candidate) = pending.pop() {
            if let Some(resource) =
                candidate.first_resource::<cove_core::scene::ModalFocusResource>()
                && let Some(chain) = resource.bound_focus_chain()
                && chain.has_modal_scope()
            {
                return Some(chain);
            }
            pending.extend(candidate.children().into_iter().rev());
        }
        None
    }
    let input_scope = modal_input(root);
    fn visit(
        node: &Node,
        clip: Option<Rect>,
        draws: bool,
        input: bool,
        input_scope: Option<&std::rc::Rc<cove_core::scene::FocusChain>>,
        result: &mut BTreeMap<String, DiagnosticPresentation>,
    ) -> Option<Rect> {
        let frame = node.frame();
        let bounds = node.absolute_frame();
        let valid = [
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        ]
        .iter()
        .all(|value| value.is_finite())
            && frame.width >= 0.0
            && frame.height >= 0.0;
        let draws = draws
            && valid
            && node.is_layout_visible()
            && node.opacity() > 0.0
            && node.subtree_opacity() > 0.0;
        let input = input
            && node.is_layout_visible()
            && node.allows_hit_testing()
            && node.is_interaction_enabled()
            && node
                .accessibility()
                .is_none_or(|semantics| semantics.state.is_enabled);
        let mut own = None;
        if draws && bounds.width > 0.0 && bounds.height > 0.0 {
            let visible = match clip {
                Some(clip) => clip.intersection(bounds),
                None => Some(bounds),
            };
            if let Some(visible) = visible
                && visible.width > 0.0
                && visible.height > 0.0
            {
                own = Some(visible);
            }
        }
        let child_clip = if node.clips_to_bounds() {
            if valid {
                Some(match clip {
                    Some(clip) => clip.intersection(bounds).unwrap_or_default(),
                    None => bounds,
                })
            } else {
                None
            }
        } else {
            clip
        };
        let mut descendants: Option<Rect> = None;
        for child in node.children() {
            if let Some(frame) = visit(&child, child_clip, draws, input, input_scope, result) {
                descendants = Some(descendants.map_or(frame, |current| current.union(frame)));
            }
        }
        // Layout-transparent component anchors inherit their visible
        // descendants without fabricating an independent hit-test box.
        // A layout-transparent component anchor inherits its visible
        // descendants; Swift's `bounds.isEmpty` is true on either empty axis.
        let empty_bounds = bounds.width <= 0.0 || bounds.height <= 0.0;
        let visible = if node.layout_node().is_none() && empty_bounds {
            descendants
        } else {
            own
        };
        result.insert(
            node.id().0.to_string(),
            DiagnosticPresentation {
                is_visible: visible.is_some(),
                input_enabled: input
                    && visible.is_some()
                    && input_scope.is_none_or(|chain| chain.permits_input(node)),
                visible_frame: visible.map(|frame| NodeFrame {
                    x: frame.x as f64,
                    y: frame.y as f64,
                    w: frame.width as f64,
                    h: frame.height as f64,
                }),
            },
        );
        match own {
            Some(own) => Some(descendants.map_or(own, |current| own.union(current))),
            None => descendants,
        }
    }
    let frame = root.absolute_frame();
    visit(
        root,
        (frame.width > 0.0 && frame.height > 0.0).then_some(frame),
        true,
        true,
        input_scope.as_ref(),
        &mut result,
    );
    result
}

// MARK: - Trace capture

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePhase {
    pub phase: String,
    pub count: usize,
    pub average_ms: f64,
    pub p95_ms: f64,
    pub maximum_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceCapture {
    pub id: String,
    pub duration_ms: f64,
    pub event_count: usize,
    pub dropped: usize,
    pub phases: Vec<TracePhase>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceQuery {
    pub capture: TraceCapture,
    pub events: DiagnosticPage<cove_core::scene::TimelineEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePhaseChange {
    pub phase: String,
    pub baseline: Option<TracePhase>,
    pub candidate: Option<TracePhase>,
    pub average_delta_ms: Option<f64>,
    pub p95_delta_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceComparison {
    pub baseline: TraceCapture,
    pub candidate: TraceCapture,
    pub changes: Vec<TracePhaseChange>,
}

struct TraceRecord {
    summary: TraceCapture,
    events: Vec<cove_core::scene::TimelineEvent>,
}

#[derive(Default)]
struct TraceStore {
    records: Vec<TraceRecord>,
    generation: u64,
    /// An open `begin_capture` window: the sequence cursor it started after and
    /// the instant it opened. `None` when no capture is in flight.
    pending: Option<(u64, std::time::Instant)>,
}

/// Bounded retention: at most eight captures.
const MAX_CAPTURES: usize = 8;

impl TraceStore {
    fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.records.clear();
        self.pending = None;
    }

    /// Open a capture window and return immediately. Spans the host emits while
    /// the window is open are collected by [`Self::finish`].
    fn begin(
        &mut self,
        tree: &Rc<RefCell<NodeTree>>,
        milliseconds: u64,
    ) -> Result<(), DiagnosticError> {
        self.check_duration(milliseconds)?;
        if self.pending.is_some() {
            return Err(DiagnosticError::InvalidConfiguration(
                "Capture duration must be 50–5000 ms; only one capture may run at a time.".into(),
            ));
        }
        let after = tree.borrow().timeline.borrow().latest_sequence();
        tree.borrow().timeline.borrow_mut().begin_recording();
        self.pending = Some((after, std::time::Instant::now()));
        Ok(())
    }

    /// Close an open window and publish the spans recorded inside it.
    fn finish(&mut self, tree: &Rc<RefCell<NodeTree>>) -> Result<TraceCapture, DiagnosticError> {
        let Some((after, started)) = self.pending.take() else {
            return Err(DiagnosticError::InvalidConfiguration(
                "No capture is recording; open one first.".into(),
            ));
        };
        tree.borrow().timeline.borrow_mut().end_recording();
        self.publish(tree, after, started)
    }

    fn check_duration(&self, milliseconds: u64) -> Result<(), DiagnosticError> {
        if (50..=5000).contains(&milliseconds) {
            Ok(())
        } else {
            Err(DiagnosticError::InvalidConfiguration(
                "Capture duration must be 50–5000 ms; only one capture may run at a time.".into(),
            ))
        }
    }

    /// Gather the spans after `after`, summarise them by phase and retain them.
    fn publish(
        &mut self,
        tree: &Rc<RefCell<NodeTree>>,
        after: u64,
        started: std::time::Instant,
    ) -> Result<TraceCapture, DiagnosticError> {
        let dropped_before = tree.borrow().timeline.borrow().dropped();
        let events = tree.borrow().timeline.borrow().events_after(after);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;

        let mut by_phase: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for event in &events {
            by_phase
                .entry(event.phase.clone())
                .or_default()
                .push(event.duration_ms);
        }
        let phases: Vec<TracePhase> = by_phase
            .into_iter()
            .map(|(phase, mut values)| {
                values.sort_by(|left, right| {
                    left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
                });
                let count = values.len();
                let sum: f64 = values.iter().sum();
                let index = (((count.saturating_sub(1)) as f64) * 0.95).ceil() as usize;
                TracePhase {
                    phase,
                    count,
                    average_ms: if count > 0 { sum / count as f64 } else { 0.0 },
                    p95_ms: values
                        .get(index.min(count.saturating_sub(1)))
                        .copied()
                        .unwrap_or(0.0),
                    maximum_ms: values.last().copied().unwrap_or(0.0),
                }
            })
            .collect();
        let summary = TraceCapture {
            id: next_capture_id(),
            duration_ms: elapsed,
            event_count: events.len(),
            dropped: dropped_before as usize,
            phases,
        };
        if self.records.len() == MAX_CAPTURES {
            self.records.remove(0);
        }
        self.records.push(TraceRecord {
            summary: summary.clone(),
            events,
        });
        Ok(summary)
    }

    /// Sample the timeline for `milliseconds` on the calling thread. Usable
    /// without a host pumping frames; a host-driven session should pair
    /// [`Self::begin`] with [`Self::finish`] so its spans land in the window.
    fn capture(
        &mut self,
        tree: &Rc<RefCell<NodeTree>>,
        milliseconds: u64,
    ) -> Result<TraceCapture, DiagnosticError> {
        self.check_duration(milliseconds)?;
        if self.pending.is_some() {
            return Err(DiagnosticError::InvalidConfiguration(
                "Capture duration must be 50–5000 ms; only one capture may run at a time.".into(),
            ));
        }
        let started = std::time::Instant::now();
        let after = tree.borrow().timeline.borrow().latest_sequence();
        // The recording window is enabled for the whole capture so spans
        // produced by the host are retained even if the timeline was idle.
        tree.borrow().timeline.borrow_mut().begin_recording();
        std::thread::sleep(std::time::Duration::from_millis(milliseconds));
        tree.borrow().timeline.borrow_mut().end_recording();
        self.publish(tree, after, started)
    }

    fn record(&self, id: &str) -> Result<&TraceRecord, DiagnosticError> {
        self.records
            .iter()
            .find(|record| record.summary.id == id)
            .ok_or_else(|| {
                DiagnosticError::InvalidConfiguration(format!(
                    "Capture is missing or expired: {id}"
                ))
            })
    }

    fn query(&self, id: &str, offset: usize, limit: usize) -> Result<TraceQuery, DiagnosticError> {
        let record = self.record(id)?;
        Ok(TraceQuery {
            capture: record.summary.clone(),
            events: DiagnosticPage::new(&record.events, offset, limit),
        })
    }

    fn compare(
        &self,
        baseline_id: &str,
        candidate_id: &str,
    ) -> Result<TraceComparison, DiagnosticError> {
        let baseline = self.record(baseline_id)?.summary.clone();
        let candidate = self.record(candidate_id)?.summary.clone();
        let mut names: Vec<String> = baseline
            .phases
            .iter()
            .chain(candidate.phases.iter())
            .map(|phase| phase.phase.clone())
            .collect();
        names.sort();
        names.dedup();
        let changes = names
            .into_iter()
            .map(|phase| {
                let before = baseline
                    .phases
                    .iter()
                    .find(|candidate| candidate.phase == phase)
                    .cloned();
                let after = candidate
                    .phases
                    .iter()
                    .find(|candidate| candidate.phase == phase)
                    .cloned();
                TracePhaseChange {
                    average_delta_ms: match (&before, &after) {
                        (Some(before), Some(after)) => Some(after.average_ms - before.average_ms),
                        _ => None,
                    },
                    p95_delta_ms: match (&before, &after) {
                        (Some(before), Some(after)) => Some(after.p95_ms - before.p95_ms),
                        _ => None,
                    },
                    phase,
                    baseline: before,
                    candidate: after,
                }
            })
            .collect();
        Ok(TraceComparison {
            baseline,
            candidate,
            changes,
        })
    }
}

fn network_log() -> serde_json::Value {
    let records: Vec<_> = cove_http::recording::snapshot()
        .iter()
        .map(|record| {
            serde_json::json!({
                "id": record.id,
                "method": record.method,
                "url": record.url,
                "status": record.status,
                "state": record.state.as_str(),
                "error": record.error,
                "startedAt": record.started_at_ms,
                "ttfbMs": record.ttfb_ms,
                "durationMs": record.duration_ms,
                "requestBytes": record.request_bytes,
                "responseBytes": record.response_bytes,
                "requestHeaders": record.request_headers,
                "responseHeaders": record.response_headers,
                "requestBody": String::from_utf8_lossy(&record.request_body),
                "requestBodyTruncated": record.request_body_truncated,
                "responseBody": String::from_utf8_lossy(&record.response_body),
                "responseBodyTruncated": record.response_body_truncated,
            })
        })
        .collect();
    serde_json::json!({
        "ok": true,
        "data": {
            "records": records,
            "limit": cove_http::recording::MAX_RECORDS,
            // The live flag, so a paused recorder is visible before any control
            // call; the client must not infer recording state from the table.
            "enabled": cove_http::recording::is_enabled(),
        }
    })
}

fn next_capture_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("trace-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Swift names for the diagnostic records. Each is the same value as the Rust
/// name it points at, so the two spellings cannot describe different shapes.
pub type UIDiagnosticNode = DiagnosticNode;
pub type UIDiagnosticCompactNode = DiagnosticCompactNode;
pub type UIDiagnosticPage<T> = DiagnosticPage<T>;
pub type UIDiagnosticGeometryIssue = DiagnosticGeometryIssue;
pub type UIDiagnosticLayoutSummary = DiagnosticLayoutSummary;
pub type UIDiagnosticOverflowIssue = DiagnosticOverflowIssue;
pub type UIDiagnosticMemorySnapshot = DiagnosticMemorySnapshot;
pub type UIDiagnosticPresentation = DiagnosticPresentation;
pub type UIDiagnosticOverview = DiagnosticOverview;
pub type UIStateQuery = DiagnosticsStateQuery;
pub type UITraceCapture = TraceCapture;
pub type UITraceComparison = TraceComparison;
pub type UITracePhase = TracePhase;
pub type UITracePhaseChange = TracePhaseChange;
pub type UITraceQuery = TraceQuery;

/// Swift's `UIDiagnostics`: the read-only facade an agent or MCP client holds.
/// It is `Diagnostics` itself — every query above already lives there.
pub type UIDiagnostics = Diagnostics;

/// The canonical diagnostic tool registry, under the Swift name.
pub use crate::inspection::tools::{Tool as UIDiagnosticTool, Tools as UIDiagnosticTools};
