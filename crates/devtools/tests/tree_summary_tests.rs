//! The wire `tree` command must fit one protocol message on a real gallery.
//!
//! `get_diagnostics` and `query_ui_nodes` still read the full snapshot; this
//! suite pins the bounded summary those clients do not use, so point selection
//! and node screenshots keep their identity and geometry without the detailed
//! payload that overflows `MAX_MESSAGE`.

use cove_core::Rect;
use cove_core::scene::{
    ComponentRecompositionMetrics, ComponentSourceLocation, LayoutNode, Node, NodeTree,
    RecompositionReason,
};
use cove_devtools::inspection::{DebugAttachmentKey, SceneInspector};
use cove_devtools_protocol::control::MAX_MESSAGE;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

const NODES: usize = 2200;
const COLUMNS: usize = 40;
const CELL: f32 = 20.0;

fn gallery_tree() -> (Node, Node, Vec<LayoutNode>) {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    root.set_source_location(Some(ComponentSourceLocation::new(
        "gallery-root",
        "examples/gallery/src/gallery_content/gallery_view.rs",
        1,
    )));
    let mut target = None;
    let mut layouts = Vec::with_capacity(NODES);
    for index in 0..NODES {
        let node = Node::new();
        let x = (index % COLUMNS) as f32 * CELL;
        let y = (index / COLUMNS) as f32 * CELL;
        node.set_frame(Rect::new(x, y, CELL - 2.0, CELL - 2.0));
        node.set_view_tag(Some("Row".to_owned()));
        node.set_attachment(
            DebugAttachmentKey::DEBUG_NAME,
            format!("GalleryRow-{index:04}-{index}"),
        );
        node.set_attachment(DebugAttachmentKey::LAYOUT_ROLE, "listItem".to_owned());
        node.set_attachment(DebugAttachmentKey::SEMANTIC_ROLE, "button".to_owned());
        node.set_source_location(Some(ComponentSourceLocation::new(
            format!("gallery-row-{index}"),
            "examples/gallery/src/gallery_content/stories/controls_story.rs",
            100 + index as u64,
        )));
        let layout = LayoutNode::new();
        node.set_layout_node(Some(&layout));
        layouts.push(layout);
        node.set_recomposition_metrics(Some(ComponentRecompositionMetrics {
            count: 7,
            initial_ms: 0.2,
            last_ms: 0.4,
            total_ms: 2.1,
            max_ms: 0.6,
            last_reasons: vec![RecompositionReason::new("gallery state write")],
        }));
        if index == NODES - 3 {
            target = Some(node.clone());
        }
        root.add_child(&node);
    }
    (root, target.expect("the target node was built"), layouts)
}

fn inspector(root: Node) -> SceneInspector {
    let mut tree = NodeTree::new();
    tree.set_root(Some(root));
    SceneInspector::new(Rc::new(RefCell::new(tree)), None, None)
}

#[test]
fn a_gallery_sized_summary_fits_one_protocol_message() {
    let (root, target, _layouts) = gallery_tree();
    let target_id = target.id().0.to_string();
    let inspector = inspector(root);

    let summary = inspector.summary_snapshot();
    let bytes = serde_json::to_vec(&summary).expect("summary serialises");
    println!("summary tree bytes = {}", bytes.len());
    assert!(
        bytes.len() < MAX_MESSAGE,
        "a {NODES}-node summary is {} bytes and must stay under MAX_MESSAGE ({MAX_MESSAGE})",
        bytes.len()
    );

    // The transport must write compact JSON: ureq's `send_json` pretty-prints,
    // which inflates this payload past the message limit even after the summary
    // dropped detailed fields.
    let pretty = serde_json::to_vec_pretty(&summary).expect("summary pretty serialises");
    println!("summary pretty bytes = {}", pretty.len());
    assert!(
        pretty.len() > MAX_MESSAGE,
        "pretty-printing must stay off the wire: {} bytes",
        pretty.len()
    );

    let full = serde_json::to_vec(&inspector.snapshot()).expect("full snapshot serialises");
    println!("full snapshot bytes = {}", full.len());
    assert!(
        full.len() > MAX_MESSAGE,
        "the detailed snapshot overflows one message at {} bytes",
        full.len()
    );
    assert!(
        bytes.len() < full.len(),
        "the summary drops detailed fields: summary {}, full {}",
        bytes.len(),
        full.len()
    );

    let payload = serde_json::to_value(&summary).expect("summary value");
    let first = &payload["root"]["children"][0];
    for dropped in [
        "layout",
        "style",
        "recomposition",
        "contentOffset",
        "zIndex",
        "opacity",
        "ownerScopeID",
    ] {
        assert!(
            first[dropped].is_null(),
            "the wire summary must not carry {dropped}"
        );
    }
    for kept in [
        "id",
        "frame",
        "absoluteFrame",
        "layoutRole",
        "semanticRole",
        "source",
    ] {
        assert!(
            first.get(kept).is_some_and(|value| !value.is_null()),
            "the wire summary keeps {kept}"
        );
    }

    // `capture_ui_node` walks this same payload type and reads the absolute
    // frame; identity and geometry survive the summary.
    let typed: cove_devtools_protocol::NodeSummary =
        serde_json::from_value(payload["root"].clone()).expect("summary decodes");
    let found = typed
        .find(&target_id)
        .expect("the deep node is still findable");
    let frame = found
        .absolute_frame
        .as_ref()
        .expect("the node keeps its absolute frame");
    assert_eq!(frame.w, f64::from(CELL - 2.0));
    assert_eq!(frame.h, f64::from(CELL - 2.0));
    assert!(found.layout.is_none() && found.style.is_none());

    assert_eq!(
        payload["inputInventory"]["nodeCount"],
        Value::from(NODES + 1)
    );
    assert!(payload.get("renderInventory").is_some());
    assert!(payload.get("inspection").is_some());
}

#[test]
fn the_full_snapshot_still_carries_layout_style_and_recomposition() {
    let (root, _, _layouts) = gallery_tree();
    let payload = serde_json::to_value(inspector(root).snapshot()).expect("snapshot value");
    let first = &payload["root"]["children"][0];
    assert!(first["layout"].is_object());
    assert!(first["style"].is_object());
    assert!(first["recomposition"].is_object());
    assert!(!first["contentOffset"].is_null());
    assert_eq!(first["ownerScopeID"], Value::from(first["id"].clone()));
}
