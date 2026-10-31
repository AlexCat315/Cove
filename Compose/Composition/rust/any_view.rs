use crate::{DynamicField, View, ViewKind};
use std::rc::Rc;
#[derive(Clone)]
pub struct AnyView {
    pub storage: Rc<dyn View>,
}
impl AnyView {
    pub fn new<V: View + 'static>(view: V) -> Self {
        if let Some(view) = view.as_any().downcast_ref::<Self>() {
            return view.clone();
        }
        Self {
            storage: Rc::new(view),
        }
    }
    pub fn from_rc(view: Rc<dyn View>) -> Self {
        if let Some(view) = view.as_any().downcast_ref::<Self>() {
            return view.clone();
        }
        Self { storage: view }
    }
    /// Stacks `modifier` on this erased view.
    ///
    /// `AnyView` is the type builders hand back, so this is the one place that
    /// must accept a modifier without the caller re-importing [`crate::ViewExt`]
    /// or wrapping in another `AnyView`. `ViewExt::modifier` cannot serve here
    /// because callers hold an already-erased value.
    pub fn with_modifier(
        self,
        modifier: impl crate::ViewModifier + 'static,
    ) -> crate::ModifiedContent {
        crate::ModifiedContent::new(self, modifier)
    }
}
impl View for AnyView {
    fn body(&self) -> AnyView {
        self.storage.body()
    }
    fn kind(&self) -> ViewKind<'_> {
        self.storage.kind()
    }
    fn tag(&self) -> &'static str {
        self.storage.tag()
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        self.storage.dynamic_properties()
    }
    fn same_inputs(&self, other: &dyn View) -> bool {
        self.storage.same_inputs(other)
    }
    fn precise_state_reads(&self) -> bool {
        self.storage.precise_state_reads()
    }
}
