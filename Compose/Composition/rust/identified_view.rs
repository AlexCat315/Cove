use cove_core::scene::NodeKey;
use crate::{View,ViewKind,AnyView};
#[derive(Clone)]
pub struct IdentifiedView{pub content:AnyView,pub id:NodeKey}
impl IdentifiedView{pub fn new(view:impl View+'static,key:impl std::hash::Hash+Eq+'static)->Self{Self{content:AnyView::new(view),id:NodeKey::new(key)}}
    /// Stacks `modifier` on the identified view, mirroring [`crate::AnyView::with_modifier`].
    pub fn with_modifier(self, modifier: impl crate::ViewModifier + 'static) -> crate::ModifiedContent {
        crate::ModifiedContent::new(crate::AnyView::new(self), modifier)
    }
}
impl View for IdentifiedView{fn kind(&self)->ViewKind<'_>{ViewKind::Identified(self)}}
