use crate::{View,ViewKind,AnyView};
#[derive(Clone,Copy,Default)]
pub struct EmptyView;
impl View for EmptyView{fn kind(&self)->ViewKind<'_>{ViewKind::Empty}}
#[derive(Clone,Default)]
pub struct TupleView{pub value:Vec<AnyView>}
impl TupleView{pub fn new(value:Vec<AnyView>)->Self{Self{value}}}
impl View for TupleView{fn kind(&self)->ViewKind<'_>{ViewKind::Structural(self.value.clone())}}
pub enum ConditionalContent<A:View,B:View>{First(A),Second(B)}
impl<A:View+'static,B:View+'static>View for ConditionalContent<A,B>where A:Clone,B:Clone{fn kind(&self)->ViewKind<'_>{ViewKind::Structural(match self{Self::First(v)=>vec![AnyView::new(v.clone())],Self::Second(v)=>vec![AnyView::new(v.clone())]})}}
