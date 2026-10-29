use cove_core::scene::NodeKey;
use crate::{View,ViewKind,AnyView};
#[derive(Clone)]
pub struct IdentifiedView{pub content:AnyView,pub id:NodeKey}
impl IdentifiedView{pub fn new(view:impl View+'static,key:impl std::hash::Hash+Eq+'static)->Self{Self{content:AnyView::new(view),id:NodeKey::new(key)}}}
impl View for IdentifiedView{fn kind(&self)->ViewKind<'_>{ViewKind::Identified(self)}}
