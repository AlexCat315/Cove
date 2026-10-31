//! Translation of Primitives/Controls/Form/LabelScope.swift.
use crate::{AnyView, PrimitiveView, impl_primitive_view};
use cove_core::scene::{LayoutNode, Node};

pub const FORM_LABEL_SCOPE_KEY: &str = "CoveUI.form.label-scope";

/// A field ID is local to its form. Identical field names in another form
/// must not steal focus when this form's associated label is activated.
pub struct FormLabelScope {
    pub content: AnyView,
}

impl FormLabelScope {
    pub fn new(content: impl crate::View + 'static) -> Self {
        Self {
            content: AnyView::new(content),
        }
    }
    pub fn with_erased_content(content: AnyView) -> Self {
        Self { content }
    }
}

impl PrimitiveView for FormLabelScope {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(FORM_LABEL_SCOPE_KEY, true);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(FormLabelScope);
