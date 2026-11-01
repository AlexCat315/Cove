use crate::{
    AnyView, DynamicField, PrimitiveView, Scope, identified_view::IdentifiedView,
    modified_content_materialise::AnyModifiedContent, source_location::SourceLocatedView,
};
use std::any::Any;
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
}
impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
pub enum ViewKind<'a> {
    Component,
    Empty,
    Primitive(&'a dyn PrimitiveView),
    Structural(Vec<AnyView>),
    Modified(&'a dyn AnyModifiedContent),
    Identified(&'a IdentifiedView),
    Source(&'a SourceLocatedView),
}
/// A value description of UI. Primitive and structural views are handled through kind().
///
/// `body` receives the view's composition [`Scope`]: state created with
/// `cx.state` lives as long as this view keeps its place in the tree, and every
/// signal the body reads subscribes the scope to recomposition.
pub trait View: AsAny {
    fn body(&self, _cx: &Scope) -> AnyView {
        panic!("this view is materialized directly through ViewGraph")
    }
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Component
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        Vec::new()
    }
    fn same_inputs(&self, _other: &dyn View) -> bool {
        false
    }
    fn precise_state_reads(&self) -> bool {
        false
    }
}
pub trait ViewExt: View + Sized + 'static {
    fn erased(self) -> AnyView {
        AnyView::new(self)
    }
    fn id<K: std::hash::Hash + Eq + 'static>(self, key: K) -> IdentifiedView {
        IdentifiedView::new(self, key)
    }
    fn modifier<M: crate::ViewModifier + 'static>(self, modifier: M) -> crate::ModifiedContent {
        crate::ModifiedContent::new(AnyView::new(self), modifier)
    }
    /// Provide `value` for `local` to every descendant of this view.
    fn composition_local<Value: Clone + PartialEq + 'static>(
        self,
        local: std::rc::Rc<cove_core::CompositionLocal<Value>>,
        value: Value,
    ) -> crate::ModifiedContent {
        self.modifier(crate::ProvideCompositionLocalModifier::new(local, value))
    }
    #[track_caller]
    fn source_location(self) -> SourceLocatedView {
        SourceLocatedView::explicit(self)
    }
}
impl<T: View + Sized + 'static> ViewExt for T {}
