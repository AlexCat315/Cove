//! Point selection and content geometry on the runtime side.
//!
//! These pin the two facts a client depends on for Flutter-style select-at-point
//! and node screenshots: `NodeAt` is the platform's own hit test (no input is
//! dispatched and no selection is taken), and content geometry is either the
//! host's real window facts or an explicit `unavailable`, never a guess.

use cove_core::Rect;
use cove_core::scene::{Node, NodeTransform, NodeTree};
use cove_devtools::protocol::ContentGeometry;
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

fn tree() -> (Node, Node, Node, Node) {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    let left = Node::new();
    left.set_frame(Rect::new(0.0, 0.0, 200.0, 300.0));
    let right = Node::new();
    right.set_frame(Rect::new(200.0, 0.0, 200.0, 300.0));
    let inner = Node::new();
    inner.set_frame(Rect::new(10.0, 20.0, 40.0, 30.0));
    right.add_child(&inner);
    root.add_child(&left);
    root.add_child(&right);
    (root, left, right, inner)
}

fn probe() -> RuntimeProbe {
    probe_from(tree().0)
}

fn probe_from(root: Node) -> RuntimeProbe {
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(root));
    RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None)
}

fn node_id(node: &Node) -> String {
    node.id().0.to_string()
}

#[test]
fn node_at_hits_the_deepest_node_and_reports_the_root_first_path() {
    let (root, _, right, inner) = tree();
    let mut probe = probe_from(root.clone());
    let hit = probe
        .execute(ProbeCommand::NodeAt { x: 220.0, y: 40.0 })
        .unwrap();
    assert_eq!(hit["nodeID"], node_id(&inner));
    assert_eq!(
        hit["path"],
        serde_json::json!([node_id(&root), node_id(&right), node_id(&inner)])
    );
    assert_eq!(hit["localPoint"], serde_json::json!({"x": 10.0, "y": 20.0}));
    assert_eq!(
        hit["absoluteFrame"],
        serde_json::json!({"x": 210.0, "y": 20.0, "w": 40.0, "h": 30.0})
    );
    assert_eq!(probe.scene.editor.state().selected_id, None);
}

#[test]
fn node_at_reports_no_node_outside_the_tree() {
    let mut probe = probe();
    let hit = probe
        .execute(ProbeCommand::NodeAt { x: 500.0, y: 40.0 })
        .unwrap();
    assert_eq!(hit["nodeID"], Value::Null);
    assert_eq!(hit["path"], serde_json::json!([]));
    assert_eq!(hit["localPoint"], Value::Null);
}

#[test]
fn node_at_rejects_non_finite_points() {
    let mut probe = probe();
    assert_eq!(
        probe
            .execute(ProbeCommand::NodeAt {
                x: f64::NAN,
                y: 0.0,
            })
            .unwrap_err()
            .code,
        "invalid_request"
    );
}

#[test]
fn node_at_maps_through_a_scaled_node_like_platform_input() {
    let (root, _, right, inner) = tree();
    right.set_transform(NodeTransform::anchored(2.0, 0.0, 0.0));
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(root));
    let mut probe = RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None);
    // `right` draws at 2x with its origin at content x = 200, so its local
    // (20, 40) is content (240, 80) and the inner node at local (10, 20) is hit
    // there with the same local point a platform click would report.
    let hit = probe
        .execute(ProbeCommand::NodeAt { x: 240.0, y: 80.0 })
        .unwrap();
    assert_eq!(hit["nodeID"], node_id(&inner));
    assert_eq!(hit["localPoint"], serde_json::json!({"x": 10.0, "y": 20.0}));
}

#[test]
fn content_geometry_is_explicit_when_the_host_has_no_source() {
    let mut probe = probe();
    let capabilities = probe.capabilities();
    assert!(capabilities.contains(&"inspect_nodes".to_owned()));
    assert!(!capabilities.contains(&"content_geometry".to_owned()));
    let fault = probe.execute(ProbeCommand::ContentGeometry).unwrap_err();
    assert_eq!(fault.code, "unavailable");
}

#[test]
fn content_geometry_returns_the_host_values() {
    let mut probe = probe();
    probe.content_geometry = Some(Rc::new(|| {
        Ok(ContentGeometry {
            origin_x: 120.5,
            origin_y: 80.0,
            scale: 2.0,
            width: 800.0,
            height: 600.0,
        })
    }));
    assert!(
        probe
            .capabilities()
            .contains(&"content_geometry".to_owned())
    );
    let geometry = probe.execute(ProbeCommand::ContentGeometry).unwrap();
    assert_eq!(
        geometry,
        serde_json::json!({
            "originX": 120.5, "originY": 80.0, "scale": 2.0,
            "width": 800.0, "height": 600.0,
        })
    );
}

#[test]
fn invalid_host_geometry_is_refused() {
    let mut probe = probe();
    probe.content_geometry = Some(Rc::new(|| {
        Ok(ContentGeometry {
            origin_x: 0.0,
            origin_y: 0.0,
            scale: 0.0,
            width: 800.0,
            height: 600.0,
        })
    }));
    assert_eq!(
        probe
            .execute(ProbeCommand::ContentGeometry)
            .unwrap_err()
            .code,
        "unavailable"
    );
}
