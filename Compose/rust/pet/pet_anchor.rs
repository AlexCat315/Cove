//! Layout-driven pet perch registration.
use super::PetWorld;
use crate::{AnyView, PrimitiveView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{CompositionLocal, LayoutNode, Node};
use std::rc::Rc;

#[derive(Clone)]
pub struct PetWorldHandle(pub Rc<PetWorld>);
impl PartialEq for PetWorldHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

thread_local! {
    static PET_WORLD_LOCAL: Rc<CompositionLocal<Option<PetWorldHandle>>> =
        Rc::new(CompositionLocal::constant(None));
}

/// Composition-local channel installed by [`super::PetScene`].
pub fn pet_world_local() -> Rc<CompositionLocal<Option<PetWorldHandle>>> {
    PET_WORLD_LOCAL.with(Rc::clone)
}

/// A transparent wrapper that reports its absolute frame after layout.
#[derive(Clone)]
pub struct PetAnchorNode {
    id: String,
    content: AnyView,
}
impl PetAnchorNode {
    pub fn new(id: impl Into<String>, content: AnyView) -> Self {
        Self {
            id: id.into(),
            content,
        }
    }

    fn hook(&self, node: &Node) {
        let id = self.id.clone();
        node.set_layout_did_update(Some(Rc::new(move |node| {
            if let Some(world) = node.composition_value(&pet_world_local()) {
                world.0.update_anchor(id.clone(), node.absolute_frame());
            }
        })));
    }
}
impl PrimitiveView for PetAnchorNode {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        self.hook(&node);
        node
    }
    fn update_node(&self, node: &Node) {
        self.hook(node);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(PetAnchorNode);

/// Mark a view as a perch that the pet can use as a jump destination.
pub trait PetAnchorExt: View + Sized + 'static {
    fn pet_anchor(self, id: impl Into<String>) -> PetAnchorNode {
        PetAnchorNode::new(id, AnyView::new(self))
    }
}
impl<T: View + Sized + 'static> PetAnchorExt for T {}

/// Apply the active pet world to a subtree.
pub fn provide_pet_world<V: View + Sized + 'static>(
    view: V,
    world: Rc<PetWorld>,
) -> crate::ModifiedContent {
    view.composition_local(pet_world_local(), Some(PetWorldHandle(world)))
}
