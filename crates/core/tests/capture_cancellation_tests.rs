//! Translation of `Tests/Headless/Tests/CoveUISceneTests/Input/CaptureCancellationTests.swift`.
//!
//! Pointer capture must hand over cleanly: the previous owner is told it lost
//! capture exactly once, and while that callback runs the new owner is already
//! visible. When an ancestor stops accepting input, stale capture and focus are
//! dropped everywhere — including every input route — rather than delivered.
//!
//! Behaviour checklist:
//! - transferring capture cancels the old owner once
//! - the new owner is visible while the cancellation callback runs
//! - re-acquiring the same node cancels nothing
//! - releasing twice cancels once, with no target visible during the callback
//! - a disabled ancestor rejects stale captured and focused input on every route
//! - re-enabling the ancestor restores delivery

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::Rect;
use cove_core::scene::{
    EventDispatcher, EventPhase, EventResult, InputHandlerRoute, Node, NodeTree,
    PlatformInputContext, PointerCapture, PointerCaptureAttachmentKey, PointerPhase,
};
use cove_platform::{
    InputEvent, KeyEvent, MouseButton, MouseButtonEvent, TextEditingEvent, WindowId,
};

/// SDL scancode for the space bar; the Rust port keeps scancodes as raw `u32`.
const SCANCODE_SPACE: u32 = 44;

type LoseCapture = Rc<dyn Fn()>;

fn lose_capture(node: &Node, callback: impl Fn() + 'static) {
    node.set_attachment::<LoseCapture>(
        PointerCaptureAttachmentKey::DID_LOSE_CAPTURE,
        Rc::new(callback),
    );
}

fn left_button(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        ..MouseButtonEvent::default()
    }
}

#[test]
fn transfer() {
    let capture = Rc::new(PointerCapture::new());
    let old = Node::new();
    let next = Node::new();

    let cancellations = Rc::new(Cell::new(0_usize));

    let counter = cancellations.clone();
    let observed = capture.clone();
    let expected = next.id();
    lose_capture(&old, move || {
        counter.set(counter.get() + 1);
        assert_eq!(
            observed.target().map(|node| node.id()),
            Some(expected),
            "the new owner is visible while the cancellation callback runs",
        );
    });

    capture.acquire(&old, false);
    capture.acquire(&old, false);
    assert_eq!(
        cancellations.get(),
        0,
        "re-acquiring the same node cancels nothing"
    );

    capture.acquire(&next, false);
    assert_eq!(cancellations.get(), 1, "the transfer cancels the old owner");

    let counter = cancellations.clone();
    let observed = capture.clone();
    lose_capture(&next, move || {
        counter.set(counter.get() + 1);
        assert!(
            observed.target().is_none(),
            "the released target is already gone during the callback",
        );
    });

    capture.release();
    capture.release();
    assert_eq!(cancellations.get(), 2, "a double release cancels once");
}

#[test]
fn disabled_ancestor() {
    let context = Rc::new(PlatformInputContext::new());
    let root = Node::new();
    let child = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    child.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    child.set_is_focusable(true);
    root.add_child(&child);

    let mut node_tree = NodeTree::new();
    node_tree.set_root(Some(root.clone()));

    let dispatcher = EventDispatcher::new(
        Rc::new(RefCell::new(node_tree)),
        context.interactions.clone(),
        context.pointer_capture.clone(),
        context.focus_chain.clone(),
        WindowId::MAIN,
    );

    let delivered = Rc::new(Cell::new(0_usize));
    let cancellations = Rc::new(Cell::new(0_usize));

    let counter = cancellations.clone();
    lose_capture(&child, move || counter.set(counter.get() + 1));

    let counter = delivered.clone();
    context.interactions.set_pointer(
        &child,
        InputHandlerRoute::control(),
        Rc::new(
            move |_: &MouseButtonEvent, _: PointerPhase, _: EventPhase| {
                counter.set(counter.get() + 1);
                EventResult::Handled
            },
        ),
    );

    let counter = delivered.clone();
    context.interactions.set_key(
        &child,
        InputHandlerRoute::shortcut(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            counter.set(counter.get() + 1);
            EventResult::Handled
        }),
    );

    context.pointer_capture.acquire(&child, false);
    context.focus_chain.focus(Some(&child), false);

    root.set_allows_hit_testing(false);

    dispatcher.dispatch(&InputEvent::MouseButtonUp(left_button(10.0, 10.0)));
    dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
        scancode: SCANCODE_SPACE,
        keycode: 0,
        modifiers: Default::default(),
        is_repeat: false,
    }));
    dispatcher.dispatch(&InputEvent::TextInput("ignored".into()));

    assert_eq!(delivered.get(), 0, "a disabled ancestor blocks every route");
    assert_eq!(
        cancellations.get(),
        1,
        "stale capture is cancelled exactly once"
    );
    assert!(
        context.pointer_capture.target().is_none(),
        "the stale capture target is dropped"
    );
    assert!(
        context.focus_chain.focused().is_none(),
        "the stale focus target is dropped"
    );

    root.set_allows_hit_testing(true);
    dispatcher.dispatch(&InputEvent::MouseButtonDown(left_button(10.0, 10.0)));
    assert_eq!(
        delivered.get(),
        1,
        "re-enabling the ancestor restores pointer delivery"
    );

    // Editing input is gated by the same ancestor check, so it must not throw
    // or resurrect the cancelled capture.
    dispatcher.dispatch(&InputEvent::TextEditing(TextEditingEvent {
        text: "ignored".into(),
        start: 0,
        length: 0,
    }));
    assert!(
        context.pointer_capture.target().is_none(),
        "editing input does not re-acquire capture"
    );
}
