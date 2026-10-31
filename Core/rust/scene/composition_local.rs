//! Translation of CoveUIScene/CompositionLocal.swift.
use std::sync::atomic::{AtomicU64, Ordering};

/// Stable lookup key for a `CompositionLocal` declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompositionLocalKey(pub u64);

/// A typed key for passing values implicitly down the composition tree.
///
/// Declarations are shared (`Rc`/`Arc`), so each one keeps a stable key for the
/// lifetime of the process. Consumers walk the `Node` parent chain until a
/// provider pushed for the same key is found, then fall back to `default_value`.
pub struct CompositionLocal<Value> {
    key: CompositionLocalKey,
    default_value: Box<dyn Fn() -> Value>,
}
impl<Value> CompositionLocal<Value> {
    pub fn new(default_value: impl Fn() -> Value + 'static) -> Self {
        static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
        Self {
            key: CompositionLocalKey(NEXT_KEY.fetch_add(1, Ordering::Relaxed)),
            default_value: Box::new(default_value),
        }
    }
    /// A declaration whose default is a cheaply cloned value.
    pub fn constant(value: Value) -> Self
    where
        Value: Clone + 'static,
    {
        Self::new(move || value.clone())
    }
    /// The value used when no ancestor provider is found.
    pub fn default_value(&self) -> Value {
        (self.default_value)()
    }
    pub fn key(&self) -> CompositionLocalKey {
        self.key
    }
    /// Backwards-compatible accessor — equivalent to `default_value`.
    pub fn current_value(&self) -> Value {
        (self.default_value)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declarations_have_distinct_keys_and_lazy_defaults() {
        let accent = CompositionLocal::constant(7u32);
        let other = CompositionLocal::constant(7u32);
        assert_ne!(accent.key(), other.key());
        assert_eq!(accent.default_value(), 7);
        let calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = calls.clone();
        let lazy = CompositionLocal::new(move || {
            counter.set(counter.get() + 1);
            counter.get()
        });
        assert_eq!(calls.get(), 0);
        assert_eq!(lazy.current_value(), 1);
        assert_eq!(lazy.default_value(), 2);
    }
}
