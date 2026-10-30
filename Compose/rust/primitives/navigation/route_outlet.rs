//! Composition boundary for the selected route.
use cove_core::Binding;
use crate::{AnyView, View, ViewExt};
use std::hash::Hash;
use std::rc::Rc;

pub struct RouteOutlet<Route, Content>
where
    Route: Clone + Hash + Eq + 'static,
    Content: View + 'static,
{
    pub selection: Binding<Route>,
    destination: Rc<dyn Fn(Route) -> Content>,
}

impl<Route, Content> RouteOutlet<Route, Content>
where
    Route: Clone + Hash + Eq + 'static,
    Content: View + 'static,
{
    pub fn new(selection: Binding<Route>, destination: impl Fn(Route) -> Content + 'static) -> Self {
        Self { selection, destination: Rc::new(destination) }
    }
}

impl<Route, Content> View for RouteOutlet<Route, Content>
where
    Route: Clone + Hash + Eq + 'static,
    Content: View + 'static,
{
    fn body(&self) -> AnyView {
        let route = self.selection.get();
        AnyView::new((self.destination)(route.clone()).id(route))
    }
}
