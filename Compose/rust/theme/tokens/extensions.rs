//! Translation of Theme/Tokens/Extensions.swift.
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Applications add design tokens without modifying CoveUI's closed set of
/// built-in roles.
pub trait ThemeExtension: 'static {
    fn default_value() -> Self
    where
        Self: Sized;
}

/// Value-stable extension storage. Clones share an identity until either side
/// mutates, mirroring Swift's copy-on-write dictionary semantics for equality.
pub struct ThemeExtensions {
    values: HashMap<TypeId, Rc<dyn Any>>,
    identity: Rc<Cell<u64>>,
}
impl Default for ThemeExtensions {
    fn default() -> Self {
        Self { values: HashMap::new(), identity: Rc::new(Cell::new(next_identity())) }
    }
}
impl Clone for ThemeExtensions {
    fn clone(&self) -> Self {
        Self { values: self.values.clone(), identity: Rc::new(Cell::new(self.identity.get())) }
    }
}
impl PartialEq for ThemeExtensions {
    fn eq(&self, other: &Self) -> bool {
        self.identity.get() == other.identity.get()
    }
}
impl Eq for ThemeExtensions {}
impl std::fmt::Debug for ThemeExtensions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ThemeExtensions").field("count", &self.values.len()).finish()
    }
}
fn next_identity() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
impl ThemeExtensions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get<Value: ThemeExtension + Clone>(&self) -> Value {
        self.values
            .get(&TypeId::of::<Value>())
            .and_then(|value| value.downcast_ref::<Value>())
            .cloned()
            .unwrap_or_else(Value::default_value)
    }
    pub fn set<Value: ThemeExtension>(&mut self, value: Value) {
        self.identity.set(next_identity());
        self.values.insert(TypeId::of::<Value>(), Rc::new(value));
    }
    pub fn remove<Value: ThemeExtension>(&mut self) {
        self.identity.set(next_identity());
        self.values.remove(&TypeId::of::<Value>());
    }
}
