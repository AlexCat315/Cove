use cove_core::scene::ComponentSourceLocation;
use crate::{View,ViewKind,AnyView};
#[derive(Clone)]
pub struct SourceLocatedView{pub content:AnyView,pub location:ComponentSourceLocation}
impl SourceLocatedView{
    #[track_caller]pub fn explicit(view:impl View+'static)->Self{let source=std::panic::Location::caller();Self{content:AnyView::new(view),location:ComponentSourceLocation{file_id:source.file().into(),file_path:source.file().into(),line:source.line(),column:source.column(),explicit:true}}}
    #[track_caller]pub fn expression(view:impl View+'static)->Self{let mut result=Self::explicit(view);result.location.explicit=false;result}
}
impl View for SourceLocatedView{fn kind(&self)->ViewKind<'_>{ViewKind::Source(self)}}
