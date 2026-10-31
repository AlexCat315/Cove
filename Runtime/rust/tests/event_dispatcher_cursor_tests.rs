//! Translation of `Tests/CoveUIRuntimeTests/EventDispatcherCursorTests.swift`.
//!
//! Behaviour checklist:
//! - the cursor sink fires when hover crosses into a node with a cursor set
//! - the cursor reverts to arrow when leaving a cursor-bearing node
//! - no sink emission when the same cursor stays
//! - a node that changes its cursor during motion updates it without leaving

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Rect;
use cove_core::scene::{
    EventDispatcher, EventResult, FocusChain, InputHandlerRoute, InteractionRegistry, Node,
    NodeTree, PointerCapture,
};
use cove_platform::{InputEvent, MouseMotionEvent, SystemCursor, WindowId};

struct TwoNode {
    root: Node,
    leaf: Node,
    received: Rc<RefCell<Vec<SystemCursor>>>,
    dispatcher: EventDispatcher,
}

fn two_node_tree() -> TwoNode {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    let leaf = Node::new();
    leaf.set_frame(Rect::new(10.0, 10.0, 40.0, 40.0));
    root.add_child(&leaf);
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    tree.borrow_mut().set_root(Some(root.clone()));

    let received = Rc::new(RefCell::new(Vec::new()));
    let sink = received.clone();
    let mut dispatcher = EventDispatcher::new(
        tree,
        Rc::new(InteractionRegistry::new()),
        Rc::new(PointerCapture::new()),
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );
    dispatcher.cursor_sink = Some(Rc::new(move |cursor: SystemCursor| {
        sink.borrow_mut().push(cursor);
    }));
    TwoNode {
        root,
        leaf,
        received,
        dispatcher,
    }
}

fn motion(x: f32, y: f32) -> InputEvent {
    InputEvent::MouseMotion(MouseMotionEvent {
        x,
        y,
        delta_x: 0.0,
        delta_y: 0.0,
        kind: Default::default(),
    })
}

#[test]
fn cursor_enter() {
    let scene = two_node_tree();
    scene.leaf.set_cursor(Some(SystemCursor::Pointer));

    scene.dispatcher.dispatch(&motion(20.0, 20.0));

    assert_eq!(*scene.received.borrow(), [SystemCursor::Pointer]);
    // `root` only keeps the tree alive, as in the Swift test.
    assert_eq!(scene.root.children().len(), 1);
}

#[test]
fn cursor_leave() {
    let scene = two_node_tree();
    scene.leaf.set_cursor(Some(SystemCursor::Pointer));

    scene.dispatcher.dispatch(&motion(20.0, 20.0));
    // Move outside leaf but still inside root.
    scene.dispatcher.dispatch(&motion(80.0, 80.0));

    assert_eq!(
        *scene.received.borrow(),
        [SystemCursor::Pointer, SystemCursor::Arrow]
    );
}

#[test]
fn cursor_stable() {
    let scene = two_node_tree();
    scene.leaf.set_cursor(Some(SystemCursor::Pointer));

    scene.dispatcher.dispatch(&motion(20.0, 20.0));
    scene.dispatcher.dispatch(&motion(25.0, 25.0));

    assert_eq!(*scene.received.borrow(), [SystemCursor::Pointer]);
}

#[test]
fn cursor_within_node() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    root.set_is_hit_testable(true);
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    tree.borrow_mut().set_root(Some(root.clone()));

    let interactions = Rc::new(InteractionRegistry::new());
    // A paragraph shows the pointer only over its link region.
    let cursor_root = root.clone();
    interactions.set_motion(
        &root,
        InputHandlerRoute::control(),
        Rc::new(move |event: &MouseMotionEvent, _| {
            cursor_root.set_cursor(if event.x > 50.0 {
                Some(SystemCursor::Pointer)
            } else {
                None
            });
            EventResult::Ignored
        }),
    );

    let received = Rc::new(RefCell::new(Vec::new()));
    let sink = received.clone();
    let mut dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );
    dispatcher.cursor_sink = Some(Rc::new(move |cursor: SystemCursor| {
        sink.borrow_mut().push(cursor);
    }));

    dispatcher.dispatch(&motion(20.0, 20.0));
    dispatcher.dispatch(&motion(70.0, 20.0));
    dispatcher.dispatch(&motion(20.0, 20.0));

    assert_eq!(
        *received.borrow(),
        [SystemCursor::Pointer, SystemCursor::Arrow],
        "motion re-resolves the cursor even when the hover path is unchanged",
    );
}
