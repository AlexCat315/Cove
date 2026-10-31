use crate::{AnyView, View};
use std::rc::Rc;
#[derive(Clone)]
pub struct MemoizedView<K> {
    pub key: K,
    content: Rc<dyn Fn() -> AnyView>,
}
impl<K> MemoizedView<K> {
    pub fn new<V: View + 'static>(key: K, content: impl Fn() -> V + 'static) -> Self {
        Self {
            key,
            content: Rc::new(move || AnyView::new(content())),
        }
    }
}
impl<K: PartialEq + 'static> View for MemoizedView<K> {
    fn body(&self) -> AnyView {
        (self.content)()
    }
    fn same_inputs(&self, other: &dyn View) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.key == other.key)
    }
}
