//! Translation of CoveUIScene/ObservableStateTracking.swift.
use super::RecompositionReason;
use crate::state_read_tracking::{ReadScope, ReadScopeId, with_read_scope};
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, Weak};

/// A type-erased, hashable key for `ObservableStateRegistrar`.
#[derive(Clone)]
pub struct StateAccessKey(Arc<dyn KeyValue>);
trait KeyValue: 'static + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn key_eq(&self, other: &dyn Any) -> bool;
    fn key_hash(&self) -> u64;
}
impl<T: Hash + Eq + Send + Sync + 'static> KeyValue for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn key_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|other| self == other)
    }
    fn key_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}
impl StateAccessKey {
    pub fn new<T: Hash + Eq + Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(value))
    }
    pub fn text(value: impl Into<String>) -> Self {
        Self::new(value.into())
    }
}
impl PartialEq for StateAccessKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.key_hash() == other.0.key_hash() && self.0.key_eq(other.0.as_any())
    }
}
impl Eq for StateAccessKey {}
impl PartialOrd for StateAccessKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for StateAccessKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.key_hash().cmp(&other.0.key_hash())
    }
}

pub struct ObservableStateScope {
    pub id: ReadScopeId,
    pub invalidate: Arc<dyn Fn() + Send + Sync>,
    pub invalidate_tracked: Option<Arc<dyn Fn(RecompositionReason) + Send + Sync>>,
}

thread_local! {
    static SCOPE_STACK: RefCell<Vec<Arc<ObservableStateScope>>> = const { RefCell::new(Vec::new()) };
}

pub struct ObservableStateTracking;
impl ObservableStateTracking {
    pub fn current() -> Option<Arc<ObservableStateScope>> {
        SCOPE_STACK.with(|stack| stack.borrow().last().cloned())
    }
    /// Run `body` inside a tracking scope, clearing the previous dependencies.
    pub fn with_scope<R>(
        id: ReadScopeId,
        invalidate: Arc<dyn Fn() + Send + Sync>,
        invalidate_tracked: Option<Arc<dyn Fn(RecompositionReason) + Send + Sync>>,
        body: impl FnOnce() -> R,
    ) -> R {
        let scope = Arc::new(ObservableStateScope {
            id,
            invalidate: invalidate.clone(),
            invalidate_tracked,
        });
        SCOPE_STACK.with(|stack| stack.borrow_mut().push(scope.clone()));
        let tracked = scope.invalidate_tracked.clone();
        let read_scope = ReadScope::with_id(id, move |field: Option<&str>| match &tracked {
            Some(tracked) => {
                tracked(RecompositionReason::new("state").with_detail(field.unwrap_or_default()))
            }
            None => invalidate(),
        });
        struct Pop;
        impl Drop for Pop {
            fn drop(&mut self) {
                SCOPE_STACK.with(|stack| {
                    stack.borrow_mut().pop();
                });
            }
        }
        let _pop = Pop;
        with_read_scope(&read_scope, body)
    }
    /// Track a single evaluation boundary.
    pub fn observe_reads<R>(body: impl FnOnce() -> R) -> R {
        body()
    }
    pub fn remove_scope(id: ReadScopeId) {
        crate::state_read_tracking::remove_read_scope(id);
    }
}

#[derive(Clone)]
struct Observer {
    id: ReadScopeId,
    invalidate: Arc<dyn Fn() + Send + Sync>,
    invalidate_tracked: Option<Arc<dyn Fn(RecompositionReason) + Send + Sync>>,
}

#[derive(Default)]
struct RegistrarInner {
    observers_by_key: Mutex<BTreeMap<StateAccessKey, BTreeMap<ReadScopeId, Observer>>>,
    keys_by_scope: Mutex<BTreeMap<ReadScopeId, BTreeSet<StateAccessKey>>>,
}
impl RegistrarInner {
    fn remove_dependencies(&self, scope: ReadScopeId) {
        let Some(keys) = self
            .keys_by_scope
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&scope)
        else {
            return;
        };
        let mut observers = self
            .observers_by_key
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for key in keys {
            if let Some(keyed) = observers.get_mut(&key) {
                keyed.remove(&scope);
                if keyed.is_empty() {
                    observers.remove(&key);
                }
            }
        }
    }
}

/// Per-owner dependency table: `access` records that the current scope read a
/// key, `invalidate` wakes every scope that read it.
///
/// Cloning shares the dependency table and keeps the same owner identity: a
/// clone observes the same reads and wakes the same scopes, which is what a
/// value-semantics handle over one registrar means.
#[derive(Clone, Default)]
pub struct ObservableStateRegistrar {
    inner: Arc<RegistrarInner>,
    identity: u64,
}
impl ObservableStateRegistrar {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RegistrarInner::default()),
            identity: crate::state_read_tracking::allocate_tracking_owner(),
        }
    }
    pub fn access(&self, key: StateAccessKey) {
        let Some(scope) = ObservableStateTracking::current() else {
            return;
        };
        let inner: Weak<RegistrarInner> = Arc::downgrade(&self.inner);
        crate::state_read_tracking::register_read_cleaner(
            self.identity,
            scope.id,
            move |scope_id| {
                if let Some(inner) = inner.upgrade() {
                    inner.remove_dependencies(scope_id);
                }
            },
        );
        self.inner
            .observers_by_key
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entry(key.clone())
            .or_default()
            .insert(
                scope.id,
                Observer {
                    id: scope.id,
                    invalidate: scope.invalidate.clone(),
                    invalidate_tracked: scope.invalidate_tracked.clone(),
                },
            );
        self.inner
            .keys_by_scope
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entry(scope.id)
            .or_default()
            .insert(key);
    }
    pub fn invalidate(&self, key: &StateAccessKey) {
        let observers: Vec<Observer> = self
            .inner
            .observers_by_key
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)
            .map(|keyed| keyed.values().cloned().collect())
            .unwrap_or_default();
        for observer in observers {
            match &observer.invalidate_tracked {
                Some(tracked) => tracked(RecompositionReason::new("observable")),
                None => (observer.invalidate)(),
            }
        }
    }
    pub fn invalidate_all(&self) {
        let mut by_scope: BTreeMap<ReadScopeId, Observer> = BTreeMap::new();
        for keyed in self
            .inner
            .observers_by_key
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
        {
            for observer in keyed.values() {
                by_scope.insert(observer.id, observer.clone());
            }
        }
        for observer in by_scope.into_values() {
            match &observer.invalidate_tracked {
                Some(tracked) => {
                    tracked(RecompositionReason::new("observable").with_detail("all fields"))
                }
                None => (observer.invalidate)(),
            }
        }
    }
    pub fn remove_dependencies(&self, scope: ReadScopeId) {
        self.inner.remove_dependencies(scope);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn access_then_invalidate_wakes_the_reading_scope() {
        let registrar = ObservableStateRegistrar::new();
        let hits = Arc::new(AtomicU32::new(0));
        let counter = hits.clone();
        ObservableStateTracking::with_scope(
            ReadScopeId(1),
            Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }),
            None,
            || registrar.access(StateAccessKey::text("focus")),
        );
        registrar.invalidate(&StateAccessKey::text("focus"));
        assert_eq!(hits.load(Ordering::Relaxed), 1);
        registrar.invalidate(&StateAccessKey::text("other"));
        assert_eq!(hits.load(Ordering::Relaxed), 1);
        registrar.invalidate_all();
        assert_eq!(hits.load(Ordering::Relaxed), 2);
        registrar.remove_dependencies(ReadScopeId(1));
        registrar.invalidate_all();
        assert_eq!(hits.load(Ordering::Relaxed), 2);
    }
}
