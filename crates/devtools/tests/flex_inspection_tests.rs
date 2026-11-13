//! Editable flex inspection: temporary `flexGrow`, `flexShrink`,
//! `justifyContent` and `alignItems` overrides on the live layout tree.
//!
//! These pin the facts a write-capable Flex panel depends on: the values reach
//! `LayoutNode` and change real layout, each field's authored value is captured
//! before its first override so a partial update cannot lose an earlier
//! restoration, and clear/undo/redo/reset/prune put the originals back. The
//! engine is the real one: every geometry assertion comes from a `LayoutPass`.

use cove_core::scene::{Align, Justify, LayoutNode, LayoutPass, Node, NodeTree};
use cove_devtools::protocol::control::ProbeCommand;
use cove_devtools::runtime_probe::RuntimeProbe;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

const WIDTH: f32 = 300.0;
const HEIGHT: f32 = 100.0;

struct Flex {
    probe: RuntimeProbe,
    root: Node,
    child: Node,
    root_layout: LayoutNode,
    child_layout: LayoutNode,
}

impl Flex {
    /// A column root with one fixed 50x20 child.
    fn new() -> Self {
        let root = Node::new();
        let child = Node::new();
        let root_layout = LayoutNode::new();
        root_layout.set_width(Some(WIDTH));
        root_layout.set_height(Some(HEIGHT));
        let child_layout = LayoutNode::new();
        child_layout.set_width(Some(50.0));
        child_layout.set_height(Some(20.0));
        root_layout.add_child(&child_layout);
        root.set_layout_node(Some(&root_layout));
        child.set_layout_node(Some(&child_layout));
        root.add_child(&child);
        let mut nodes = NodeTree::new();
        nodes.set_root(Some(root.clone()));
        Self {
            probe: RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None),
            root,
            child,
            root_layout,
            child_layout,
        }
    }

    fn layout(&self) {
        LayoutPass::run(&self.root_layout, &self.root, Some(WIDTH), Some(HEIGHT));
    }

    fn execute(&mut self, command: &str, arguments: Value) -> Result<Value, String> {
        self.probe
            .execute(ProbeCommand::Edit {
                name: command.to_owned(),
                arguments,
            })
            .map_err(|fault| fault.code)
    }

    fn set(&mut self, id: &str, properties: Value) -> Result<Value, String> {
        self.execute(
            "inspect.style.set",
            json!({"id": id, "properties": properties}),
        )
    }

    fn clear(&mut self, id: &str) -> Result<Value, String> {
        self.execute("inspect.style.clear", json!({"id": id}))
    }

    fn command(&mut self, command: &str) -> Value {
        self.execute(command, json!({}))
            .expect("the command succeeds")
    }

    fn root_id(&self) -> String {
        self.root.id().0.to_string()
    }

    fn child_id(&self) -> String {
        self.child.id().0.to_string()
    }

    fn overrides_of(&self, node: &Node) -> Vec<String> {
        self.probe
            .scene
            .editor
            .override_properties(&node.id().0.to_string())
            .into_iter()
            .map(str::to_string)
            .collect()
    }
}

#[test]
fn flex_grow_reaches_the_layout_node_and_redistributes_real_space() {
    let mut flex = Flex::new();
    let child = flex.child_id();
    flex.layout();
    assert_eq!(flex.child.frame().height, 20.0, "the child starts at 20pt");

    let state = flex.set(&child, json!({"flexGrow": 1})).unwrap();
    assert_eq!(state["overrideCount"], 1);
    assert_eq!(flex.child_layout.flex_grow(), 1.0);
    assert_eq!(
        flex.overrides_of(&flex.child),
        vec!["flexGrow".to_owned()],
        "the snapshot names the override"
    );
    assert!(
        flex.child.is_dirty(),
        "a flex edit marks the node's layout dirty"
    );
    assert!(
        flex.child.render_dirty(),
        "a flex edit also invalidates the render"
    );
    assert!(
        flex.root_layout.subtree_is_dirty(),
        "the layout tree is dirty, so the host relayouts on the next frame"
    );

    flex.layout();
    assert_eq!(
        flex.child.frame(),
        cove_core::Rect::new(0.0, 0.0, 50.0, HEIGHT),
        "the growing child takes the free column space"
    );

    flex.clear(&child).unwrap();
    assert_eq!(
        flex.child_layout.flex_grow(),
        0.0,
        "clear restores the original"
    );
    assert!(flex.overrides_of(&flex.child).is_empty());
    flex.layout();
    assert_eq!(flex.child.frame().height, 20.0);
}

#[test]
fn justify_content_and_align_items_move_real_child_frames() {
    let mut flex = Flex::new();
    let root = flex.root_id();
    flex.layout();
    assert_eq!(
        flex.child.frame(),
        cove_core::Rect::new(0.0, 0.0, 50.0, 20.0)
    );

    flex.set(
        &root,
        json!({"justifyContent": "FlexEnd", "alignItems": "FlexEnd"}),
    )
    .unwrap();
    assert_eq!(flex.root_layout.justify_content(), Justify::FlexEnd);
    assert_eq!(flex.root_layout.align_items(), Align::FlexEnd);
    flex.layout();
    assert_eq!(
        flex.child.frame(),
        cove_core::Rect::new(250.0, 80.0, 50.0, 20.0),
        "justify moves the child to the column end and align to the row end"
    );

    flex.clear(&root).unwrap();
    assert_eq!(flex.root_layout.justify_content(), Justify::FlexStart);
    assert_eq!(flex.root_layout.align_items(), Align::Stretch);
    flex.layout();
    assert_eq!(
        flex.child.frame(),
        cove_core::Rect::new(0.0, 0.0, 50.0, 20.0)
    );
}

#[test]
fn partial_updates_keep_each_earlier_field_original() {
    let mut flex = Flex::new();
    let child = flex.child_id();
    flex.layout();

    flex.set(&child, json!({"flexGrow": 1})).unwrap();
    // A second call sets only shrink; grow's original must survive.
    flex.set(&child, json!({"flexShrink": 2})).unwrap();
    flex.set(&child, json!({"justifyContent": "Center"}))
        .unwrap();
    assert_eq!(flex.child_layout.flex_grow(), 1.0);
    assert_eq!(flex.child_layout.flex_shrink(), 2.0);
    assert_eq!(flex.child_layout.justify_content(), Justify::Center);
    assert_eq!(
        flex.overrides_of(&flex.child),
        vec![
            "flexGrow".to_owned(),
            "flexShrink".to_owned(),
            "justifyContent".to_owned()
        ]
    );

    // Clearing one property restores only that field.
    flex.set(&child, json!({"flexGrow": null})).unwrap();
    assert_eq!(flex.child_layout.flex_grow(), 0.0);
    assert_eq!(flex.child_layout.flex_shrink(), 2.0);
    assert_eq!(flex.child_layout.justify_content(), Justify::Center);
    assert_eq!(
        flex.overrides_of(&flex.child),
        vec!["flexShrink".to_owned(), "justifyContent".to_owned()]
    );

    // Clearing the rest restores their captured originals.
    flex.set(&child, json!({"flexShrink": null, "justifyContent": null}))
        .unwrap();
    assert_eq!(flex.child_layout.flex_shrink(), 0.0);
    assert_eq!(flex.child_layout.justify_content(), Justify::FlexStart);
    assert!(flex.overrides_of(&flex.child).is_empty());
    flex.layout();
    assert_eq!(
        flex.child.frame(),
        cove_core::Rect::new(0.0, 0.0, 50.0, 20.0),
        "restoring the originals restores the authored geometry"
    );
}

#[test]
fn undo_redo_clear_all_and_reset_restore_originals() {
    let mut flex = Flex::new();
    let child = flex.child_id();
    let root = flex.root_id();
    flex.layout();
    flex.set(&child, json!({"flexGrow": 1})).unwrap();
    flex.set(&root, json!({"alignItems": "Center"})).unwrap();

    // Undo the align edit only: the child's grow override stays.
    let state = flex.command("inspect.style.undo");
    assert_eq!(state["canRedo"], true);
    assert_eq!(flex.root_layout.align_items(), Align::Stretch);
    assert_eq!(flex.child_layout.flex_grow(), 1.0);

    let state = flex.command("inspect.style.redo");
    assert_eq!(state["canUndo"], true);
    assert_eq!(flex.root_layout.align_items(), Align::Center);

    flex.command("inspect.style.undo");
    flex.command("inspect.style.undo");
    assert_eq!(flex.root_layout.align_items(), Align::Stretch);
    assert_eq!(flex.child_layout.flex_grow(), 0.0);
    flex.layout();
    assert_eq!(flex.child.frame().height, 20.0);
    let state = flex.command("inspect.style.redo");
    assert_eq!(state["canRedo"], true);
    assert_eq!(
        flex.child_layout.flex_grow(),
        1.0,
        "redo reapplies the edit"
    );
    flex.layout();
    assert_eq!(flex.child.frame().height, HEIGHT);
    flex.command("inspect.style.undo");

    flex.set(&child, json!({"flexGrow": 3})).unwrap();
    let state = flex.command("inspect.style.clearAll");
    assert_eq!(state["overrideCount"], 0);
    assert_eq!(flex.child_layout.flex_grow(), 0.0);

    flex.set(&child, json!({"flexGrow": 2})).unwrap();
    flex.set(&root, json!({"justifyContent": "SpaceEvenly"}))
        .unwrap();
    flex.probe.scene.editor.reset();
    assert_eq!(flex.child_layout.flex_grow(), 0.0);
    assert_eq!(flex.root_layout.justify_content(), Justify::FlexStart);
    assert_eq!(flex.probe.scene.editor.state().override_count, 0);
}

#[test]
fn every_layout_alignment_name_is_accepted_and_applied() {
    let mut flex = Flex::new();
    let root = flex.root_id();
    for name in cove_devtools_protocol::JUSTIFY_CONTENT {
        flex.set(&root, json!({"justifyContent": name}))
            .unwrap_or_else(|code| panic!("{name} rejected with {code}"));
        assert_eq!(format!("{:?}", flex.root_layout.justify_content()), name);
    }
    for name in cove_devtools_protocol::ALIGN_ITEMS {
        flex.set(&root, json!({"alignItems": name}))
            .unwrap_or_else(|code| panic!("{name} rejected with {code}"));
        assert_eq!(format!("{:?}", flex.root_layout.align_items()), name);
    }
}

#[test]
fn invalid_flex_values_and_names_are_refused_without_changing_the_node() {
    let mut flex = Flex::new();
    let child = flex.child_id();
    flex.layout();

    for properties in [
        json!({"flexGrow": -1}),
        json!({"flexGrow": 1000.5}),
        json!({"flexGrow": 2000}),
        json!({"flexShrink": "1"}),
        json!({"flexShrink": 1001}),
        json!({"justifyContent": "flex-start"}),
        json!({"justifyContent": "Stretch"}),
        json!({"justifyContent": "Baseline"}),
        json!({"alignItems": "Start"}),
        json!({"alignItems": 1}),
        json!({"flexBasis": 1}),
        json!({"padding": {"top": 0}}),
    ] {
        let code = flex
            .set(&child, properties.clone())
            .expect_err(&format!("{properties} is refused"));
        assert_eq!(code, "bad_request", "{properties}: {code}");
    }

    assert_eq!(flex.child_layout.flex_grow(), 0.0);
    assert_eq!(flex.child_layout.flex_shrink(), 0.0);
    assert_eq!(flex.child_layout.justify_content(), Justify::FlexStart);
    assert_eq!(flex.child_layout.align_items(), Align::Stretch);
    assert_eq!(flex.probe.scene.editor.state().override_count, 0);

    // The documented bounds themselves are legal.
    flex.set(&child, json!({"flexGrow": 0, "flexShrink": 1000}))
        .unwrap();
    assert_eq!(flex.child_layout.flex_shrink(), 1000.0);
}

#[test]
fn a_layout_less_anchor_refuses_flex_properties() {
    let root = Node::new();
    let mut nodes = NodeTree::new();
    nodes.set_root(Some(root.clone()));
    let mut probe = RuntimeProbe::new(Rc::new(RefCell::new(nodes)), None, None, None);

    let fault = probe
        .execute(ProbeCommand::Edit {
            name: "inspect.style.set".to_owned(),
            arguments: json!({
                "id": root.id().0.to_string(),
                "properties": {"flexGrow": 1},
            }),
        })
        .unwrap_err();
    assert_eq!(fault.code, "unsupported");
    assert_eq!(probe.scene.editor.state().override_count, 0);
}

#[test]
fn unmounting_a_node_restores_and_prunes_its_flex_override() {
    let mut flex = Flex::new();
    let child = flex.child_id();
    flex.layout();
    flex.set(&child, json!({"flexGrow": 1})).unwrap();
    assert_eq!(flex.child_layout.flex_grow(), 1.0);

    flex.root.remove_child(&flex.child);

    let state = flex.probe.scene.editor.state();
    assert_eq!(state.override_count, 0, "an unmounted node is pruned");
    assert_eq!(
        flex.child_layout.flex_grow(),
        0.0,
        "pruning restores the authored value instead of leaking the override"
    );
    assert!(
        flex.probe
            .scene
            .editor
            .override_properties(&flex.child.id().0.to_string())
            .is_empty()
    );
}

#[test]
fn snapshots_publish_the_flex_override_names() {
    let mut flex = Flex::new();
    let child = flex.child_id();
    flex.layout();
    flex.set(
        &child,
        json!({"flexGrow": 1, "justifyContent": "SpaceAround"}),
    )
    .unwrap();

    let snapshot = flex.probe.scene.snapshot();
    let layout = snapshot
        .root
        .as_ref()
        .and_then(|root| root.children.first())
        .and_then(|child| child.layout.as_ref())
        .expect("the child reports its layout");
    assert_eq!(layout.flex_grow, 1.0);
    assert_eq!(layout.justify_content, "SpaceAround");
    let styled = snapshot
        .root
        .as_ref()
        .and_then(|root| root.children.first())
        .and_then(|child| child.style.as_ref())
        .expect("the child reports its style");
    assert_eq!(
        styled.overrides,
        vec!["flexGrow".to_owned(), "justifyContent".to_owned()]
    );

    flex.clear(&child).unwrap();
    let snapshot = flex.probe.scene.snapshot();
    let styled = snapshot
        .root
        .as_ref()
        .and_then(|root| root.children.first())
        .and_then(|child| child.style.as_ref())
        .expect("the child reports its style");
    assert!(styled.overrides.is_empty());
}
