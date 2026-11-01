//! Regression for the `SceneEditor::intercept` RefCell double-borrow panic.
//!
//! Under Rust 2024 an `if let` scrutinee's temporary `RefMut` lives for the whole
//! body, so the original code called `borrow_mut()` while a `borrow()` guard was
//! still alive and panicked with "RefCell already borrowed" on every second
//! mouse-up while picking. Swift's editor held no such guard.

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::NodeTree;
use cove_devtools::inspection::SceneEditor;
use cove_devtools_protocol::Envelope;
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, PointerKind};

fn press(button: MouseButton) -> InputEvent {
    InputEvent::MouseButtonUp(MouseButtonEvent {
        button,
        x: 1.0,
        y: 1.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: PointerKind::default(),
    })
}

#[test]
fn repeated_mouse_ups_while_picking_do_not_panic() {
    let editor = SceneEditor::new(Rc::new(RefCell::new(NodeTree::new())));
    // Enable picking through the real command, so the test exercises the path
    // the host uses rather than poking private state.
    let _ = editor.handle(&Envelope::new("inspect.pick.start", Some(1), None));

    // The first up is swallowed and recorded; the second must consume the record
    // without holding a borrow across the mutation. Before the fix this panicked.
    for _ in 0..4 {
        let _ = editor.intercept(&press(MouseButton::Left));
    }
    let _ = editor.intercept(&press(MouseButton::Right));
    let _ = editor.intercept(&press(MouseButton::Right));
}
