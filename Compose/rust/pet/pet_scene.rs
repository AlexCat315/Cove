//! Full-scene assembly for content and its software-pet overlay.
use super::{pet_anchor::provide_pet_world, PetOverlay, PetWorld};
use crate::{AnyView, TupleView, View};
use crate::primitives::layout::ZStack;
use crate::Alignment;
use std::rc::Rc;

pub struct PetScene {
    world: Rc<PetWorld>,
    content: AnyView,
}
impl PetScene {
    pub fn new<Content: View + 'static>(world: Rc<PetWorld>, content: Content) -> Self {
        Self { world, content: AnyView::new(content) }
    }
}
impl View for PetScene {
    fn body(&self) -> AnyView {
        let layers = TupleView::new(vec![
            self.content.clone(),
            AnyView::new(PetOverlay::new(self.world.clone())),
        ]);
        let stack = ZStack::new(Alignment::TopLeading, AnyView::new(layers));
        AnyView::new(provide_pet_world(stack, self.world.clone()))
    }
}
