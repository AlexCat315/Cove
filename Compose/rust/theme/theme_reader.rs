//! Translation of Theme/ThemeReader.swift.
use super::{Theme, theme_for_node};
use crate::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::scene::{LayoutNode, Node};
use std::rc::Rc;

/// Resolves the active theme while constructing content (for syntax colors
/// and other palettes that cannot be expressed as a single semantic token).
pub struct ThemeReader<Content: View> {
    content: Rc<dyn Fn(&Theme) -> Content>,
}
impl<Content: View> ThemeReader<Content> {
    pub fn new(content: impl Fn(&Theme) -> Content + 'static) -> Self {
        Self {
            content: Rc::new(content),
        }
    }
}
impl<Content: View + 'static> PrimitiveView for ThemeReader<Content> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    // Reading an environment must not insert an extra layout container that
    // prevents a flex editor from filling the space assigned by its parent.
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        vec![AnyView::new((self.content)(&theme))]
    }
}
impl<Content: View + 'static> View for ThemeReader<Content> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
