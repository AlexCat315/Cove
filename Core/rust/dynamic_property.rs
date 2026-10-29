//! Translation of CoveUICore/DynamicProperty.swift.
pub trait DynamicProperty {}
impl<T> DynamicProperty for crate::State<T> {}
impl<T> DynamicProperty for crate::Binding<T> {}
