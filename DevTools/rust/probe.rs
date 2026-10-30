//! Headless diagnostic host.
//!
//! Swift shipped `CoveUIDevToolsProbe` as an executable target (`main.swift`).
//! Rust keeps it as a library module plus a `[[bin]]`, so the harness can also be
//! driven from a test or an example without spawning a process. The logic is
//! identical: build a small scene, start the real server, and exercise it.
use crate::inspection::{Diagnostics, SceneInspector};
use crate::server::config::DevToolsConfig;
use crate::server::DevServer;
use cove_core::scene::{InvalidationLog, LayoutNode, Node, NodeTree};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Result of a probe run, so a caller can assert on it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeReport {
    pub bound_port: u16,
    pub endpoint: String,
    pub node_count: usize,
}

/// Build the probe scene: one 320x180 root tagged `TransportProbe`.
///
/// Layout is computed here rather than by a composer, because the point of the
/// probe is to exercise the server without a renderer.
pub fn build_scene() -> Rc<RefCell<NodeTree>> {
    let mut tree = NodeTree::new();
    let root = Node::new();
    root.set_view_tag(Some("TransportProbe".into()));
    root.set_attachment("CoveUI.debug.name", "headless.root".to_string());
    root.set_background_color(Some(cove_core::Color::WHITE));
    let layout = LayoutNode::new();
    layout.set_width(Some(320.0));
    layout.set_height(Some(180.0));
    layout.set_padding(8.0, cove_core::scene::Edge::All);
    root.set_layout_node(Some(&layout));
    layout.calculate_layout(Some(320.0), Some(180.0), cove_core::scene::Direction::Ltr);
    root.set_frame(layout.frame());
    let child = Node::new();
    child.set_attachment("CoveUI.debug.name", "headless.child".to_string());
    let child_layout = LayoutNode::new();
    child_layout.set_width(Some(120.0));
    child_layout.set_height(Some(40.0));
    child.set_layout_node(Some(&child_layout));
    root.add_child(&child);
    tree.set_root(Some(root));
    Rc::new(RefCell::new(tree))
}

/// Start the probe server on `port` (0 for an ephemeral one).
///
/// Returns the running server and its report. The caller owns the server's
/// lifetime and must call `stop`.
pub fn start(port: u16) -> std::io::Result<(DevServer, ProbeReport)> {
    let _ = cove_logging::LogSystem::bootstrap(None, Default::default(), None);
    let tree = build_scene();
    let mut server = DevServer::new(DevToolsConfig::new("127.0.0.1", port, "CoveUI transport probe", true));

    let inspector = Rc::new(RefCell::new(SceneInspector::new(tree.clone(), None, None)));
    let diagnostics = Rc::new(RefCell::new(Diagnostics::new(tree.clone(), None)));
    let snapshot_inspector = inspector.clone();
    let select_inspector = inspector.clone();
    let inspect_inspector = inspector.clone();
    let reset_inspector = inspector.clone();
    let clear_inspector = inspector.clone();
    server.host.scene = crate::server::config::DevToolsHostScene {
        snapshot: Some(Rc::new(move || snapshot_inspector.borrow().snapshot())),
        select: Some(Rc::new(move |id: &str| select_inspector.borrow().editor.select(Some(id)))),
        clear_selection: Some(Rc::new(move || clear_inspector.borrow().editor.select(None))),
        inspect: Some(Rc::new(move |request| inspect_inspector.borrow().editor.handle(request))),
        reset: Some(Rc::new(move || reset_inspector.borrow().editor.reset())),
        diagnostics: Some(Rc::new(move |request| diagnostics.borrow().handle_read_request(request))),
    };
    let timeline_tree = tree.clone();
    let capture_tree = tree.clone();
    server.host.observation = crate::server::config::DevToolsHostObservation {
        state: None,
        timeline: Some(Rc::new(move |after| {
            let timeline = timeline_tree.borrow().timeline.clone();
            let timeline = timeline.borrow();
            cove_devtools_protocol::TimelineSnapshot {
                events: timeline
                    .events_after(after)
                    .into_iter()
                    .map(|event| cove_devtools_protocol::TimelineEvent {
                        sequence: event.sequence,
                        phase: event.phase,
                        name: event.name,
                        scope_id: event.scope_id,
                        start_ms: event.start_ms,
                        duration_ms: event.duration_ms,
                        reasons: Vec::new(),
                    })
                    .collect(),
                dropped: timeline.dropped() as usize,
            }
        })),
        capture: Some(Rc::new(move |enabled| {
            capture_tree.borrow().timeline.borrow_mut().set_enabled(enabled);
        })),
    };
    server.advertised_capabilities = [
        "tree",
        "select",
        "log",
        "timing",
        "inspect",
        "style",
        "source",
        "recomposition",
        "timeline",
        "diagnostics",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    let state = std::collections::BTreeMap::new();
    server.host.checkpoint = crate::server::config::DevToolsHostCheckpoint {
        capture: Some(Rc::new({
            let state = state.clone();
            move || state.clone()
        })),
        restore: Some(Rc::new(|_next| true)),
    };
    server.start()?;
    let bound_port = server.bound_port().unwrap_or(port);
    let node_count = tree.borrow_mut().node_count();
    let report = ProbeReport {
        bound_port,
        endpoint: format!("ws://127.0.0.1:{bound_port}/"),
        node_count,
    };
    Ok((server, report))
}

/// Exercise the scene and inspector without a network client.
pub fn probe_inspection() -> TreeProbe {
    let tree = build_scene();
    let inspector = SceneInspector::new(tree.clone(), None, None);
    let snapshot = inspector.snapshot();
    let root_id = snapshot.root.as_ref().map(|root| root.id.clone());
    let found = root_id.as_deref().and_then(|id| inspector.find(id));
    let diagnostics = Diagnostics::new(tree.clone(), None);
    let overview = diagnostics.overview(50);
    TreeProbe {
        has_root: snapshot.root.is_some(),
        child_count: snapshot.root.as_ref().map(|root| root.children.len()).unwrap_or(0),
        found_root: found.is_some(),
        node_count: overview.node_count,
        layout_issue_total: overview.layout_issues.total,
    }
}

/// Facts the probe verified, so a caller can assert instead of printing.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeProbe {
    pub has_root: bool,
    pub child_count: usize,
    pub found_root: bool,
    pub node_count: usize,
    pub layout_issue_total: usize,
}

/// Attach an invalidation log, mirroring the probe's optional wiring.
pub fn with_invalidation_log(tree: Rc<RefCell<NodeTree>>, log: Arc<InvalidationLog>) -> SceneInspector {
    SceneInspector::new(tree, Some(log), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_scene_has_a_root_and_a_child() {
        let report = probe_inspection();
        assert!(report.has_root);
        assert_eq!(report.child_count, 1);
        assert!(report.found_root, "the snapshot id resolves back to a live node");
        assert_eq!(report.node_count, 2);
    }

    #[test]
    fn probe_server_binds_and_reports_its_port() {
        let (mut server, report) = start(0).expect("server starts");
        assert!(report.bound_port != 0, "ephemeral port was assigned");
        assert_eq!(report.node_count, 2);
        assert!(report.endpoint.starts_with("ws://127.0.0.1:"));
        server.stop();
    }
}
