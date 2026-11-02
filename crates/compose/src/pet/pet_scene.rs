//! Full-scene assembly for content and its software-pet overlay.
use super::{PetOverlay, PetWorld, pet_anchor::provide_pet_world};
use crate::Alignment;
use crate::Scope;
use crate::primitives::layout::ZStack;
use crate::{AnyView, TupleView, View};
use std::rc::Rc;

pub struct PetScene {
    world: Rc<PetWorld>,
    content: AnyView,
}
impl PetScene {
    pub fn new<Content: View + 'static>(world: Rc<PetWorld>, content: Content) -> Self {
        Self {
            world,
            content: AnyView::new(content),
        }
    }
}
impl View for PetScene {
    fn body(&self, _cx: &Scope) -> AnyView {
        let layers = TupleView::new(vec![
            self.content.clone(),
            AnyView::new(PetOverlay::new(self.world.clone())),
        ]);
        let stack = ZStack::new(AnyView::new(layers)).alignment(Alignment::TopLeading);
        AnyView::new(provide_pet_world(stack, self.world.clone()))
    }
}
