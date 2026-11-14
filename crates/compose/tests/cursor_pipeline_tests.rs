//! Translation of `Tests/CoveUIComposeTests/CursorPipelineTests.swift`.
//!
//! Behaviour checklist:
//! - an enabled button writes the pointer cursor onto its host node
//! - a disabled button writes the not-allowed cursor instead
//! - a text field writes the ibeam cursor
//! - `.cursor(_:)` overrides the cursor the primitive wrote

use crate::support;

use cove_composition::{View, ViewGraph};
use cove_core::scene::Node;
use cove_platform::SystemCursor;
use cove_ui_compose::modifiers::interaction::CursorExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::{Text, TextField};

use support::GlobalTestLock;

/// The Swift helper skips the anchor root and prefers the deepest hit-testable
/// descendant, which is the primitive the widget materialised.
fn first_hit_testable(node: &Node) -> Option<Node> {
    for child in node.children() {
        if let Some(deeper) = first_hit_testable(&child) {
            return Some(deeper);
        }
    }
    if node.is_hit_testable() && node.parent().is_some() {
        Some(node.clone())
    } else {
        None
    }
}

fn installed_cursor(root: impl View + 'static) -> Option<SystemCursor> {
    let graph = ViewGraph::new(
        std::rc::Rc::new(std::cell::RefCell::new(cove_core::scene::NodeTree::new())),
        std::rc::Rc::new(cove_core::scene::Recomposer::new()),
    );
    let graph = std::rc::Rc::new(graph);
    graph.install(root, Default::default());
    let tree = graph.tree.borrow();
    let root_node = tree.root().expect("installed root");
    first_hit_testable(&root_node).and_then(|node| node.cursor())
}

#[test]
fn enabled_button_has_pointer_cursor() {
    GlobalTestLock::locked(|| {
        assert_eq!(
            installed_cursor(Button::new(Text::new("Tap"), || {})),
            Some(SystemCursor::Pointer),
            "an enabled button shows the pointer cursor"
        );
    });
}

#[test]
fn disabled_button_has_not_allowed_cursor() {
    GlobalTestLock::locked(|| {
        assert_eq!(
            installed_cursor(Button::with_options(
                Text::new("Off"),
                || {},
                cove_ui_compose::theme::styles::buttons::ButtonRole::Normal,
                false,
                false,
                false,
                None,
            )),
            Some(SystemCursor::NotAllowed),
            "a disabled button shows the not-allowed cursor"
        );
    });
}

#[test]
fn text_field_has_ibeam_cursor() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            assert_eq!(
                installed_cursor(TextField::new(
                    "placeholder",
                    cove_core::Binding::new(TextBuffer::empty, |_| {}),
                )),
                Some(SystemCursor::Ibeam),
                "a text field shows the ibeam cursor"
            );
        });
    });
}

#[test]
fn cursor_modifier_overrides_the_primitive() {
    GlobalTestLock::locked(|| {
        assert_eq!(
            installed_cursor(Button::new(Text::new("Tap"), || {}).cursor(SystemCursor::Crosshair)),
            Some(SystemCursor::Crosshair),
            "the modifier runs after the primitive and wins"
        );
    });
}

#[test]
fn text_cursor_survives_focus_drag_and_recomposition() {
    use cove_core::Point;
    use cove_platform::{InputEvent, MouseButtonEvent, MouseMotionEvent};
    use cove_ui_compose::primitives::text::TextSelectionViewExt;
    use support::InteractiveScene;

    fn motion(point: Point) -> InputEvent {
        InputEvent::MouseMotion(MouseMotionEvent {
            x: point.x,
            y: point.y,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        })
    }
    for field in [false, true] {
        InteractiveScene::run(320.0, 100.0, |scene| {
            if field {
                scene.install(TextField::new(
                    "Type here",
                    cove_core::Binding::new(TextBuffer::empty, |_| {}),
                ));
            } else {
                scene.install(Text::new("Selectable text").text_selection(true));
            }
            let text = scene
                .first(|node| node.cursor() == Some(SystemCursor::Ibeam))
                .unwrap();
            let bounds = text.absolute_frame();
            let inside = Point::new(bounds.x + 4.0, bounds.y + bounds.height * 0.5);
            let events = scene.dispatcher();
            events.dispatch(&motion(inside));
            assert_eq!(events.cursor(), SystemCursor::Ibeam);
            let press = MouseButtonEvent {
                x: inside.x,
                y: inside.y,
                button: cove_platform::MouseButton::Left,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            };
            events.dispatch(&InputEvent::MouseButtonDown(press.clone()));
            scene.settle(2, 1.0);
            events.dispatch(&motion(Point::new(inside.x + 20.0, inside.y)));
            assert_eq!(
                events.cursor(),
                SystemCursor::Ibeam,
                "focus, capture and recompose keep the text cursor"
            );
            events.dispatch(&InputEvent::MouseButtonUp(press));
            events.dispatch(&motion(Point::new(-10.0, -10.0)));
            assert_eq!(
                events.cursor(),
                SystemCursor::Arrow,
                "leaving text restores the arrow"
            );
        });
    }
}

#[test]
fn a_label_inside_a_control_keeps_the_control_cursor() {
    use cove_core::Point;
    use cove_platform::{InputEvent, MouseMotionEvent};
    use support::InteractiveScene;

    fn motion(point: Point) -> InputEvent {
        InputEvent::MouseMotion(MouseMotionEvent {
            x: point.x,
            y: point.y,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        })
    }

    InteractiveScene::run(480.0, 200.0, |scene| {
        scene.install(Button::title("Save", || {}));
        let label = scene
            .first(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.label == "Save" && node.children().is_empty()
                })
            })
            .expect("the button label");
        let bounds = label.absolute_frame();
        let inside = Point::new(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        let events = scene.dispatcher();
        events.dispatch(&motion(inside));
        assert_eq!(
            events.cursor(),
            SystemCursor::Pointer,
            "text inside a control shows the control's cursor, not the text cursor"
        );
    });
}
