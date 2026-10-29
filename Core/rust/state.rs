use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

type Observer = Arc<dyn Fn(u64) + Send + Sync>;
struct Storage<T> {
    value: Mutex<(T, u64)>,
    observers: Mutex<BTreeMap<u64, Observer>>,
    next_id: AtomicU64,
}

pub struct State<T>(Arc<Storage<T>>);
impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> State<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(Storage {
            value: Mutex::new((value, 0)),
            observers: Mutex::new(BTreeMap::new()),
            next_id: AtomicU64::new(1),
        }))
    }
    pub fn snapshot(&self) -> (T, u64)
    where
        T: Clone,
    {
        self.0
            .value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.snapshot().0
    }
    /// Callbacks run outside the value lock and may read or update this state.
    /// Concurrent callbacks can arrive out of order; the revision identifies stale work.
    pub fn update(&self, change: impl FnOnce(&mut T)) {
        let revision = {
            let mut value = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
            change(&mut value.0);
            value.1 = value.1.checked_add(1).expect("state revision exhausted");
            value.1
        };
        let callbacks: Vec<_> = self
            .0
            .observers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for callback in callbacks {
            callback(revision);
        }
    }
    pub fn set(&self, value: T) {
        self.update(|current| *current = value);
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
