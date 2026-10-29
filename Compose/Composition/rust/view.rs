use std::any::Any;
use crate::{AnyView,PrimitiveView,source_location::SourceLocatedView,identified_view::IdentifiedView,modified_content_materialise::AnyModifiedContent,DynamicField};
pub trait AsAny{fn as_any(&self)->&dyn Any;}
impl<T:Any> AsAny for T{fn as_any(&self)->&dyn Any{self}}
pub enum ViewKind<'a>{Component,Empty,Primitive(&'a dyn PrimitiveView),Structural(Vec<AnyView>),Modified(&'a dyn AnyModifiedContent),Identified(&'a IdentifiedView),Source(&'a SourceLocatedView)}
/// A value description of UI. Primitive and structural views are handled through kind().
pub trait View:AsAny{
    fn body(&self)->AnyView{panic!("this view is materialized directly through ViewGraph")}
    fn kind(&self)->ViewKind<'_>{ViewKind::Component}
    fn tag(&self)->&'static str{std::any::type_name::<Self>()}
    fn dynamic_properties(&self)->Vec<DynamicField>{Vec::new()}
    fn same_inputs(&self,_other:&dyn View)->bool{false}
    fn precise_state_reads(&self)->bool{false}
}
pub trait ViewExt:View+Sized+'static{
    fn erased(self)->AnyView{AnyView::new(self)}
    fn id<K:std::hash::Hash+Eq+'static>(self,key:K)->IdentifiedView{IdentifiedView::new(self,key)}
    fn modifier<M:crate::ViewModifier+'static>(self,modifier:M)->crate::ModifiedContent{crate::ModifiedContent::new(AnyView::new(self),modifier)}
    /// Provide `value` for `local` to every descendant of this view.
    fn composition_local<Value:Clone+PartialEq+'static>(self,local:std::rc::Rc<cove_core::CompositionLocal<Value>>,value:Value)->crate::ModifiedContent{
        self.modifier(crate::ProvideCompositionLocalModifier::new(local,value))
    }
    #[track_caller]fn source_location(self)->SourceLocatedView{SourceLocatedView::explicit(self)}
}
impl<T:View+Sized+'static>ViewExt for T{}
