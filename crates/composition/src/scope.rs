//! The composition scope handed to [`View::body`](crate::View::body).
//!
//! State belongs to the scope, not to the view value: the parent rebuilds view
//! values on every recomposition, but a scope lives as long as its view keeps
//! its identity in the tree. `cx.state` keys each signal by its call site (and
//! occurrence, for loops), so successive bodies get the same signal back.
use cove_core::scene::{CompositionLocal, Node, NodeWeak};
use cove_core::{AnySignal, Owner, Signal};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::panic::Location;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Site {
    location: &'static Location<'static>,
    discriminator: u64,
    type_id: TypeId,
}

#[derive(Default)]
struct Slots {
    owner: Owner,
    signals: RefCell<HashMap<(Site, u32), AnySignal>>,
    visits: RefCell<HashMap<Site, u32>>,
}

/// State storage and identity for one view instance.
#[derive(Clone, Default)]
pub struct Scope {
    slots: Rc<Slots>,
    /// The node this view composes into, for reading ambient values.
    anchor: Option<NodeWeak>,
}

impl Scope {
    /// A scope that is not part of a view graph, for evaluating a body
    /// directly (tests, previews). Its state is dropped with the last clone.
    pub fn detached() -> Self {
        Self::default()
    }

    pub(crate) fn anchored(anchor: &Node) -> Self {
        Self {
            slots: Rc::default(),
            anchor: Some(anchor.downgrade()),
        }
    }

    /// The value `local` has where this view is mounted, or its default for a
    /// detached scope. A change to it recomposes the view, so a body can read
    /// the surface environment, theme or other ambient values directly.
    pub fn composition_value<Value: Clone + 'static>(
        &self,
        local: &CompositionLocal<Value>,
    ) -> Value {
        self.anchor.as_ref().and_then(Node::upgrade).map_or_else(
            || local.default_value(),
            |node| node.composition_value(local),
        )
    }

    /// Start a body evaluation: occurrence counters restart so the n-th call
    /// at a site maps to the same slot as last time.
    pub(crate) fn begin(&self) {
        self.slots.visits.borrow_mut().clear();
    }

    /// State that persists for the lifetime of this view's identity.
    ///
    /// `init` runs only the first time this call site is reached. Calls inside
    /// a loop are told apart by order; use [`Scope::keyed_state`] when items
    /// can be inserted or reordered.
    #[track_caller]
    pub fn state<T: 'static>(&self, init: impl FnOnce() -> T) -> Signal<T> {
        self.slot(Location::caller(), 0, init)
    }

    /// A value created once for this view's identity and handed back on every
    /// body, without subscribing to it: retained handles, caches, controllers.
    #[track_caller]
    pub fn remember<T: Clone + 'static>(&self, init: impl FnOnce() -> T) -> T {
        self.slot(Location::caller(), 0, init).get_untracked()
    }

    /// State identified by `key` at this call site, so it follows its item
    /// across insertions and reorders.
    #[track_caller]
    pub fn keyed_state<K: Hash, T: 'static>(&self, key: K, init: impl FnOnce() -> T) -> Signal<T> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        self.slot(Location::caller(), hasher.finish(), init)
    }

    fn slot<T: 'static>(
        &self,
        location: &'static Location<'static>,
        discriminator: u64,
        init: impl FnOnce() -> T,
    ) -> Signal<T> {
        let site = Site {
            location,
            discriminator,
            type_id: TypeId::of::<T>(),
        };
        let occurrence = {
            let mut visits = self.slots.visits.borrow_mut();
            let count = visits.entry(site).or_insert(0);
            let occurrence = *count;
            *count += 1;
            occurrence
        };
        let key = (site, occurrence);
        if let Some(signal) = self
            .slots
            .signals
            .borrow()
            .get(&key)
            .and_then(|signal| signal.downcast::<T>())
        {
            return signal;
        }
        let signal = self.slots.owner.signal(init());
        self.slots.signals.borrow_mut().insert(key, signal.into());
        signal
    }

    /// Number of signals this scope owns.
    pub fn state_count(&self) -> usize {
        self.slots.owner.len()
    }
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scope")
            .field("states", &self.state_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(cx: &Scope) -> (Signal<i32>, Vec<Signal<i32>>) {
        cx.begin();
        let single = cx.state(|| 1);
        let looped = (0..3).map(|i| cx.state(|| i * 10)).collect();
        (single, looped)
    }

    #[test]
    fn repeated_bodies_get_the_same_signals() {
        let cx = Scope::detached();
        let (single, looped) = body(&cx);
        single.set(5);
        let (again, looped_again) = body(&cx);
        assert_eq!(single, again);
        assert_eq!(looped, looped_again);
        assert_eq!(again.get(), 5);
        assert_eq!(cx.state_count(), 4);
    }

    #[test]
    fn keyed_state_follows_its_key() {
        let cx = Scope::detached();
        let read = |order: &[&str]| {
            cx.begin();
            order
                .iter()
                .map(|key| cx.keyed_state(*key, || key.len()))
                .collect::<Vec<_>>()
        };
        let first = read(&["a", "bb"]);
        first[1].set(9);
        let second = read(&["bb", "a"]);
        assert_eq!(second[0].get(), 9);
    }

    #[test]
    fn dropping_the_scope_disposes_its_state() {
        let cx = Scope::detached();
        let (single, _) = body(&cx);
        drop(cx);
        assert!(!single.is_alive());
    }
}
