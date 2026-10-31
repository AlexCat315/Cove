use crate::AnyView;
use cove_core::scene::{LayoutNode, Node};
pub trait PrimitiveView {
    fn make_node(&self) -> Node;
    fn update_node(&self, node: &Node);
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, _layout: &LayoutNode) {}
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
#[macro_export]
macro_rules! impl_primitive_view {
    ($type:ty) => {
        impl $crate::View for $type {
            fn kind(&self) -> $crate::ViewKind<'_> {
                $crate::ViewKind::Primitive(self)
            }
            fn tag(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    };
}
