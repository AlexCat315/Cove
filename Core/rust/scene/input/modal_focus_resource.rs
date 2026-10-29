//! Translation of CoveUIScene/Input/ModalFocusResource.swift.
use super::{FocusChain, Node, NodeResource, PointerCaptureHolder};
use std::cell::RefCell;
use std::rc::Rc;

/// Restores the previous focus when a modal subtree leaves the window.
#[derive(Default)]
pub struct ModalFocusResource {
    chain: RefCell<Option<Rc<FocusChain>>>,
}
impl ModalFocusResource {
    pub fn new() -> Self {
        Self::default()
    }
    /// Inspection must use the scope's owning window rather than ambient input
    /// from a different window when an agent query arrives between frames.
    pub fn bound_focus_chain(&self) -> Option<Rc<FocusChain>> {
        self.chain.borrow().clone()
    }
    pub fn bind(&self, node: &Node, chain: Option<Rc<FocusChain>>, restores_commands: bool) {
        let same = match (self.chain.borrow().as_ref(), chain.as_ref()) {
            (Some(current), Some(next)) => Rc::ptr_eq(current, next),
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(current) = self.chain.borrow().clone() {
                current.pop_scope(node);
            }
            *self.chain.borrow_mut() = chain.clone();
        }
        if let Some(next) = chain {
            next.push_scope(node, restores_commands);
            if let Some(captured) = PointerCaptureHolder::current_target()
                && !next.permits_input(&captured)
            {
                PointerCaptureHolder::release_current();
            }
        }
    }
    pub fn activate(&self, node: &Node, chain: Option<Rc<FocusChain>>) {
        self.bind(node, chain, false);
    }
}
impl NodeResource for ModalFocusResource {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, node: &Node) {
        if let Some(chain) = self.chain.borrow().clone() {
            chain.end_modal(node);
        }
        *self.chain.borrow_mut() = None;
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_resource_ends_its_scope_on_unmount() {
        let context = super::super::PlatformInputContext::new();
        context.with_current(|| {
            let root = Node::new();
            let dialog = Node::new();
            let button = Node::new();
            button.set_is_focusable(true);
            dialog.add_child(&button);
            root.add_child(&dialog);
            let chain = FocusChainHolder::current().unwrap();
            let resource = Rc::new(ModalFocusResource::new());
            dialog.add_resource(resource.clone());
            resource.bind(&dialog, Some(chain.clone()), false);
            assert!(chain.has_modal_scope());
            assert_eq!(chain.focused(), Some(button));
            dialog.remove_from_parent();
            assert!(!chain.has_modal_scope());
        });
    }
}
