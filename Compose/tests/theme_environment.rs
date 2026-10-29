//! Translation of Tests/CoveUIComposeTests/ThemeEnvironmentTests.swift.
use cove_composition::{PrimitiveView, View, ViewGraph, ViewKind};
use cove_core::scene::{ComponentSourceLocation, Node, NodeTree, Recomposer};
use cove_core::{Color, Font};
use cove_ui_compose::modifiers::style::{FONT, LINE_HEIGHT};
use cove_ui_compose::theme::{
    theme_for_node, SemanticColorRef, SemanticFontRef, SemanticStyleViewExt, Theme, ThemeViewExt,
};
use std::cell::RefCell;
use std::rc::Rc;

struct DebugNode {
    label: String,
}
impl View for DebugNode {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        "DebugNode"
    }
}
impl PrimitiveView for DebugNode {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        let mut frame = node.frame();
        frame.origin.x = self.label.chars().count() as f32;
        node.set_frame(frame);
    }
}

fn location() -> ComponentSourceLocation {
    ComponentSourceLocation::new("theme_environment.rs", "Compose/tests/theme_environment.rs", 1)
}

fn descendant_of(root: &Node) -> Node {
    let mut cursor = root.clone();
    while let Some(next) = cursor.children().first().cloned() {
        cursor = next;
    }
    cursor
}

fn install(root: impl View + 'static) -> Rc<RefCell<NodeTree>> {
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    let graph = Rc::new(ViewGraph::new(tree.clone(), Rc::new(Recomposer::new())));
    graph.install(root, location());
    tree
}

#[test]
fn theme_provision() {
    let mut custom = Theme::default_dark();
    custom.colors.accent = Color { r: 0.9, g: 0.1, b: 0.1, a: 1.0 };
    let (tree, _) = install(DebugNode { label: "leaf".into() }.theme(custom));
    let leaf = descendant_of(&tree.borrow().root().unwrap());
    assert_eq!(theme_for_node(&leaf).colors.accent, Color { r: 0.9, g: 0.1, b: 0.1, a: 1.0 });
}

#[test]
fn theme_fallback() {
    let (tree, _) = install(DebugNode { label: "leaf".into() });
    let leaf = descendant_of(&tree.borrow().root().unwrap());
    assert_eq!(theme_for_node(&leaf).colors.accent, Theme::default_dark().colors.accent);
}

#[test]
fn semantic_background_resolves_on_install() {
    let mut custom = Theme::default_dark();
    custom.colors.surface = Color { r: 0.5, g: 0.0, b: 0.5, a: 1.0 };
    let (tree, _) = install(
        DebugNode { label: "leaf".into() }
            .semantic_background(SemanticColorRef::surface())
            .theme(custom),
    );
    let leaf = descendant_of(&tree.borrow().root().unwrap());
    assert_eq!(leaf.background_color(), Some(Color { r: 0.5, g: 0.0, b: 0.5, a: 1.0 }));
}

#[test]
fn semantic_foreground_resolves_on_install() {
    let mut custom = Theme::default_dark();
    custom.colors.on_surface = Color { r: 0.1, g: 0.9, b: 0.2, a: 1.0 };
    let (tree, _) = install(
        DebugNode { label: "leaf".into() }
            .semantic_foreground(SemanticColorRef::on_surface())
            .theme(custom),
    );
    let leaf = descendant_of(&tree.borrow().root().unwrap());
    assert_eq!(leaf.foreground_color(), Some(Color { r: 0.1, g: 0.9, b: 0.2, a: 1.0 }));
}

#[test]
fn semantic_font_writes_attachments() {
    let (tree, _) = install(
        DebugNode { label: "leaf".into() }.semantic_font(SemanticFontRef::title()),
    );
    let leaf = descendant_of(&tree.borrow().root().unwrap());
    let title = Theme::default_dark().typography.title;
    assert_eq!(leaf.attachment::<Font>(FONT), Some(title.font.clone()));
    assert_eq!(leaf.attachment::<f32>(LINE_HEIGHT), Some(title.line_height));
}

#[test]
fn nested_theme_overrides() {
    let mut outer = Theme::default_dark();
    outer.colors.accent = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let mut inner = Theme::default_dark();
    inner.colors.accent = Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    let (tree, _) = install(DebugNode { label: "leaf".into() }.theme(inner).theme(outer));
    let leaf = descendant_of(&tree.borrow().root().unwrap());
    // Innermost wins.
    assert_eq!(theme_for_node(&leaf).colors.accent, Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 });
}

// Keep AnyView referenced so this test file also validates the erased root path.
#[allow(dead_code)]
fn erased_root(view: impl View + 'static) -> AnyView {
    AnyView::new(view)
}
