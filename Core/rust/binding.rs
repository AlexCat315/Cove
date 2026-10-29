//! Translation of CoveUICore/Binding.swift.
use std::sync::Arc;

pub struct Binding<T> { get: Arc<dyn Fn() -> T + Send + Sync>, set: Arc<dyn Fn(T) + Send + Sync> }
impl<T> Clone for Binding<T> { fn clone(&self) -> Self { Self { get: self.get.clone(), set: self.set.clone() } } }
impl<T> Binding<T> {
    pub fn new(get: impl Fn() -> T + Send + Sync + 'static, set: impl Fn(T) + Send + Sync + 'static) -> Self {
        Self { get: Arc::new(get), set: Arc::new(set) }
    }
    pub fn get(&self) -> T { (self.get)() }
    pub fn set(&self, value: T) { (self.set)(value); }
    pub fn constant(value: T) -> Self where T: Clone + Send + Sync + 'static { Self::new(move || value.clone(), |_| {}) }
}
