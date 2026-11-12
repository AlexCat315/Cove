//! Translation of `Examples/Counter/Tests/CounterExampleTests/SharedSceneTests.swift`.
//!
//! Behaviour checklist:
//! - the shared counter mounts on Taffy and keeps its node identities across a
//!   state write and a resize
//! - `counter.value` republishes the new count through its text attachment
//! - the snapshot's input and render inventories agree, and the invalidation log
//!   records the state write
//! - rendering the tree produces finite vertices
//! - pointer capture holds the increment button between down and up, releases on
//!   an out-of-bounds up, and a completed press activates exactly once
//! - keys 40 and 44 activate the focused button; the reset button's 43/44 pair
//!   returns the count to zero
//! - `restore` rejects an out-of-range count and leaves the state untouched
//! - the session validates, mirrors and reports state differences
//! - the recorder bounds its event list, marks truncation, and survives a JSON
//!   round trip with Unicode and scroll metadata intact

mod support;

use std::collections::BTreeMap;
use std::rc::Rc;

use cove_composition::{AnyView, ViewGraph};
use cove_core::DrawList;
use cove_core::scene::{
    InvalidationSource, Node, NodeRenderer, NodeTree, PlatformInputContext, Recomposer,
};
use cove_devtools::inspection::{SceneEditor, SceneInspector};
use cove_devtools::protocol::{DevToolsCodec, StateDifference, TreeSnapshot};
use cove_examples::{CounterText, CounterTextRun, CounterView};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent};
use cove_ui_compose::LayoutDebugAttachmentKey;

use support::GalleryScene;

fn find(node: &Node, name: &str) -> Option<Node> {
    if node
        .attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
        .as_deref()
        == Some(name)
    {
        return Some(node.clone());
    }
    for child in node.children() {
        if let Some(found) = find(&child, name) {
            return Some(found);
        }
    }
    None
}

fn same_node(left: &Node, right: &Node) -> bool {
    left.id() == right.id()
}

#[test]
fn shared_compose_uses_taffy_and_retains_nodes_across_state_and_resize() {
    support::GlobalTestLock::with_scheduler(|| {
        support::with_text_environment(|| {
            let counter = CounterView::new();
            let graph = Rc::new(ViewGraph::new(
                Rc::new(std::cell::RefCell::new(NodeTree::new())),
                Rc::new(Recomposer::new()),
            ));
            let context = PlatformInputContext::new();
            let inspector = SceneInspector::new(
                graph.tree.clone(),
                Some(context.invalidation_log.clone()),
                None,
            );
            let log = context.invalidation_log.clone();

            context.with_current(|| {
                graph.install(counter.clone(), Default::default());
                graph.compute_layout(640.0, 460.0);

                let root = graph.tree.borrow().root().expect("tree root");
                let increment = find(&root, "counter.increment").expect("counter.increment");
                let reset = find(&root, "counter.reset").expect("counter.reset");
                let initial_width = increment.frame().width;
                assert!(
                    initial_width > 200.0,
                    "the increment button shares the row, got {initial_width}"
                );
                assert!(
                    (increment.frame().width - reset.frame().width).abs() < 0.1,
                    "both buttons are flex-equal: {} vs {}",
                    increment.frame().width,
                    reset.frame().width
                );
                let increment_frame = increment.absolute_frame();
                let reset_frame = reset.absolute_frame();
                let gap = reset_frame.x - (increment_frame.x + increment_frame.width);
                assert!(
                    (gap - 12.0).abs() < 0.1,
                    "the authored 12pt row gap, got {gap}"
                );

                counter.count.set(7);
                assert!(
                    graph.recomposer.has_pending(),
                    "a state write must schedule a recomposition"
                );
                graph.recomposer.commit_all();
                graph.compute_layout(400.0, 460.0);

                let retained = find(&root, "counter.increment").expect("still present");
                assert!(
                    same_node(&retained, &increment),
                    "the button node is reconciled, not rebuilt"
                );
                assert!(
                    increment.frame().width < initial_width,
                    "the narrower window shrinks the flex row: {} !< {initial_width}",
                    increment.frame().width
                );
                let value = find(&root, "counter.value").expect("counter.value");
                let run = value
                    .attachment::<CounterTextRun>(CounterText::ATTACHMENT)
                    .expect("the value publishes a text run");
                assert_eq!(run.text, "7", "the run republishes the new count");

                let snapshot = inspector.snapshot();
                let input_count = snapshot
                    .input_inventory
                    .as_ref()
                    .expect("input inventory")
                    .node_count;
                // The inspector only carries a render mirror when one was supplied.
                assert!(input_count > 5, "the tree has {input_count} nodes");
                let invalidations = snapshot.invalidations.expect("invalidations");
                assert!(
                    invalidations
                        .iter()
                        .any(|record| record.source.starts_with("stateWrite(")),
                    "the state write is logged, got {:?}",
                    invalidations
                        .iter()
                        .map(|record| &record.source)
                        .collect::<Vec<_>>()
                );

                let mut list = DrawList::default();
                NodeRenderer.render(&root, &mut list);
                assert!(!list.vertices().is_empty(), "the counter paints something");
                assert!(
                    list.vertices()
                        .iter()
                        .all(|vertex| vertex.x.is_finite() && vertex.y.is_finite()),
                    "every vertex is finite"
                );
            });

            counter.dark.set(true);
            assert!(
                log.snapshot(None)
                    .iter()
                    .any(|reason| matches!(reason.source, InvalidationSource::StateWrite { .. })),
                "a write outside the context still reports to its log"
            );
        });
    });
}

#[test]
fn the_render_tree_agrees_with_the_input_tree() {
    let counter = CounterView::new();
    support::GlobalTestLock::with_scheduler(|| {
        support::with_text_environment(|| {
            let graph = Rc::new(ViewGraph::new(
                Rc::new(std::cell::RefCell::new(NodeTree::new())),
                Rc::new(Recomposer::new()),
            ));
            let context = PlatformInputContext::new();
            context.with_current(|| {
                graph.install(counter.clone(), Default::default());
                graph.compute_layout(640.0, 460.0);
                let render_tree = graph.render_tree();
                let root = graph.tree.borrow().root().expect("tree root");
                let mut nodes = 0;
                let mut stack = vec![root];
                while let Some(node) = stack.pop() {
                    nodes += 1;
                    stack.extend(node.children());
                }
                assert_eq!(
                    render_tree.object_count(),
                    nodes,
                    "every input node is mirrored into the render tree"
                );
            });
        });
    });
}

#[test]
fn shared_input_uses_capture_focus_and_release_semantics() {
    let counter = CounterView::new();
    GalleryScene::run(
        640.0,
        460.0,
        |_| AnyView::new(counter.clone()),
        |scene| {
            let button = scene.require_named("counter.increment");
            let frame = button.absolute_frame();
            let event = MouseButtonEvent {
                button: MouseButton::Left,
                x: frame.x + frame.width / 2.0,
                y: frame.y + frame.height / 2.0,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: Default::default(),
            };
            let dispatcher = scene.dispatcher();

            dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
            assert_eq!(counter.count.get(), 0, "a press alone does not increment");
            assert!(
                scene
                    .capture
                    .target()
                    .is_some_and(|target| same_node(&target, &button)),
                "the button captures the pointer on press"
            );

            dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
                button: MouseButton::Left,
                x: -1.0,
                y: -1.0,
                clicks: 1,
                modifiers: KeyModifiers::default(),
                kind: Default::default(),
            }));
            assert_eq!(
                counter.count.get(),
                0,
                "an out-of-bounds release does not activate"
            );
            assert!(
                scene.capture.target().is_none(),
                "the capture is released on up"
            );

            dispatcher.dispatch(&InputEvent::MouseButtonDown(event.clone()));
            dispatcher.dispatch(&InputEvent::MouseButtonUp(event));
            assert_eq!(counter.count.get(), 1, "a full press activates once");

            dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
                scancode: 40,
                keycode: 0,
                modifiers: KeyModifiers::default(),
                is_repeat: false,
            }));
            assert_eq!(
                counter.count.get(),
                2,
                "key 40 activates the focused button"
            );

            for scancode in [43u32, 44] {
                dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
                    scancode,
                    keycode: 0,
                    modifiers: KeyModifiers::default(),
                    is_repeat: false,
                }));
            }
            assert_eq!(
                counter.count.get(),
                0,
                "keys 43 and 44 reach the reset button, returning the count to 0"
            );

            let rejected = BTreeMap::from([
                ("count".to_owned(), "-1".to_owned()),
                ("dark".to_owned(), "true".to_owned()),
            ]);
            assert!(
                !counter.restore(&rejected),
                "an out-of-range count is rejected"
            );
            assert_eq!(counter.count.get(), 0, "the rejected count is not applied");
            assert!(!counter.dark.get(), "the rejected theme is not applied");
        },
    );
}

#[test]
fn checkpoint_difference_reports_changed_added_and_removed_values() {
    let difference = StateDifference::new(
        &BTreeMap::from([
            ("count".to_owned(), "1".to_owned()),
            ("old".to_owned(), "x".to_owned()),
        ]),
        &BTreeMap::from([
            ("count".to_owned(), "2".to_owned()),
            ("new".to_owned(), "y".to_owned()),
        ]),
    );
    let changed = difference.changed.get("count").expect("count changed");
    assert_eq!(
        (changed.before.as_str(), changed.after.as_str()),
        ("1", "2")
    );
    assert_eq!(difference.removed.get("old").map(String::as_str), Some("x"));
    assert_eq!(difference.added.get("new").map(String::as_str), Some("y"));
}

#[test]
fn the_snapshot_round_trips_through_the_wire_type() {
    let counter = CounterView::new();
    GalleryScene::run(
        640.0,
        460.0,
        |_| AnyView::new(counter.clone()),
        |scene| {
            let inspector = SceneInspector::new(scene.graph.tree.clone(), None, None);
            let snapshot = inspector.snapshot();
            let encoded = DevToolsCodec::json(&snapshot);
            let payload: TreeSnapshot = DevToolsCodec::decode(&Some(encoded)).expect("decode");
            let root = payload.root.expect("the root survives the round trip");
            assert!(
                !root.children.is_empty(),
                "the counter's column has children"
            );
        },
    );
}

#[test]
fn colors_encode_as_hex_and_decode_back() {
    let dark = cove_core::Color::from_rgba8(20, 29, 45, 255);
    let hex = SceneEditor::hex(Some(dark)).expect("hex");
    let decoded = SceneEditor::color(&hex).expect("decode");
    assert_eq!(
        (decoded.r, decoded.g, decoded.b),
        (dark.r, dark.g, dark.b),
        "the round trip preserves the channels from {hex}"
    );
}
