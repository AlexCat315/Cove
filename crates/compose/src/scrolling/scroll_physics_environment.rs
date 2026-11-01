//! CompositionLocal carrying the overscroll physics policy.
use crate::{ModifiedContent, ProvideCompositionLocalModifier, View, ViewExt};
use cove_core::scene::{CompositionLocal, Node, ScrollPhysics};
use std::rc::Rc;

pub struct ScrollPhysicsEnvironment;
impl ScrollPhysicsEnvironment {
    pub fn key() -> Rc<CompositionLocal<ScrollPhysics>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<ScrollPhysics>> = Rc::new(CompositionLocal::constant(ScrollPhysics::default()));
        }
        KEY.with(Rc::clone)
    }
}

pub trait ScrollPhysicsExt: View + Sized + 'static {
    fn scroll_physics(self, physics: ScrollPhysics) -> ModifiedContent {
        self.modifier(ProvideCompositionLocalModifier::new(
            ScrollPhysicsEnvironment::key(),
            physics,
        ))
    }
}
impl<T: View + Sized + 'static> ScrollPhysicsExt for T {}

pub trait NodeScrollPhysicsExt {
    fn scroll_physics(&self) -> ScrollPhysics;
}
impl NodeScrollPhysicsExt for Node {
    fn scroll_physics(&self) -> ScrollPhysics {
        self.composition_value(&ScrollPhysicsEnvironment::key())
    }
}
