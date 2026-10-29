//! Translation of CoveUIComposeCore/Layout/LayoutValue.swift.
use cove_core::scene::LayoutNode;
use crate::{ModifiedContent, View, ViewExt, ViewModifier};

/// A per-child value a custom `Layout` reads from its subviews, such as a grid
/// cell's span. Values live with the child's layout node, so children declare
/// them with `.layout_value(...)` and need no wrapper type.
pub trait LayoutValueKey {
    type Value: Clone + PartialEq + 'static;
    fn default_value() -> Self::Value;
}

pub struct LayoutValueStorage;
impl LayoutValueStorage {
    pub fn key<K: LayoutValueKey>() -> String {
        format!("layoutValue.{}", std::any::type_name::<K>())
    }
}

struct LayoutValueModifier<K: LayoutValueKey> {
    value: K::Value,
}
impl<K: LayoutValueKey> ViewModifier for LayoutValueModifier<K> {
    fn apply_layout(&self, layout: &LayoutNode) {
        let key = LayoutValueStorage::key::<K>();
        if layout.attachment::<K::Value>(&key).as_ref() == Some(&self.value) {
            return;
        }
        layout.set_attachment(&key, self.value.clone());
        // The owning custom layout measures through its own callback.
        if let Some(parent) = layout.parent()
            && parent.has_measure_func()
        {
            parent.mark_dirty();
        }
    }
}

pub trait LayoutValueExt: View + Sized + 'static {
    fn layout_value<K: LayoutValueKey>(self, value: K::Value) -> ModifiedContent;
}
impl<T: View + Sized + 'static> LayoutValueExt for T {
    fn layout_value<K: LayoutValueKey>(self, value: K::Value) -> ModifiedContent {
        self.modifier(LayoutValueModifier::<K> { value })
    }
}
