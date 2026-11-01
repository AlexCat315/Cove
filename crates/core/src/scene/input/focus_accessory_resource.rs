//! Translation of CoveUIScene/Input/FocusAccessoryResource.swift.
use crate::scene::NodeWeak;
use crate::scene::{
    FocusChain, FocusChainHolder, Node, NodeResource, PointerCapture, PointerCaptureHolder,
};
use std::cell::RefCell;
use std::rc::Rc;

/// A portal's logical scope relationship has exactly the portal node's lifetime.
pub struct FocusAccessoryResource {
    owner: NodeWeak,
    chain: RefCell<Option<Rc<FocusChain>>>,
    capture: RefCell<Option<Rc<PointerCapture>>>,
}
impl FocusAccessoryResource {
    pub fn new(owner: &Node) -> Self {
        Self {
            owner: owner.downgrade(),
            chain: RefCell::new(None),
            capture: RefCell::new(None),
        }
    }
}
impl NodeResource for FocusAccessoryResource {
    fn mount(&self, node: &Node) {
        *self.chain.borrow_mut() = FocusChainHolder::current();
        *self.capture.borrow_mut() = PointerCaptureHolder::current();
        if let (Some(owner), Some(chain)) = (
            self.owner.upgrade().map(Node::from_cell),
            self.chain.borrow().clone(),
        ) {
            chain.attach_accessory(node, &owner);
        }
    }
    fn unmount(&self, node: &Node) {
        if let (Some(captured), Some(chain)) = (
            self.capture
                .borrow()
                .as_ref()
                .and_then(|capture| capture.target()),
            self.chain.borrow().clone(),
        ) && chain.contains(&captured, node)
            && let Some(capture) = self.capture.borrow().as_ref()
        {
            capture.release();
        }
        if let Some(chain) = self.chain.borrow().clone() {
            chain.detach_accessory(node);
        }
        *self.chain.borrow_mut() = None;
        *self.capture.borrow_mut() = None;
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessory_scope_follows_the_portal_lifetime() {
        let context = super::super::PlatformInputContext::new();
        context.with_current(|| {
            let trigger = Node::new();
            let portal = Node::new();
            let chain = FocusChainHolder::current().unwrap();
            let resource = Rc::new(FocusAccessoryResource::new(&trigger));
            portal.add_resource(resource.clone());
            // A resource attaches when its node mounts, exactly as `addResource`
            // does on a live tree; attaching without a mount would leave the
            // chain unaware of the portal.
            resource.mount(&portal);
            assert_eq!(chain.accessory_roots(), vec![portal.clone()]);
            resource.unmount(&portal);
            assert!(chain.accessory_roots().is_empty());
        });
    }
}
