//! Translation of `Tests/CoveUIComposeTests/CursorPipelineTests.swift`.
//!
//! Behaviour checklist:
//! - an enabled button writes the pointer cursor onto its host node
//! - a disabled button writes the not-allowed cursor instead
//! - a text field writes the ibeam cursor
//! - `.cursor(_:)` overrides the cursor the primitive wrote

mod support;

use cove_composition::{AnyView, View, ViewGraph};
use cove_core::scene::Node;
use cove_platform::SystemCursor;
use cove_ui_compose::modifiers::interaction::CursorExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::text::{Text, TextField};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;

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
        std::rc::Rc::new(std::cell::RefCell::new(
            cove_core::scene::NodeTree::new(),
        )),
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
