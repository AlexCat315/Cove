//! Scene inspection: snapshot builder and the temporary-style editor.
//!
//! Both types reach the live tree through `Rc<RefCell<NodeTree>>` because the
//! composition graph owns it. Swift used `@MainActor` to serialize access; here
//! the contract is the same — every method must run on the UI thread — and the
//! shared ownership simply makes that explicit.
use crate::inspection::editor::SceneEditor;
use cove_core::Rect;
use cove_core::scene::{
    InvalidationLog, InvalidationPhase, InvalidationSource, LayoutInsets, Node, NodeTree,
    Overflow, RenderTree,
};
use cove_devtools_protocol::{
    InputInventory, InspectionInsets, InvalidationRecord, MAX_TREE_PAGE_DEPTH, MAX_TREE_PAGE_NODES,
    NodeConstraints, NodeFlags, NodeFrame, NodeLayoutInfo, NodePoint, NodeStyleInfo, NodeSummary,
    Recomposition,
    RecompositionReason, RenderInventory, SourceLocation, TreePage, TreePageNode, TreeSnapshot,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Attachment keys the composition layer and hosts use to label nodes.
pub struct DebugAttachmentKey;
impl DebugAttachmentKey {
    pub const DEBUG_NAME: &'static str = "CoveUI.debug.name";
    pub const LAYOUT_ROLE: &'static str = "CoveUI.layout.role";
    pub const SEMANTIC_ROLE: &'static str = "CoveUI.semantic.role";
}

/// The role spelling clients read: Swift publishes the enum's raw value
/// (`listItem`), not its `Debug` name (`ListItem`). The serde rename is the
/// single source of truth for that spelling.
pub(crate) fn role_name(role: cove_core::scene::AccessibilityRole) -> String {
    serde_json::to_value(role)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{role:?}"))
}

/// Search for a live node by its element ID.
pub(crate) fn find_in(node: &Node, id: &str) -> Option<Node> {
    if SceneInspector::identifier(node) == id {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_in(&child, id) {
            return Some(found);
        }
    }
    None
}

/// Why a bounded tree page could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreePageError {
    /// `depth` is outside `1..=MAX_TREE_PAGE_DEPTH`.
    Depth,
    /// `limit` is outside `1..=MAX_TREE_PAGE_NODES`.
    Limit,
    /// The requested root ID is empty or longer than an element ID can be.
    NodeId,
    /// No live node has the requested element ID.
    Stale(String),
}

impl TreePageError {
    /// The fault code the wire reports for this error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Stale(_) => "stale_node",
            Self::Depth | Self::Limit | Self::NodeId => "invalid_request",
        }
    }
}

impl std::fmt::Display for TreePageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Depth => write!(
                formatter,
                "depth must be an integer from 1 to {MAX_TREE_PAGE_DEPTH}."
            ),
            Self::Limit => write!(
                formatter,
                "limit must be an integer from 1 to {MAX_TREE_PAGE_NODES}."
            ),
            Self::NodeId => {
                formatter.write_str("node_id must be a live element ID of 1 to 256 bytes.")
            }
            Self::Stale(id) => write!(formatter, "No live node has element ID {id}."),
        }
    }
}

impl std::error::Error for TreePageError {}

/// Pre-order walk that stops adding nodes at the page's node limit.
struct PageBuilder {
    limit: usize,
    node_count: usize,
    truncated: bool,
}

impl PageBuilder {
    /// Adds `node` when the page still has room and returns its summary.
    ///
    /// `levels` is how many node levels remain below and including `node`.
    fn build(&mut self, node: &Node, levels: usize) -> Option<TreePageNode> {
        if self.node_count >= self.limit {
            self.truncated = true;
            return None;
        }
        self.node_count += 1;
        let children = if levels > 1 {
            let mut children = Vec::new();
            for child in node.children() {
                match self.build(&child, levels - 1) {
                    Some(page) => children.push(page),
                    // The only reason a live child is skipped is the node
                    // limit, which `build` already recorded.
                    None => break,
                }
            }
            children
        } else {
            if !node.children().is_empty() {
                self.truncated = true;
            }
            Vec::new()
        };
        Some(page_node(node, children))
    }
}

/// Builds `NodeSummary` snapshots from a `NodeTree`.
///
/// IDs are valid for the lifetime of their nodes in this runtime.
pub struct SceneInspector {
    tree: Rc<RefCell<NodeTree>>,
    pub editor: SceneEditor,
    /// Optional log to attach to snapshots, so clients can show recent
    /// dirty-propagation events alongside the tree.
    pub invalidation_log: Option<Arc<InvalidationLog>>,
    /// Optional render tree, surfaced as `renderInventory`.
    pub render_tree: Option<RenderTree>,
}

impl SceneInspector {
    pub fn new(
        tree: Rc<RefCell<NodeTree>>,
        invalidation_log: Option<Arc<InvalidationLog>>,
        render_tree: Option<RenderTree>,
    ) -> Self {
        Self {
            editor: SceneEditor::new(tree.clone()),
            tree,
            invalidation_log,
            render_tree,
        }
    }

    /// Capture a snapshot of the current scene. Must run on the thread that
    /// mutates the tree.
    pub fn snapshot(&self) -> TreeSnapshot {
        self.build_snapshot(true)
    }

    /// A snapshot for the wire `tree` command.
    ///
    /// A live gallery publishes thousands of nodes; full layout, style and
    /// recomposition detail does not fit one protocol message. This keeps the
    /// identity, geometry, roles, flags, source and children a client needs to
    /// render and select, and leaves the detailed read to `get_diagnostics` and
    /// `query_ui_nodes`, which use [`Self::snapshot`].
    pub fn summary_snapshot(&self) -> TreeSnapshot {
        self.build_snapshot(false)
    }

    fn build_snapshot(&self, detailed: bool) -> TreeSnapshot {
        // `state()` prunes stale overrides, so it is deliberately `&self`.
        let inspection = self.editor.state();
        let invalidations = self.invalidation_log.as_ref().map(|log| {
            log.snapshot(Some(64))
                .into_iter()
                .map(encode_invalidation)
                .collect()
        });
        let render_inventory = self.render_tree.as_ref().map(|tree| RenderInventory {
            object_count: tree.object_count(),
            layer_roots: tree
                .layer_roots()
                .into_iter()
                .map(|object| object.element_id().0.to_string())
                .collect(),
        });
        let root = self.tree.borrow().root();
        let input_inventory = root.as_ref().map(|root| {
            let mut node_count = 0usize;
            let mut focusables = Vec::new();
            let mut hit_testables = Vec::new();
            let mut stack = vec![root.clone()];
            while let Some(node) = stack.pop() {
                node_count += 1;
                if node.is_focusable() {
                    focusables.push(node.id().0.to_string());
                }
                if node.is_hit_testable() {
                    hit_testables.push(node.id().0.to_string());
                }
                stack.extend(node.children());
            }
            InputInventory {
                node_count,
                focusables,
                hit_testables,
            }
        });
        TreeSnapshot {
            root: root.map(|root| {
                if detailed {
                    summarize(&root, None, &self.editor)
                } else {
                    summarize_summary(&root)
                }
            }),
            invalidations,
            render_inventory,
            input_inventory,
            inspection: Some(inspection),
        }
    }

    /// A bounded subtree rooted at `node_id`, or at the scene root when it is
    /// absent.
    ///
    /// `depth` counts node levels and includes the page root, so `depth = 1`
    /// returns the root alone. Every returned node is a live node: nodes past
    /// `depth`, past `limit` or past the caller's byte budget are omitted and
    /// reported through [`TreePage::truncated`]. The byte budget is applied by
    /// the transport, which measures the serialised page; the inspector only
    /// enforces the node and depth bounds.
    pub fn tree_page(
        &self,
        node_id: Option<&str>,
        depth: usize,
        limit: usize,
    ) -> Result<TreePage, TreePageError> {
        if !(1..=MAX_TREE_PAGE_DEPTH).contains(&depth) {
            return Err(TreePageError::Depth);
        }
        if !(1..=MAX_TREE_PAGE_NODES).contains(&limit) {
            return Err(TreePageError::Limit);
        }
        if node_id.is_some_and(|id| id.is_empty() || id.len() > 256) {
            return Err(TreePageError::NodeId);
        }
        let tree = self.tree.borrow();
        let Some(root) = tree.root() else {
            return match node_id {
                Some(id) => Err(TreePageError::Stale(id.to_owned())),
                None => Ok(TreePage {
                    root: None,
                    depth,
                    limit,
                    node_count: 0,
                    truncated: false,
                }),
            };
        };
        let start = match node_id {
            Some(id) => find_in(&root, id).ok_or_else(|| TreePageError::Stale(id.to_owned()))?,
            None => root,
        };
        let mut builder = PageBuilder {
            limit,
            node_count: 0,
            truncated: false,
        };
        let page_root = builder.build(&start, depth);
        Ok(TreePage {
            root: page_root,
            depth,
            limit,
            node_count: builder.node_count,
            truncated: builder.truncated,
        })
    }

    /// The tree this inspector reads. Handles let a caller mutate the tree
    /// between snapshots without an extra borrow.
    pub fn tree(&self) -> &Rc<RefCell<NodeTree>> {
        &self.tree
    }

    /// Find a node previously reported in a snapshot. Returns `None` when the id
    /// no longer corresponds to a live node.
    pub fn find(&self, id: &str) -> Option<Node> {
        let root = self.tree.borrow().root()?;
        find_in(&root, id)
    }

    /// The same live element ID used by node queries and inspection commands.
    pub fn identifier(node: &Node) -> String {
        node.id().0.to_string()
    }

    /// The deepest node under a content-space point.
    ///
    /// This is the same hit test platform input uses, so a client selecting a
    /// node at a point agrees with what a click would target. Nothing is
    /// dispatched and no state changes.
    pub fn node_at(&self, point: cove_core::Point) -> Option<cove_core::scene::HitResult> {
        let root = self.tree.borrow().root()?;
        cove_core::scene::HitTester::hit_test(&root, point)
    }
}

fn summarize(node: &Node, owner_scope_id: Option<&str>, editor: &SceneEditor) -> NodeSummary {
    let absolute = node.absolute_frame();
    let recomposition_metrics = node.recomposition_metrics();
    let owner = if recomposition_metrics.is_some() {
        Some(node.id().0.to_string())
    } else {
        owner_scope_id.map(str::to_string)
    };
    let children: Vec<NodeSummary> = node
        .children()
        .iter()
        .map(|child| summarize(child, owner.as_deref(), editor))
        .collect();
    let layout = node.layout_node().map(|layout| {
        let padding = layout.resolved_padding();
        let border = layout.resolved_border();
        // Flutter's "RenderFlex overflowed" warning: a non-clipping parent whose
        // child escapes its box. Children are positioned in this node's local
        // origin, so any frame outside `[0, 0, width, height]` is spilling out.
        let epsilon = 0.5;
        let box_frame = node.frame();
        let overflow_detected = layout.overflow() == Overflow::Visible
            && node.children().iter().any(|child| {
                let child_frame = child.frame();
                child_frame.x < -epsilon
                    || child_frame.y < -epsilon
                    || child_frame.x + child_frame.width > box_frame.width + epsilon
                    || child_frame.y + child_frame.height > box_frame.height + epsilon
            });
        NodeLayoutInfo {
            padding: insets(padding),
            margin: insets(layout.resolved_margin()),
            border: insets(border),
            content_width: (node.frame().width
                - (padding.left + padding.right + border.left + border.right))
                .max(0.0) as f64,
            content_height: (node.frame().height
                - (padding.top + padding.bottom + border.top + border.bottom))
                .max(0.0) as f64,
            direction: format!("{:?}", layout.direction()),
            flex_direction: format!("{:?}", layout.flex_direction()),
            justify_content: format!("{:?}", layout.justify_content()),
            align_items: format!("{:?}", layout.align_items()),
            flex_grow: layout.flex_grow() as f64,
            flex_shrink: layout.flex_shrink() as f64,
            constraints: NodeConstraints {
                min_width: layout.min_width().map(|value| value as f64),
                max_width: layout.max_width().map(|value| value as f64),
                min_height: layout.min_height().map(|value| value as f64),
                max_height: layout.max_height().map(|value| value as f64),
            },
            overflow_detected,
        }
    });
    let overrides: Vec<String> = editor
        .override_properties(&SceneInspector::identifier(node))
        .into_iter()
        .map(str::to_string)
        .collect();
    NodeSummary {
        id: SceneInspector::identifier(node),
        view_tag: node.view_tag(),
        debug_name: node.attachment::<String>(DebugAttachmentKey::DEBUG_NAME),
        frame: encode_frame(node.frame()),
        absolute_frame: Some(encode_frame(absolute)),
        content_offset: Some(NodePoint {
            x: node.content_offset().x as f64,
            y: node.content_offset().y as f64,
        }),
        z_index: Some(node.z_index() as f64),
        opacity: Some(node.opacity() as f64),
        layout_role: node.attachment::<String>(DebugAttachmentKey::LAYOUT_ROLE),
        semantic_role: semantic_role(node),
        flags: node_flags(node),
        children,
        layout,
        style: Some(NodeStyleInfo {
            background_color: SceneEditor::hex(node.background_color()),
            foreground_color: SceneEditor::hex(node.inherited_foreground_color()),
            overrides,
        }),
        source: source_location(node),
        owner_scope_id: owner,
        recomposition: recomposition_metrics.map(|metrics| Recomposition {
            count: metrics.count,
            initial_ms: metrics.initial_ms,
            last_ms: metrics.last_ms,
            total_ms: metrics.total_ms,
            max_ms: metrics.max_ms,
            reasons: metrics
                .last_reasons
                .iter()
                .map(|reason| RecompositionReason {
                    kind: reason.kind.clone(),
                    detail: reason.detail.clone(),
                    origin_scope_id: reason.origin_scope.map(|scope| scope.to_string()),
                })
                .collect(),
        }),
    }
}

/// The wire `tree` payload: identity, geometry, roles, flags and children only.
///
/// Detailed layout, style and recomposition are deliberately absent so a
/// gallery-sized tree stays inside one bounded protocol message.
fn summarize_summary(node: &Node) -> NodeSummary {
    NodeSummary {
        id: SceneInspector::identifier(node),
        view_tag: node.view_tag(),
        debug_name: node.attachment::<String>(DebugAttachmentKey::DEBUG_NAME),
        frame: encode_frame(node.frame()),
        absolute_frame: Some(encode_frame(node.absolute_frame())),
        content_offset: None,
        z_index: None,
        opacity: None,
        layout_role: node.attachment::<String>(DebugAttachmentKey::LAYOUT_ROLE),
        semantic_role: semantic_role(node),
        flags: node_flags(node),
        children: node.children().iter().map(summarize_summary).collect(),
        layout: None,
        style: None,
        source: source_location(node),
        owner_scope_id: None,
        recomposition: None,
    }
}

/// A page node: the wire summary's identity, geometry, roles, source and
/// children, with the summary's always-null detail slots left out.
fn page_node(node: &Node, children: Vec<TreePageNode>) -> TreePageNode {
    TreePageNode {
        id: SceneInspector::identifier(node),
        view_tag: node.view_tag(),
        debug_name: node.attachment::<String>(DebugAttachmentKey::DEBUG_NAME),
        frame: encode_frame(node.frame()),
        absolute_frame: encode_frame(node.absolute_frame()),
        layout_role: node.attachment::<String>(DebugAttachmentKey::LAYOUT_ROLE),
        semantic_role: semantic_role(node),
        source: source_location(node),
        children,
    }
}

fn semantic_role(node: &Node) -> Option<String> {
    node.attachment::<String>(DebugAttachmentKey::SEMANTIC_ROLE)
        .or_else(|| {
            node.accessibility()
                .map(|semantics| crate::inspection::inspector::role_name(semantics.role))
        })
}

fn node_flags(node: &Node) -> NodeFlags {
    NodeFlags {
        hit_testable: node.is_hit_testable(),
        focusable: node.is_focusable(),
        clips_to_bounds: node.clips_to_bounds(),
        has_background: node.background_color().is_some(),
        has_border: node.border_color().is_some() && node.border_width() > 0.0,
    }
}

fn source_location(node: &Node) -> Option<SourceLocation> {
    node.source_location().map(|location| SourceLocation {
        file_id: location.file_id,
        file_path: location.file_path,
        line: location.line,
        column: location.column,
    })
}

fn insets(value: LayoutInsets) -> InspectionInsets {
    InspectionInsets {
        top: value.top as f64,
        right: value.right as f64,
        bottom: value.bottom as f64,
        left: value.left as f64,
    }
}

pub(crate) fn encode_frame(frame: Rect) -> NodeFrame {
    NodeFrame {
        x: frame.x as f64,
        y: frame.y as f64,
        w: frame.width as f64,
        h: frame.height as f64,
    }
}

fn encode_invalidation(reason: cove_core::scene::DirtyReason) -> InvalidationRecord {
    InvalidationRecord {
        target: reason.target.0.to_string(),
        source: describe_source(&reason.source),
        phase: InvalidationPhase::as_str(reason.phase).to_string(),
        timestamp: reason.timestamp,
    }
}

fn describe_source(source: &InvalidationSource) -> String {
    match source {
        InvalidationSource::StateWrite { scope } => format!("stateWrite({scope})"),
        InvalidationSource::StyleSet { field } => format!("styleSet({field})"),
        InvalidationSource::LayoutChange => "layoutChange".into(),
        InvalidationSource::StructuralChange => "structuralChange".into(),
        InvalidationSource::FocusChange => "focusChange".into(),
        InvalidationSource::PlatformResize => "platformResize".into(),
        InvalidationSource::Unknown => "unknown".into(),
    }
}

impl std::fmt::Debug for SceneInspector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SceneInspector")
            .field("editor", &self.editor)
            .finish_non_exhaustive()
    }
}
