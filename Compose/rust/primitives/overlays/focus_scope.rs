//! A modal subtree that traps focus and optionally restores commands.
use cove_core::scene::{FocusChain, FocusChainHolder, LayoutNode, ModalFocusResource, Node};
use crate::{impl_primitive_view, AnyView, PrimitiveView, View};
use std::rc::{Rc, Weak};

pub struct FocusScope {
    content: AnyView,
    restores_commands: bool,
}

impl FocusScope {
    pub fn new(content: impl View + 'static) -> Self {
        Self::with_restoring_commands(false, content)
    }

    pub fn with_restoring_commands(restores_commands: bool, content: impl View + 'static) -> Self {
        Self { content: AnyView::new(content), restores_commands }
    }
}

impl PrimitiveView for FocusScope {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_is_focusable(true);
        node.add_resource(Rc::new(ModalFocusResource::new()));
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> { None }

    fn update_node(&self, node: &Node) {
        let chain = FocusChainHolder::current();
        if let Some(resource) = node.first_resource::<ModalFocusResource>() {
            resource.bind(node, chain.clone(), self.restores_commands);
        }
        let weak_chain: Option<Weak<FocusChain>> = chain.as_ref().map(Rc::downgrade);
        node.set_layout_did_update(weak_chain.map(|weak| {
            Rc::new(move |node: &Node| {
                if let Some(chain) = weak.upgrade() { chain.settle_scope(node); }
            }) as cove_core::scene::LayoutUpdateHook
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> { vec![self.content.clone()] }
}
impl_primitive_view!(FocusScope);
