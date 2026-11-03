//! Translation of CoveUICore/State.swift and the storage half of StateReadTracking.swift.
use crate::state_read_tracking::{
    current_read_scope, is_read_scope_installed, register_read_cleaner,
};
use crate::{Binding, ReadScope, ReadScopeId, SnapshotValue};
use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

type Observer = Arc<dyn Fn(u64) + Send + Sync>;
type Invalidator = Arc<dyn Fn(Option<&str>) + Send + Sync>;
type Summary<T> = Arc<dyn Fn(&T) -> String + Send + Sync>;
/// Scene-side observers are not `Send`: composition and node trees live on the
/// UI thread. They are registered in thread-local storage keyed by state id so
/// `Storage<T>` stays `Send + Sync` for cross-thread value access.
type LocalObserver = std::rc::Rc<dyn Fn(u64)>;
use std::cell::RefCell;
thread_local! {
    static LOCAL_OBSERVERS: RefCell<BTreeMap<u64, BTreeMap<u64, LocalObserver>>> = const { RefCell::new(BTreeMap::new()) };
}
trait DictionaryKey: Any + Send + Sync {
    fn key_type(&self) -> TypeId;
    fn key_hash(&self) -> u64;
    fn key_eq(&self, other: &dyn DictionaryKey) -> bool;
    fn as_any(&self) -> &dyn Any;
}
struct TypedKey<K>(K);
impl<K: Hash + Eq + Send + Sync + 'static> DictionaryKey for TypedKey<K> {
    fn key_type(&self) -> TypeId {
        TypeId::of::<K>()
    }
    fn key_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
    fn key_eq(&self, other: &dyn DictionaryKey) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.0 == other.0)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
#[derive(Clone)]
enum Selector {
    Field,
    Array(usize),
    Dictionary(Arc<dyn DictionaryKey>),
}
impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Field, Self::Field) => true,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Dictionary(a), Self::Dictionary(b)) => a.key_eq(b.as_ref()),
            _ => false,
        }
    }
}
impl Eq for Selector {}
impl Hash for Selector {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Field => {}
            Self::Array(i) => i.hash(state),
            Self::Dictionary(key) => {
                key.key_type().hash(state);
                key.key_hash().hash(state);
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
struct ReadKey {
    name: String,
    selector: Selector,
}
struct FieldRead<T> {
    name: String,
    unchanged: Arc<dyn Fn(&T) -> bool + Send + Sync>,
}
struct Reader<T> {
    invalidate: Invalidator,
    whole: bool,
    fields: HashMap<ReadKey, FieldRead<T>>,
}
struct Tracking<T> {
    owner: Option<ReadScope>,
    precise: bool,
    readers: HashMap<ReadScopeId, Reader<T>>,
}
impl<T> Default for Tracking<T> {
    fn default() -> Self {
        Self {
            owner: None,
            precise: false,
            readers: HashMap::new(),
        }
    }
}
struct Storage<T> {
    id: u64,
    value: Mutex<(T, u64)>,
    observers: Mutex<BTreeMap<u64, Observer>>,
    next_id: AtomicU64,
    tracking: Mutex<Tracking<T>>,
    summary: Mutex<Option<Summary<T>>>,
}

pub struct State<T>(Arc<Storage<T>>);
impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: Send + 'static> State<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(Storage {
            id: crate::state_read_tracking::allocate_tracking_owner(),
            value: Mutex::new((value, 0)),
            observers: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
            tracking: Mutex::new(Tracking::default()),
            summary: Mutex::new(None),
        }))
    }
    pub fn id(&self) -> u64 {
        self.0.id
    }
    pub fn value_type(&self) -> &'static str {
        std::any::type_name::<T>()
    }
    pub fn expose(&self, summary: impl Fn(&T) -> String + Send + Sync + 'static) {
        *self.0.summary.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(summary));
    }
    pub fn expose_debug(&self)
    where
        T: std::fmt::Debug,
    {
        self.expose(|value| format!("{value:?}"));
    }
    pub fn inspection_summary(&self) -> Option<String> {
        let summary = self
            .0
            .summary
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()?;
        Some(summary(
            &self.0.value.lock().unwrap_or_else(|e| e.into_inner()).0,
        ))
    }
    pub fn set_read_tracking_owner(&self, scope: ReadScope, precise: bool) {
        let mut tracking = self.0.tracking.lock().unwrap_or_else(|e| e.into_inner());
        tracking.owner = Some(scope);
        tracking.precise = precise;
    }
    pub fn clear_read_tracking_owner(&self, scope: ReadScopeId) {
        let mut tracking = self.0.tracking.lock().unwrap_or_else(|e| e.into_inner());
        if tracking
            .owner
            .as_ref()
            .is_some_and(|owner| owner.id == scope)
        {
            tracking.owner = None;
            tracking.precise = false;
        }
        tracking.readers.remove(&scope);
    }
    fn track(&self, key: Option<ReadKey>, field: Option<FieldRead<T>>) {
        self.track_access(key, field, false);
    }
    fn track_access(&self, key: Option<ReadKey>, field: Option<FieldRead<T>>, binding: bool) {
        let (scope, first) = {
            let mut tracking = self.0.tracking.lock().unwrap_or_else(|e| e.into_inner());
            if !tracking.precise && !binding {
                return;
            }
            let Some(scope) = current_read_scope(tracking.owner.as_ref()) else {
                return;
            };
            let first = !tracking.readers.contains_key(&scope.id);
            let reader = tracking.readers.entry(scope.id).or_insert_with(|| Reader {
                invalidate: scope.invalidate.clone(),
                whole: false,
                fields: HashMap::new(),
            });
            match (key, field) {
                (Some(key), Some(field)) => {
                    reader.fields.insert(key, field);
                }
                _ => reader.whole = true,
            }
            (scope, first)
        };
        if first {
            let storage = Arc::downgrade(&self.0);
            register_read_cleaner(self.0.id, scope.id, move |scope| {
                if let Some(storage) = storage.upgrade() {
                    storage
                        .tracking
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .readers
                        .remove(&scope);
                }
            });
        }
    }
    pub fn snapshot(&self) -> (T, u64)
    where
        T: Clone,
    {
        let value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
        self.track(None, None);
        value.clone()
    }
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.snapshot().0
    }
    pub fn binding(&self) -> Binding<T>
    where
        T: Clone,
    {
        let read = self.clone();
        let write = self.clone();
        Binding::new(move || {
            let value = read.0.value.lock().unwrap_or_else(|e| e.into_inner());
            read.track_access(None, None, true);
            value.0.clone()
        }, move |value| write.set(value))
    }
    /// Projections are pure and must not recursively lock this state.
    pub fn read_field<F: SnapshotValue>(
        &self,
        name: &str,
        select: impl Fn(&T) -> F + Send + Sync + 'static,
    ) -> F {
        self.read_selected(
            ReadKey {
                name: name.into(),
                selector: Selector::Field,
            },
            select,
        )
    }
    pub fn read_signal<F: SnapshotValue>(
        &self,
        name: &str,
        select: impl Fn(&T) -> F + Send + Sync + 'static,
    ) -> F {
        self.read_field(name, select)
    }
    fn read_selected<F: SnapshotValue>(
        &self,
        key: ReadKey,
        select: impl Fn(&T) -> F + Send + Sync + 'static,
    ) -> F {
        let value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
        let observed = select(&value.0);
        let saved = observed.clone();
        let name = key.name.clone();
        self.track(
            Some(key),
            Some(FieldRead {
                name,
                unchanged: Arc::new(move |new| saved == select(new)),
            }),
        );
        observed
    }
    pub fn read_opaque<F>(&self, name: &str, select: impl FnOnce(&T) -> F) -> F {
        let value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
        let observed = select(&value.0);
        self.track(
            Some(ReadKey {
                name: name.into(),
                selector: Selector::Field,
            }),
            Some(FieldRead {
                name: name.into(),
                unchanged: Arc::new(|_| false),
            }),
        );
        observed
    }
    pub fn read_array_element<F: SnapshotValue>(
        &self,
        name: &str,
        index: usize,
        select: impl Fn(&T) -> &[F] + Send + Sync + 'static,
    ) -> F {
        let value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
        let observed = select(&value.0)[index].clone();
        let saved = observed.clone();
        self.track(
            Some(ReadKey {
                name: name.into(),
                selector: Selector::Array(index),
            }),
            Some(FieldRead {
                name: name.into(),
                unchanged: Arc::new(move |new| {
                    select(new).get(index).is_some_and(|value| value == &saved)
                }),
            }),
        );
        observed
    }
    pub fn read_map_value<K, F>(
        &self,
        name: &str,
        key: K,
        select: impl Fn(&T) -> &HashMap<K, F> + Send + Sync + 'static,
    ) -> Option<F>
    where
        K: Hash + Eq + Clone + Send + Sync + 'static,
        F: SnapshotValue,
    {
        let value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
        let observed = select(&value.0).get(&key).cloned();
        let saved = observed.clone();
        let selector = Selector::Dictionary(Arc::new(TypedKey(key.clone())));
        self.track(
            Some(ReadKey {
                name: name.into(),
                selector,
            }),
            Some(FieldRead {
                name: name.into(),
                unchanged: Arc::new(move |new| select(new).get(&key).cloned() == saved),
            }),
        );
        observed
    }
    /// Observers and invalidation callbacks run outside both value and reader locks.
    pub fn update(&self, change: impl FnOnce(&mut T)) {
        let (revision, invalidations) = {
            let mut value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
            change(&mut value.0);
            value.1 = value.1.checked_add(1).expect("state revision exhausted");
            let tracking = self.0.tracking.lock().unwrap_or_else(|e| e.into_inner());
            let mut pending: Vec<(Invalidator, Option<String>)> = Vec::new();
            for reader in tracking.readers.values() {
                if reader.whole {
                    pending.push((reader.invalidate.clone(), None));
                } else if let Some(field) = reader
                    .fields
                    .values()
                    .find(|field| !(field.unchanged)(&value.0))
                {
                    pending.push((reader.invalidate.clone(), Some(field.name.clone())));
                }
            }
            if (!tracking.precise
                || tracking
                    .owner
                    .as_ref()
                    .is_none_or(|owner| !is_read_scope_installed(owner.id)))
                && let Some(owner) = &tracking.owner
            {
                pending.push((owner.invalidate.clone(), None));
            }
            (value.1, pending)
        };
        let callbacks: Vec<_> = self
            .0
            .observers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        let local_callbacks: Vec<LocalObserver> = LOCAL_OBSERVERS.with(|observers| {
            observers
                .borrow()
                .get(&self.0.id)
                .map(|entries| entries.values().cloned().collect())
                .unwrap_or_default()
        });
        for (invalidate, field) in invalidations {
            invalidate(field.as_deref());
        }
        for callback in callbacks {
            callback(revision);
        }
        for callback in local_callbacks {
            callback(revision);
        }
    }
    pub fn set(&self, value: T) {
        self.update(|current| *current = value);
    }
    pub fn copy_runtime_value(&self, other: &Self)
    where
        T: Clone,
    {
        if Arc::ptr_eq(&self.0, &other.0) {
            return;
        }
        let value = other
            .0
            .value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .0
            .clone();
        self.0.value.lock().unwrap_or_else(|e| e.into_inner()).0 = value;
    }
    pub fn subscribe(&self, observer: impl Fn(u64) + Send + Sync + 'static) -> Subscription<T> {
        let id = self.0.next_id.fetch_add(1, Ordering::Relaxed);
        self.0
            .observers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Arc::new(observer));
        Subscription {
            storage: Arc::downgrade(&self.0),
            id,
        }
    }
    /// Observer confined to the current thread, for UI scopes that cannot be
    /// `Send` (`Rc`-based composition and node graph owners).
    pub fn subscribe_local(&self, observer: impl Fn(u64) + 'static) -> LocalSubscription {
        let id = self.0.next_id.fetch_add(1, Ordering::Relaxed);
        LOCAL_OBSERVERS.with(|observers| {
            observers
                .borrow_mut()
                .entry(self.0.id)
                .or_default()
                .insert(id, std::rc::Rc::new(observer));
        });
        LocalSubscription {
            state_id: self.0.id,
            observer_id: id,
        }
    }
}

/// Dropping the token releases its thread-local observer.
pub struct LocalSubscription {
    state_id: u64,
    observer_id: u64,
}
impl Drop for LocalSubscription {
    fn drop(&mut self) {
        LOCAL_OBSERVERS.with(|observers| {
            if let Some(entries) = observers.borrow_mut().get_mut(&self.state_id) {
                entries.remove(&self.observer_id);
            }
        });
    }
}
/// Dropping the token releases its observer without keeping the state alive.
pub struct Subscription<T> {
    storage: Weak<Storage<T>>,
    id: u64,
}
impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.upgrade() {
            storage
                .observers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_updates_and_reentrant_observer() {
        let state = State::new(0);
        let weak = Arc::downgrade(&state.0);
        let calls = Arc::new(AtomicU64::new(0));
        let seen = calls.clone();
        let subscription = state.subscribe(move |_| {
            let _value = State(weak.upgrade().unwrap()).get();
            seen.fetch_add(1, Ordering::Relaxed);
        });
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let state = state.clone();
                scope.spawn(move || {
                    for _ in 0..1000 {
                        state.update(|v| *v += 1);
                    }
                });
            }
        });
        assert_eq!(state.snapshot(), (4000, 4000));
        assert_eq!(calls.load(Ordering::Relaxed), 4000);
        drop(subscription);
        state.set(0);
        assert_eq!(calls.load(Ordering::Relaxed), 4000);
    }
}

#[cfg(test)]
mod binding_read_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn binding_reads_subscribe_without_a_dynamic_property_owner() {
        let value = State::new(1);
        let binding = value.binding();
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let scope = ReadScope::new(move |_| { count.fetch_add(1, Ordering::SeqCst); });
        crate::state_read_tracking::with_read_scope(&scope, || assert_eq!(binding.get(), 1));
        value.set(2);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        crate::state_read_tracking::with_read_scope(&scope, || {});
        value.set(3);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        crate::state_read_tracking::remove_read_scope(scope.id);
    }
}
