//! Single-threaded reactive runtime.
//!
//! A [`Signal`] is a `Copy` handle into a per-thread arena. Reading it inside a
//! running [`Observer`] subscribes that observer; writing it invalidates every
//! subscriber. Signals belong to an [`Owner`] and are disposed with it, so a
//! composition scope owns its state instead of the view value carrying it.
//!
//! Nothing here is `Send`: UI state lives on the scene thread. Other threads
//! write through a [`SignalSender`], which posts the update to the scene
//! thread's inbox and wakes it.
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SlotKey {
    index: u32,
    generation: u32,
}

/// Identity of a reading context (a composition scope, an effect).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObserverId(u64);

struct Slot {
    generation: u32,
    value: Option<Rc<dyn Any>>,
    subscribers: Vec<ObserverId>,
}

struct ObserverEntry {
    invalidate: Rc<dyn Fn()>,
    sources: Vec<SlotKey>,
}

type Posted = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct Runtime {
    slots: RefCell<Vec<Slot>>,
    free: RefCell<Vec<u32>>,
    observers: RefCell<HashMap<ObserverId, ObserverEntry>>,
    stack: RefCell<Vec<Option<ObserverId>>>,
    owners: RefCell<Vec<Rc<OwnerInner>>>,
    next_observer: Cell<u64>,
    batch_depth: Cell<u32>,
    pending: RefCell<Vec<ObserverId>>,
    inbox: Arc<Mutex<Vec<Posted>>>,
}

thread_local! {
    static RUNTIME: Runtime = Runtime::default();
}

impl Runtime {
    fn insert(&self, value: Rc<dyn Any>) -> SlotKey {
        let mut slots = self.slots.borrow_mut();
        if let Some(index) = self.free.borrow_mut().pop() {
            let slot = &mut slots[index as usize];
            slot.value = Some(value);
            return SlotKey {
                index,
                generation: slot.generation,
            };
        }
        let index = u32::try_from(slots.len()).expect("signal arena exhausted");
        slots.push(Slot {
            generation: 0,
            value: Some(value),
            subscribers: Vec::new(),
        });
        SlotKey {
            index,
            generation: 0,
        }
    }

    fn value(&self, key: SlotKey) -> Option<Rc<dyn Any>> {
        let slots = self.slots.borrow();
        let slot = slots.get(key.index as usize)?;
        (slot.generation == key.generation)
            .then(|| slot.value.clone())
            .flatten()
    }

    fn dispose(&self, key: SlotKey) {
        let mut slots = self.slots.borrow_mut();
        let Some(slot) = slots.get_mut(key.index as usize) else {
            return;
        };
        if slot.generation != key.generation || slot.value.is_none() {
            return;
        }
        slot.value = None;
        slot.subscribers.clear();
        slot.generation = slot.generation.wrapping_add(1);
        self.free.borrow_mut().push(key.index);
    }

    fn current_observer(&self) -> Option<ObserverId> {
        self.stack.borrow().last().copied().flatten()
    }

    fn track(&self, key: SlotKey) {
        let Some(observer) = self.current_observer() else {
            return;
        };
        {
            let mut slots = self.slots.borrow_mut();
            let Some(slot) = slots.get_mut(key.index as usize) else {
                return;
            };
            if slot.generation != key.generation || slot.subscribers.contains(&observer) {
                return;
            }
            slot.subscribers.push(observer);
        }
        if let Some(entry) = self.observers.borrow_mut().get_mut(&observer) {
            entry.sources.push(key);
        }
    }

    fn notify(&self, key: SlotKey) {
        let subscribers = {
            let slots = self.slots.borrow();
            match slots.get(key.index as usize) {
                Some(slot) if slot.generation == key.generation => slot.subscribers.clone(),
                _ => return,
            }
        };
        if subscribers.is_empty() {
            return;
        }
        if self.batch_depth.get() > 0 {
            let mut pending = self.pending.borrow_mut();
            for observer in subscribers {
                if !pending.contains(&observer) {
                    pending.push(observer);
                }
            }
            return;
        }
        self.invalidate(&subscribers);
    }

    /// Callbacks run with no runtime borrow held, so they may read or write.
    fn invalidate(&self, observers: &[ObserverId]) {
        let callbacks: Vec<Rc<dyn Fn()>> = {
            let entries = self.observers.borrow();
            observers
                .iter()
                .filter_map(|id| entries.get(id).map(|entry| entry.invalidate.clone()))
                .collect()
        };
        for callback in callbacks {
            callback();
        }
    }

    fn clear_sources(&self, observer: ObserverId) {
        let sources = match self.observers.borrow_mut().get_mut(&observer) {
            Some(entry) => std::mem::take(&mut entry.sources),
            None => return,
        };
        let mut slots = self.slots.borrow_mut();
        for key in sources {
            if let Some(slot) = slots.get_mut(key.index as usize)
                && slot.generation == key.generation
            {
                slot.subscribers.retain(|id| *id != observer);
            }
        }
    }
}

/// Run `body` without subscribing the current observer to anything it reads.
pub fn untrack<R>(body: impl FnOnce() -> R) -> R {
    RUNTIME.with(|runtime| runtime.stack.borrow_mut().push(None));
    let _pop = PopObserver;
    body()
}

/// Coalesce the invalidations of every write in `body` into one notification
/// per observer, delivered when the outermost batch ends.
pub fn batch<R>(body: impl FnOnce() -> R) -> R {
    RUNTIME.with(|runtime| runtime.batch_depth.set(runtime.batch_depth.get() + 1));
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            RUNTIME.with(|runtime| {
                let depth = runtime.batch_depth.get() - 1;
                runtime.batch_depth.set(depth);
                if depth == 0 {
                    let pending = std::mem::take(&mut *runtime.pending.borrow_mut());
                    runtime.invalidate(&pending);
                }
            });
        }
    }
    let _end = End;
    body()
}

/// Apply every update other threads posted through a [`SignalSender`].
/// Called by the scene loop when it drains invalidations.
pub fn drain_posted_signal_updates() {
    let posted = RUNTIME.with(|runtime| {
        std::mem::take(&mut *runtime.inbox.lock().unwrap_or_else(|e| e.into_inner()))
    });
    if posted.is_empty() {
        return;
    }
    batch(|| {
        for update in posted {
            update();
        }
    });
}

struct PopObserver;
impl Drop for PopObserver {
    fn drop(&mut self) {
        RUNTIME.with(|runtime| {
            runtime.stack.borrow_mut().pop();
        });
    }
}

// MARK: - Observer

/// A reading context. Signals read inside [`Observer::track`] subscribe it;
/// each `track` replaces the previous run's dependencies. Dropping the
/// observer unsubscribes it.
pub struct Observer {
    id: ObserverId,
    _local: PhantomData<Rc<()>>,
}

impl Observer {
    pub fn new(invalidate: impl Fn() + 'static) -> Self {
        RUNTIME.with(|runtime| {
            let id = ObserverId(runtime.next_observer.get() + 1);
            runtime.next_observer.set(id.0);
            runtime.observers.borrow_mut().insert(
                id,
                ObserverEntry {
                    invalidate: Rc::new(invalidate),
                    sources: Vec::new(),
                },
            );
            Self {
                id,
                _local: PhantomData,
            }
        })
    }

    pub fn id(&self) -> ObserverId {
        self.id
    }

    /// Evaluate `body` with fresh dependency tracking.
    pub fn track<R>(&self, body: impl FnOnce() -> R) -> R {
        RUNTIME.with(|runtime| {
            runtime.clear_sources(self.id);
            runtime.stack.borrow_mut().push(Some(self.id));
        });
        let _pop = PopObserver;
        body()
    }

    /// Evaluate `body` under this observer, adding to its current dependencies.
    pub fn extend<R>(&self, body: impl FnOnce() -> R) -> R {
        RUNTIME.with(|runtime| runtime.stack.borrow_mut().push(Some(self.id)));
        let _pop = PopObserver;
        body()
    }

    /// Number of signals this observer currently depends on.
    pub fn dependency_count(&self) -> usize {
        RUNTIME.with(|runtime| {
            runtime
                .observers
                .borrow()
                .get(&self.id)
                .map_or(0, |entry| entry.sources.len())
        })
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        // The thread-local may already be gone during thread teardown.
        let _ = RUNTIME.try_with(|runtime| {
            runtime.clear_sources(self.id);
            runtime.observers.borrow_mut().remove(&self.id);
            runtime.pending.borrow_mut().retain(|id| *id != self.id);
        });
    }
}

impl fmt::Debug for Observer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("Observer").field(&self.id.0).finish()
    }
}

// MARK: - Owner

#[derive(Default)]
struct OwnerInner {
    signals: RefCell<Vec<SlotKey>>,
}

/// Owns signals and disposes them on drop. Signals created by
/// [`Signal::new`] while an owner is [running](Owner::run) belong to it.
#[derive(Default)]
pub struct Owner {
    inner: Rc<OwnerInner>,
}

impl Owner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn signal<T: 'static>(&self, value: T) -> Signal<T> {
        let signal = Signal::detached(value);
        self.inner.signals.borrow_mut().push(signal.key);
        signal
    }

    /// Make this owner current for `body`.
    pub fn run<R>(&self, body: impl FnOnce() -> R) -> R {
        RUNTIME.with(|runtime| runtime.owners.borrow_mut().push(self.inner.clone()));
        struct Pop;
        impl Drop for Pop {
            fn drop(&mut self) {
                RUNTIME.with(|runtime| {
                    runtime.owners.borrow_mut().pop();
                });
            }
        }
        let _pop = Pop;
        body()
    }

    pub fn len(&self) -> usize {
        self.inner.signals.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        let signals = std::mem::take(&mut *self.inner.signals.borrow_mut());
        let _ = RUNTIME.try_with(|runtime| {
            for key in signals {
                runtime.dispose(key);
            }
        });
    }
}

// MARK: - Signal

/// A `Copy` handle to reactive state on the current thread.
///
/// `get`/`with` subscribe the running observer; `set`/`update` invalidate
/// subscribers. Writes to a disposed signal are ignored; reads panic, because
/// they mean a view kept a handle past its scope.
pub struct Signal<T: 'static> {
    key: SlotKey,
    _marker: PhantomData<(fn() -> T, Rc<()>)>,
}

impl<T: 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Copy for Signal<T> {}
impl<T: 'static> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl<T: 'static> Eq for Signal<T> {}
impl<T: 'static> std::hash::Hash for Signal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}
impl<T: 'static> fmt::Debug for Signal<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Signal")
            .field("index", &self.key.index)
            .field("generation", &self.key.generation)
            .finish()
    }
}

impl<T: 'static> Signal<T> {
    /// A signal owned by the current [`Owner`], or detached when none runs.
    pub fn new(value: T) -> Self {
        let signal = Self::detached(value);
        RUNTIME.with(|runtime| {
            if let Some(owner) = runtime.owners.borrow().last() {
                owner.signals.borrow_mut().push(signal.key);
            }
        });
        signal
    }

    /// A signal with no owner. It lives until [`Signal::dispose`].
    pub fn detached(value: T) -> Self {
        let key = RUNTIME.with(|runtime| runtime.insert(Rc::new(RefCell::new(value))));
        Self {
            key,
            _marker: PhantomData,
        }
    }

    fn cell(&self) -> Option<Rc<RefCell<T>>> {
        RUNTIME
            .with(|runtime| runtime.value(self.key))
            .map(|value| {
                value
                    .downcast::<RefCell<T>>()
                    .unwrap_or_else(|_| unreachable!("signal slot holds its own type"))
            })
    }

    fn live_cell(&self) -> Rc<RefCell<T>> {
        self.cell().unwrap_or_else(|| {
            panic!(
                "read of a disposed Signal<{}>: a handle outlived the scope that owns it",
                std::any::type_name::<T>()
            )
        })
    }

    /// The value cell, kept alive by holders that may outlive the scope
    /// (bindings handed to closures). Tracking and notification still go
    /// through the key, so they stop once the signal is disposed.
    pub(crate) fn retained_cell(&self) -> Rc<RefCell<T>> {
        self.live_cell()
    }

    pub(crate) fn track_key(&self) {
        RUNTIME.with(|runtime| runtime.track(self.key));
    }

    pub fn is_alive(&self) -> bool {
        RUNTIME.with(|runtime| runtime.value(self.key).is_some())
    }

    /// Borrow the value, subscribing the current observer.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let cell = self.live_cell();
        RUNTIME.with(|runtime| runtime.track(self.key));
        read(&cell.borrow())
    }

    /// Borrow the value without subscribing.
    pub fn with_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.live_cell().borrow())
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        self.with_untracked(T::clone)
    }

    pub fn try_get(&self) -> Option<T>
    where
        T: Clone,
    {
        let cell = self.cell()?;
        RUNTIME.with(|runtime| runtime.track(self.key));
        Some(cell.borrow().clone())
    }

    /// Mutate in place and invalidate subscribers.
    pub fn update(&self, change: impl FnOnce(&mut T)) {
        let Some(cell) = self.cell() else { return };
        change(&mut cell.borrow_mut());
        RUNTIME.with(|runtime| runtime.notify(self.key));
    }

    /// Mutate in place; invalidate only when `change` reports a change.
    pub fn update_if(&self, change: impl FnOnce(&mut T) -> bool) {
        let Some(cell) = self.cell() else { return };
        if change(&mut cell.borrow_mut()) {
            RUNTIME.with(|runtime| runtime.notify(self.key));
        }
    }

    pub fn set(&self, value: T) {
        self.update(|current| *current = value);
    }

    /// Write only when the value differs, so equal writes cause no work.
    pub fn set_if_changed(&self, value: T)
    where
        T: PartialEq,
    {
        self.update_if(|current| {
            if *current == value {
                false
            } else {
                *current = value;
                true
            }
        });
    }

    /// Subscribe the current observer without reading the value: an
    /// invalidation signal for state kept outside the signal.
    pub fn track(&self) {
        if self.is_alive() {
            RUNTIME.with(|runtime| runtime.track(self.key));
        }
    }

    /// Invalidate subscribers without changing the value.
    pub fn notify(&self) {
        RUNTIME.with(|runtime| runtime.notify(self.key));
    }

    pub fn dispose(self) {
        RUNTIME.with(|runtime| runtime.dispose(self.key));
    }

    /// A writer usable from any thread. Updates apply on this thread at the
    /// next [`drain_posted_signal_updates`].
    pub fn sender(&self) -> SignalSender<T>
    where
        T: Send,
    {
        let inbox = RUNTIME.with(|runtime| runtime.inbox.clone());
        let key = self.key;
        SignalSender {
            inbox,
            apply: Arc::new(move |change: Box<dyn FnOnce(&mut T) + Send>| {
                Box::new(move || {
                    Signal::<T> {
                        key,
                        _marker: PhantomData,
                    }
                    .update(change)
                })
            }),
        }
    }

    pub(crate) fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

impl<T: Default + 'static> Default for Signal<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

type Apply<T> = Arc<dyn Fn(Box<dyn FnOnce(&mut T) + Send>) -> Posted + Send + Sync>;

/// Thread-safe writer for a [`Signal`].
pub struct SignalSender<T: 'static> {
    inbox: Arc<Mutex<Vec<Posted>>>,
    apply: Apply<T>,
}

impl<T: 'static> Clone for SignalSender<T> {
    fn clone(&self) -> Self {
        Self {
            inbox: self.inbox.clone(),
            apply: self.apply.clone(),
        }
    }
}

impl<T: Send + 'static> SignalSender<T> {
    pub fn update(&self, change: impl FnOnce(&mut T) + Send + 'static) {
        let posted = (self.apply)(Box::new(change));
        self.inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(posted);
        crate::scene::wake_scene_thread();
    }

    pub fn set(&self, value: T) {
        self.update(move |current| *current = value);
    }
}

// MARK: - Type-erased handle

/// A signal of unknown type, for owners that store heterogeneous state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnySignal {
    key: SlotKey,
    type_id: TypeId,
}

impl AnySignal {
    pub fn downcast<T: 'static>(self) -> Option<Signal<T>> {
        (self.type_id == TypeId::of::<T>()).then_some(Signal {
            key: self.key,
            _marker: PhantomData,
        })
    }

    pub fn dispose(self) {
        RUNTIME.with(|runtime| runtime.dispose(self.key));
    }
}

impl<T: 'static> From<Signal<T>> for AnySignal {
    fn from(signal: Signal<T>) -> Self {
        Self {
            key: signal.key,
            type_id: signal.type_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter() -> (Rc<Cell<usize>>, Observer) {
        let count = Rc::new(Cell::new(0));
        let seen = count.clone();
        (count, Observer::new(move || seen.set(seen.get() + 1)))
    }

    #[test]
    fn reads_subscribe_and_writes_invalidate() {
        let signal = Signal::detached(1);
        let (count, observer) = counter();
        assert_eq!(observer.track(|| signal.get()), 1);
        signal.set(2);
        signal.set(3);
        assert_eq!(count.get(), 2);
        assert_eq!(signal.get_untracked(), 3);
        signal.dispose();
    }

    #[test]
    fn retracking_drops_stale_dependencies() {
        let first = Signal::detached(0);
        let second = Signal::detached(0);
        let (count, observer) = counter();
        observer.track(|| first.get());
        observer.track(|| second.get());
        first.set(1);
        assert_eq!(count.get(), 0);
        second.set(1);
        assert_eq!(count.get(), 1);
        assert_eq!(observer.dependency_count(), 1);
    }

    #[test]
    fn untracked_and_handler_reads_do_not_subscribe() {
        let signal = Signal::detached(0);
        let (count, observer) = counter();
        observer.track(|| untrack(|| signal.get()));
        signal.get();
        signal.set(1);
        assert_eq!(count.get(), 0);
    }

    #[test]
    fn nested_observers_track_independently() {
        let outer_value = Signal::detached(0);
        let inner_value = Signal::detached(0);
        let (outer_count, outer) = counter();
        let (inner_count, inner) = counter();
        outer.track(|| {
            outer_value.get();
            inner.track(|| inner_value.get());
        });
        inner_value.set(1);
        assert_eq!((outer_count.get(), inner_count.get()), (0, 1));
        outer_value.set(1);
        assert_eq!((outer_count.get(), inner_count.get()), (1, 1));
    }

    #[test]
    fn batch_coalesces_and_set_if_changed_skips_equal_writes() {
        let signal = Signal::detached(0);
        let (count, observer) = counter();
        observer.track(|| signal.get());
        batch(|| {
            signal.set(1);
            signal.set(2);
        });
        assert_eq!(count.get(), 1);
        signal.set_if_changed(2);
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn owner_disposes_and_slots_are_reused_safely() {
        let owner = Owner::new();
        let stale = owner.run(|| Signal::new(5));
        assert_eq!(owner.len(), 1);
        drop(owner);
        assert!(!stale.is_alive());
        stale.set(6);
        let fresh = Signal::detached(7);
        assert_eq!(fresh.key.index, stale.key.index);
        assert!(!stale.is_alive());
        assert_eq!(fresh.get(), 7);
    }

    #[test]
    fn dropped_observer_is_not_invalidated() {
        let signal = Signal::detached(0);
        let (count, observer) = counter();
        observer.track(|| signal.get());
        drop(observer);
        signal.set(1);
        assert_eq!(count.get(), 0);
    }

    #[test]
    fn invalidation_may_write_other_signals() {
        let source = Signal::detached(0);
        let mirror = Signal::detached(0);
        let observer = Observer::new(move || mirror.set(source.get_untracked()));
        observer.track(|| source.get());
        source.set(4);
        assert_eq!(mirror.get(), 4);
    }

    #[test]
    fn senders_post_to_the_owning_thread() {
        let signal = Signal::detached(0_u32);
        let sender = signal.sender();
        std::thread::spawn(move || sender.set(9)).join().unwrap();
        assert_eq!(signal.get(), 0);
        drain_posted_signal_updates();
        assert_eq!(signal.get(), 9);
    }

    #[test]
    fn any_signal_round_trips_only_its_own_type() {
        let signal = Signal::detached(String::from("a"));
        let erased = AnySignal::from(signal);
        assert!(erased.downcast::<u32>().is_none());
        assert_eq!(erased.downcast::<String>().unwrap().get(), "a");
    }
}

#[cfg(test)]
mod binding_tests {
    use crate::{Binding, Owner, Signal};

    #[test]
    fn a_binding_outlives_its_signal() {
        let owner = Owner::new();
        let signal: Signal<i32> = owner.run(|| Signal::new(3));
        let binding: Binding<i32> = signal.into();
        drop(owner);
        assert_eq!(binding.get(), 3, "a stale binding reads the last value");
        binding.set(4);
        assert_eq!(binding.get(), 4, "writes land in the detached cell");
    }
}
