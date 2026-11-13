//! Bounded tree pages: the agent-facing read for a gallery-sized tree.
//!
//! A live gallery's `inspect_ui_tree` summary is over a megabyte; the agent
//! toolbox rejects a result above 512 KiB. `TreePage` returns a real, bounded
//! subtree instead. These pin the honesty properties: every node is a live
//! node, depth and limit cut rather than synthesise, a cut subtree is reported
//! through `truncated`, and even the widest allowed page stays inside the
//! agent's result budget.

use cove_core::Rect;
use cove_core::scene::{ComponentSourceLocation, Node, NodeTree};
use cove_devtools::agent::MAX_TOOL_RESULT_BYTES;
use cove_devtools::inspection::{DebugAttachmentKey, SceneInspector, TreePageError};
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::protocol::{
    MAX_TREE_PAGE_BYTES, MAX_TREE_PAGE_DEPTH, MAX_TREE_PAGE_NODES, TreePage,
};
use cove_devtools::runtime_probe::RuntimeProbe;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

fn node(label: &str, x: f32, y: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, 20.0, 10.0));
    node.set_view_tag(Some("Row".to_owned()));
    node.set_attachment(DebugAttachmentKey::DEBUG_NAME, label.to_owned());
    node.set_attachment(DebugAttachmentKey::LAYOUT_ROLE, "listItem".to_owned());
    node.set_attachment(DebugAttachmentKey::SEMANTIC_ROLE, "button".to_owned());
    node.set_source_location(Some(ComponentSourceLocation::new(
        format!("{label}-source"),
        "examples/gallery/src/gallery_content/stories/controls_story.rs",
        7,
    )));
    node
}

/// root(10, 20) → left, right; each child has one grandchild.
fn nested_tree() -> (Node, Node, Node, Node, Node) {
    let root = node("root", 10.0, 20.0);
    let left = node("left", 0.0, 0.0);
    let right = node("right", 30.0, 0.0);
    let left_inner = node("left-inner", 2.0, 2.0);
    let right_inner = node("right-inner", 4.0, 4.0);
    left.add_child(&left_inner);
    right.add_child(&right_inner);
    root.add_child(&left);
    root.add_child(&right);
    (root, left, right, left_inner, right_inner)
}

fn probe(root: Node) -> RuntimeProbe {
    let mut tree = NodeTree::new();
    tree.set_root(Some(root));
    RuntimeProbe::new(Rc::new(RefCell::new(tree)), None, None, None)
}

fn id(node: &Node) -> String {
    node.id().0.to_string()
}

fn page(probe: &mut RuntimeProbe, node_id: Option<&str>, depth: usize, limit: usize) -> TreePage {
    let value = probe
        .execute(ProbeCommand::TreePage {
            node_id: node_id.map(str::to_owned),
            depth,
            limit,
        })
        .expect("the page builds");
    serde_json::from_value(value).expect("the page decodes")
}

fn ids(page: &TreePage) -> Vec<String> {
    page.root
        .as_ref()
        .map(|root| {
            let mut ids = Vec::new();
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                ids.push(node.id.clone());
                stack.extend(node.children.iter().rev());
            }
            ids
        })
        .unwrap_or_default()
}

#[test]
fn depth_counts_real_node_levels_including_the_page_root() {
    let (root, left, right, left_inner, right_inner) = nested_tree();
    let mut probe = probe(root.clone());

    let top = page(&mut probe, None, 1, 100);
    assert_eq!(ids(&top), vec![id(&root)]);
    assert_eq!(top.node_count, 1);
    assert!(
        top.truncated,
        "children below the depth cut must be reported"
    );

    let children = page(&mut probe, None, 2, 100);
    assert_eq!(ids(&children), vec![id(&root), id(&left), id(&right)]);
    assert_eq!(children.node_count, 3);
    assert!(children.truncated, "grandchildren are below depth 2");

    let full = page(&mut probe, None, 3, 100);
    assert_eq!(
        ids(&full),
        vec![
            id(&root),
            id(&left),
            id(&left_inner),
            id(&right),
            id(&right_inner)
        ]
    );
    assert_eq!(full.node_count, 5);
    assert!(!full.truncated, "depth 3 covers the whole tree");
}

#[test]
fn a_leaf_page_is_not_reported_as_truncated() {
    let (root, _, _, left_inner, _) = nested_tree();
    let mut probe = probe(root);
    let leaf = page(&mut probe, Some(&id(&left_inner)), 4, 10);
    assert_eq!(ids(&leaf), vec![id(&left_inner)]);
    assert_eq!(leaf.node_count, 1);
    assert!(!leaf.truncated);
}

#[test]
fn page_nodes_are_live_nodes_with_real_geometry() {
    let (root, _, right, _, right_inner) = nested_tree();
    let mut probe = probe(root.clone());
    let value = probe
        .execute(ProbeCommand::TreePage {
            node_id: Some(id(&right)),
            depth: 2,
            limit: 10,
        })
        .unwrap();
    let page: TreePage = serde_json::from_value(value.clone()).unwrap();
    let page_root = page.root.as_ref().unwrap();
    assert_eq!(page_root.id, id(&right));
    assert_eq!(page_root.view_tag.as_deref(), Some("Row"));
    assert_eq!(page_root.debug_name.as_deref(), Some("right"));
    assert_eq!(page_root.layout_role.as_deref(), Some("listItem"));
    assert_eq!(page_root.semantic_role.as_deref(), Some("button"));
    assert_eq!(page_root.frame.x, 30.0);
    // The absolute frame is the real accumulated one, as in the wire summary.
    assert_eq!(page_root.absolute_frame.x, 40.0);
    assert_eq!(page_root.absolute_frame.y, 20.0);
    assert_eq!(page_root.children.len(), 1);
    assert_eq!(page_root.children[0].id, id(&right_inner));
    assert_eq!(page_root.children[0].absolute_frame.x, 44.0);
    assert_eq!(page_root.children[0].absolute_frame.y, 24.0);
    assert_eq!(
        value["root"]["source"]["filePath"],
        Value::from("examples/gallery/src/gallery_content/stories/controls_story.rs")
    );
    // The compact page drops the summary's absent detail slots.
    assert!(value["root"].get("style").is_none());
    assert!(value["root"].get("flags").is_none());
    assert!(value["root"].get("contentOffset").is_none());
}

#[test]
fn the_node_limit_cuts_nodes_in_document_order() {
    let root = node("root", 0.0, 0.0);
    let children: Vec<Node> = (0..5)
        .map(|index| {
            let child = node(&format!("child-{index}"), index as f32 * 30.0, 0.0);
            root.add_child(&child);
            child
        })
        .collect();
    let mut probe = probe(root.clone());
    let limited = page(&mut probe, None, 3, 3);
    assert_eq!(
        ids(&limited),
        vec![id(&root), id(&children[0]), id(&children[1])]
    );
    assert_eq!(limited.node_count, 3);
    assert_eq!(limited.limit, 3);
    assert!(limited.truncated);
    // No placeholder node stands in for the cut children.
    assert_eq!(
        limited.root.as_ref().unwrap().children.len(),
        2,
        "the page contains only the two real children that fit"
    );
}

#[test]
fn a_stale_or_malformed_root_is_an_explicit_error() {
    let (root, _, _, left_inner, _) = nested_tree();
    let mut probe = probe(root);
    let fault = probe
        .execute(ProbeCommand::TreePage {
            node_id: Some("not-a-live-node".into()),
            depth: 2,
            limit: 10,
        })
        .unwrap_err();
    assert_eq!(fault.code, "stale_node");

    let inner = page(&mut probe, Some(&id(&left_inner)), 2, 10);
    assert_eq!(ids(&inner), vec![id(&left_inner)]);

    let empty = probe
        .execute(ProbeCommand::TreePage {
            node_id: Some(String::new()),
            depth: 2,
            limit: 10,
        })
        .unwrap_err();
    assert_eq!(empty.code, "invalid_request");
    let long = probe
        .execute(ProbeCommand::TreePage {
            node_id: Some("x".repeat(257)),
            depth: 2,
            limit: 10,
        })
        .unwrap_err();
    assert_eq!(long.code, "invalid_request");
}

#[test]
fn out_of_range_bounds_are_rejected_before_the_tree_is_read() {
    let (root, _, _, _, _) = nested_tree();
    let mut probe = probe(root);
    for (depth, limit) in [
        (0, 10),
        (MAX_TREE_PAGE_DEPTH + 1, 10),
        (2, 0),
        (2, MAX_TREE_PAGE_NODES + 1),
    ] {
        let fault = probe
            .execute(ProbeCommand::TreePage {
                node_id: None,
                depth,
                limit,
            })
            .unwrap_err();
        assert_eq!(
            fault.code, "invalid_request",
            "depth {depth} limit {limit} must be refused"
        );
    }
    assert_eq!(TreePageError::Depth.code(), "invalid_request");
    assert_eq!(TreePageError::Stale("x".into()).code(), "stale_node");
}

#[test]
fn an_empty_scene_pages_to_no_nodes_instead_of_a_guess() {
    let mut tree = NodeTree::new();
    tree.set_root(None);
    let mut probe = RuntimeProbe::new(Rc::new(RefCell::new(tree)), None, None, None);
    let empty: TreePage = serde_json::from_value(
        probe
            .execute(ProbeCommand::TreePage {
                node_id: None,
                depth: 2,
                limit: 10,
            })
            .unwrap(),
    )
    .unwrap();
    assert!(empty.root.is_none());
    assert_eq!(empty.node_count, 0);
    assert!(!empty.truncated);

    let fault = probe
        .execute(ProbeCommand::TreePage {
            node_id: Some("anything".into()),
            depth: 2,
            limit: 10,
        })
        .unwrap_err();
    assert_eq!(fault.code, "stale_node");
}

/// A gallery-shaped tree: one root with 2200 wide rows.
fn gallery() -> Node {
    let root = node("gallery-root", 0.0, 0.0);
    root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    for index in 0..2200 {
        let label = format!("GalleryRow-{index:04}-{index}");
        let child = node(
            &label,
            (index % 40) as f32 * 20.0,
            (index / 40) as f32 * 20.0,
        );
        root.add_child(&child);
    }
    root
}

#[test]
fn the_widest_allowed_page_stays_inside_the_agent_result_budget() {
    let mut probe = probe(gallery());
    let value = probe
        .execute(ProbeCommand::TreePage {
            node_id: None,
            depth: MAX_TREE_PAGE_DEPTH,
            limit: MAX_TREE_PAGE_NODES,
        })
        .unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    println!(
        "page bytes = {} for {} nodes",
        bytes.len(),
        value["nodeCount"]
    );
    assert!(
        bytes.len() <= MAX_TREE_PAGE_BYTES,
        "page is {} bytes, over the {MAX_TREE_PAGE_BYTES}-byte wire ceiling",
        bytes.len()
    );
    assert!(
        bytes.len() < MAX_TOOL_RESULT_BYTES,
        "page is {} bytes, over the {MAX_TOOL_RESULT_BYTES}-byte agent budget",
        bytes.len()
    );
    let page: TreePage = serde_json::from_value(value).unwrap();
    assert!(page.node_count <= MAX_TREE_PAGE_NODES);
    assert!(
        page.truncated,
        "the 2200-row gallery cannot fit the allowed page whole"
    );
    // The page is real: every named node exists in the live tree, and the
    // cut is a prefix of the root's children rather than a resampling.
    assert_eq!(
        page.root.as_ref().unwrap().children.len() + 1,
        page.node_count
    );
    let first = &page.root.as_ref().unwrap().children[0];
    assert_eq!(first.debug_name.as_deref(), Some("GalleryRow-0000-0"));
}

#[test]
fn a_shallow_page_of_a_gallery_is_small_and_complete() {
    let mut probe = probe(gallery());
    let value = probe
        .execute(ProbeCommand::TreePage {
            node_id: None,
            depth: 2,
            limit: 200,
        })
        .unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    println!("depth 2 / limit 200 page bytes = {}", bytes.len());
    assert!(bytes.len() < MAX_TOOL_RESULT_BYTES);
    assert!(bytes.len() < 128 * 1024);
    let page: TreePage = serde_json::from_value(value).unwrap();
    assert_eq!(page.node_count, 200);
    assert_eq!(page.root.as_ref().unwrap().children.len(), 199);
    assert!(page.truncated);
}

#[test]
fn a_page_rooted_at_a_real_node_reports_its_children_as_cut() {
    let root = gallery();
    let child = root.children()[7].clone();
    let child_id = id(&child);
    let mut probe = probe(root);
    let page = page(&mut probe, Some(&child_id), 16, MAX_TREE_PAGE_NODES);
    assert_eq!(page.root.as_ref().unwrap().id, child_id);
    assert_eq!(page.node_count, 1);
    assert!(!page.truncated, "a leaf has nothing to cut");

    let inspector = SceneInspector::new(probe.scene.tree().clone(), None, None);
    assert_eq!(
        inspector
            .tree_page(Some("missing"), 2, 10)
            .unwrap_err()
            .to_string(),
        "No live node has element ID missing."
    );
}

#[test]
fn a_page_never_carries_null_detail_slots() {
    let (root, left, _, _, _) = nested_tree();
    let mut probe = probe(root);
    let value = probe
        .execute(ProbeCommand::TreePage {
            node_id: Some(id(&left)),
            depth: 1,
            limit: 1,
        })
        .unwrap();
    let object = value["root"].as_object().unwrap();
    for key in object.keys() {
        assert!(
            !value["root"][key].is_null(),
            "the compact page must omit {key}, not send null"
        );
    }
    assert!(
        !object.contains_key("children"),
        "a cut leaf omits children"
    );
    assert_eq!(value["depth"], 1);
    assert_eq!(value["limit"], 1);
    assert_eq!(value["nodeCount"], 1);
}
