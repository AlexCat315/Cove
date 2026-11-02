//! Pointer and keyboard input shared by selectable text surfaces.
use super::rich::text_selection::{SharedTextSelectionScope, TextSelectionScope};
use crate::Scancode;
use cove_core::Point;
use cove_core::scene::{
    EventPhase, EventResult, InputHandlerRoute, InteractionRegistryHolder, Node,
    PointerCaptureHolder, PointerPhase,
};
use cove_platform::{MouseButton, PointerKind, SystemCursor};
use std::rc::Rc;

pub(super) fn install(node: &Node, scope: &SharedTextSelectionScope) {
    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };
    node.set_is_hit_testable(true);
    node.set_is_focusable(true);
    node.set_is_tab_stop(false);
    node.set_cursor(Some(SystemCursor::Ibeam));

    let owner = node.downgrade();
    let selection = scope.clone();
    registry.set_pointer(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, phase, _| {
            let Some(node) = Node::upgrade(&owner) else {
                return EventResult::Ignored;
            };
            if event.button != MouseButton::Left {
                return EventResult::Ignored;
            }
            let Some(capture) = PointerCaptureHolder::current() else {
                return EventResult::Ignored;
            };
            let point = Point::new(event.x, event.y);
            let mut selection = selection.borrow_mut();
            match phase {
                PointerPhase::Down => {
                    let Some(pos) = selection.position(point) else {
                        return EventResult::Ignored;
                    };
                    match event.clicks {
                        3.. => selection.select_paragraph(&node),
                        2 => selection.select_word(point),
                        _ if event.modifiers.has_shift() => selection.extend(pos),
                        _ => selection.select(pos, pos),
                    }
                    capture.acquire(&node, event.kind == PointerKind::Touch);
                }
                PointerPhase::Up if capture.target().as_ref() == Some(&node) => {
                    capture.release();
                }
                PointerPhase::Up => return EventResult::Ignored,
            }
            EventResult::Handled
        }),
    );

    let owner = node.downgrade();
    let selection = scope.clone();
    registry.set_motion(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, _| {
            let Some(captured) = PointerCaptureHolder::current().and_then(|c| c.target()) else {
                return EventResult::Ignored;
            };
            if Some(captured) != Node::upgrade(&owner) {
                return EventResult::Ignored;
            }
            let mut selection = selection.borrow_mut();
            if let Some(pos) = selection.position(Point::new(event.x, event.y)) {
                selection.extend(pos);
            }
            EventResult::Handled
        }),
    );

    let selection = scope.clone();
    registry.set_key(
        node,
        InputHandlerRoute::text_input(),
        Rc::new(move |event, phase| {
            if phase != EventPhase::Target {
                return EventResult::Ignored;
            }
            let mut selection = selection.borrow_mut();
            let primary = event.modifiers.has_gui() || event.modifiers.has_ctrl();
            match event.scancode {
                Scancode::A if primary => selection.select_all(),
                Scancode::C if primary && selection.has_selection() => selection.copy(),
                Scancode::ESCAPE if selection.has_selection() => selection.clear(),
                _ => return EventResult::Ignored,
            }
            EventResult::Handled
        }),
    );
}

pub(super) fn disable(node: &Node) {
    TextSelectionScope::unregister(node);
    if let Some(registry) = InteractionRegistryHolder::current() {
        registry.remove(node);
    }
    node.set_is_focusable(false);
    node.set_is_hit_testable(false);
    node.set_cursor(None);
}
