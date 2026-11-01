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
    RenderTree,
};
use cove_devtools_protocol::{
    InputInventory, InspectionInsets, InvalidationRecord, NodeFlags, NodeFrame, NodeLayoutInfo,
    NodePoint, NodeStyleInfo, NodeSummary, Recomposition, RecompositionReason, RenderInventory,
    SourceLocation, TreeSnapshot,
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

/// Search for a node by its stable identity. Both the `0x…` content hash from
/// `identifier` and the raw element id are accepted, so a client may use either.
pub(crate) fn find_in(node: &Node, id: &str) -> Option<Node> {
    if SceneInspector::identifier(node) == id || node.id().0.to_string() == id {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find_in(&child, id) {
            return Some(found);
        }
    }
    None
}

/// Builds `NodeSummary` snapshots from a `NodeTree`.
///
/// Stable id strategy: content hashes derived from `ElementId`, valid for the
/// lifetime of the host process. Selection from the client is best-effort —
/// stale ids are ignored by the host.
pub struct SceneInspector {
    tree: Rc<RefCell<NodeTree>>,
    pub editor: SceneEditor,
    /// Optional log to attach to snapshots, so clients can show recent
    /// dirty-propagation events alongside the tree.
    pub invalidation_log: Option<Arc<InvalidationLog>>,
    /// Optional render-side mirror, surfaced as `renderInventory`.
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
            root: root.map(|root| summarize(&root, None)),
            invalidations,
            render_inventory,
            input_inventory,
            inspection: Some(inspection),
        }
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

    /// Stable identity for a node, matching Swift's `ObjectIdentifier` hash
    /// formatting so existing clients keep working.
    pub fn identifier(node: &Node) -> String {
        format!("0x{:x}", node.id().0)
    }
}

fn summarize(node: &Node, owner_scope_id: Option<&str>) -> NodeSummary {
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
        .map(|child| summarize(child, owner.as_deref()))
        .collect();
    let layout = node.layout_node().map(|layout| {
        let padding = layout.resolved_padding();
        let border = layout.resolved_border();
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
        }
    });
    let overrides: Vec<String> = [
        (
            "padding",
            node.layout_node()
                .is_some_and(|layout| layout.debug_padding().is_some()),
        ),
        ("backgroundColor", node.debug_background_color().is_some()),
        ("foregroundColor", node.debug_foreground_color().is_some()),
    ]
    .into_iter()
    .filter(|&(name, present)| present).map(|(name, present)| name.to_string())
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
        semantic_role: node
            .attachment::<String>(DebugAttachmentKey::SEMANTIC_ROLE)
            .or_else(|| {
                node.accessibility()
                    .map(|semantics| format!("{:?}", semantics.role))
            }),
        flags: NodeFlags {
            hit_testable: node.is_hit_testable(),
            focusable: node.is_focusable(),
            clips_to_bounds: node.clips_to_bounds(),
            has_background: node.background_color().is_some(),
            has_border: node.border_color().is_some() && node.border_width() > 0.0,
        },
        children,
        element_id: Some(node.id().0.to_string()),
        layout,
        style: Some(NodeStyleInfo {
            background_color: SceneEditor::hex(node.background_color()),
            foreground_color: SceneEditor::hex(node.inherited_foreground_color()),
            overrides,
        }),
        source: node.source_location().map(|location| SourceLocation {
            file_id: location.file_id,
            file_path: location.file_path,
            line: location.line,
            column: location.column,
        }),
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

fn insets(value: LayoutInsets) -> InspectionInsets {
    InspectionInsets {
        top: value.top as f64,
        right: value.right as f64,
        bottom: value.bottom as f64,
        left: value.left as f64,
    }
}

fn encode_frame(frame: Rect) -> NodeFrame {
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
