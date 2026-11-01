//! A CSS Grid container backed directly by the retained Taffy layout tree.
use crate::{AnyView, PrimitiveView, impl_primitive_view};
use cove_core::scene::{Display, GridStyle, Gutter, LayoutNode, LayoutStyleValue, Node};

#[derive(Clone)]
pub struct CssGrid {
    pub style: GridStyle,
    pub column_gap: LayoutStyleValue,
    pub row_gap: LayoutStyleValue,
    pub content: AnyView,
}
impl CssGrid {
    pub fn new(style: GridStyle, content: AnyView) -> Self {
        Self {
            style,
            column_gap: LayoutStyleValue::Points(0.0),
            row_gap: LayoutStyleValue::Points(0.0),
            content,
        }
    }
}
impl PrimitiveView for CssGrid {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_display(Display::Grid);
        layout.set_grid_style(self.style.clone());
        layout.set_gap_value(self.column_gap, Gutter::Column);
        layout.set_gap_value(self.row_gap, Gutter::Row);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(CssGrid);
