//! Pointer and keyboard input shared by selectable text surfaces.
use super::rich::text_selection::{SharedTextSelectionScope, TextSelectionScope};
use crate::Scancode;
use cove_core::Point;
use cove_core::scene::{
    EventPhase, EventResult, FocusChainHolder, InputHandlerRoute, InteractionRegistryHolder, Node,
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
    node.set_automatically_focus_on_pointer_down(false);
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
            if event.kind == PointerKind::Touch {
                match phase {
                    PointerPhase::Down => {
                        node.set_attachment(
                            "__text_selection_touch",
                            (std::time::Instant::now(), point),
                        );
                        return EventResult::Ignored;
                    }
                    PointerPhase::Up => {
                        let press = node
                            .attachment::<(std::time::Instant, Point)>("__text_selection_touch");
                        node.remove_attachment("__text_selection_touch");
                        let Some((began, origin)) = press else {
                            return EventResult::Ignored;
                        };
                        if began.elapsed().as_millis() < 450
                            || (point.x - origin.x).hypot(point.y - origin.y) >= 10.0
                        {
                            return EventResult::Ignored;
                        }
                        selection.select_word(point);
                        if let Some(focus) = FocusChainHolder::current() {
                            focus.focus(Some(&node), false);
                        }
                        if let Some(menu) = &selection.request_menu {
                            menu(point);
                        }
                        return EventResult::Handled;
                    }
                }
            }
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
                    node.set_attachment("__text_selection_drag", event.clicks < 2);
                    capture.acquire(&node, false);
                    if let Some(focus) = FocusChainHolder::current() {
                        focus.focus(Some(&node), false);
                    }
                }
                PointerPhase::Up if capture.target().as_ref() == Some(&node) => {
                    capture.release();
                    node.remove_attachment("__text_selection_drag");
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
            let Some(node) = Node::upgrade(&owner) else {
                return EventResult::Ignored;
            };
            if let Some((_, origin)) =
                node.attachment::<(std::time::Instant, Point)>("__text_selection_touch")
            {
                if (event.x - origin.x).hypot(event.y - origin.y) >= 10.0 {
                    node.remove_attachment("__text_selection_touch");
                }
                return EventResult::Ignored;
            }
            let Some(captured) = PointerCaptureHolder::current().and_then(|c| c.target()) else {
                return EventResult::Ignored;
            };
            if Some(captured) != Node::upgrade(&owner) {
                return EventResult::Ignored;
            }
            if node.attachment::<bool>("__text_selection_drag") != Some(true) {
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
                Scancode::ARROW_LEFT if event.modifiers.has_shift() => {
                    if !selection.extend_focus(-1) {
                        return EventResult::Ignored;
                    }
                }
                Scancode::ARROW_RIGHT if event.modifiers.has_shift() => {
                    if !selection.extend_focus(1) {
                        return EventResult::Ignored;
                    }
                }
                Scancode::HOME if event.modifiers.has_shift() => {
                    if !selection.extend_focus_to_edge(true) {
                        return EventResult::Ignored;
                    }
                }
                Scancode::END if event.modifiers.has_shift() => {
                    if !selection.extend_focus_to_edge(false) {
                        return EventResult::Ignored;
                    }
                }
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

/// The selection keeps the caret's line geometry. A small corner radius softens
/// its exposed edges without moving the glyphs or changing hit testing.
pub(crate) fn paint_selection(
    list: &mut cove_core::DrawList,
    rect: cove_core::Rect,
    color: cove_core::Color,
) {
    let radius = 2.0_f32.min(rect.width * 0.5).min(rect.height * 0.5);
    let _ = list.add_rounded_rect(rect, radius, color);
}
