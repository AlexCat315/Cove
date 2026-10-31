//! Translation of `Tests/Headless/Tests/CoveUISceneTests/StateFieldReadTests.swift`.
//!
//! The Swift suite installs a `@TrackStateReads` view, then observes
//! `Recomposer.hasPending` after writing through a `@State` model and counts how
//! often the instrumented `body` ran. The Rust port keeps the same setup and
//! counts: [`Harness::install`] queues one body evaluation on the `Recomposer`,
//! the body runs inside a `ReadScope` exactly like `ViewGraph`, and every read
//! goes through the `State::read_field` / `read_array_element` /
//! `read_map_value` / `read_opaque` calls that `#[track_state_reads]` generates.
//!
//! The read-scope stack is process-global thread-local state, so every test runs
//! under [`GlobalTestLock`].
//!
//! Behaviour checklist:
//! - only a changed read field invalidates; conditional reads replace old
//!   dependencies
//! - an `Equatable` value holding a reference cannot hide a mutation
//! - whole value reads retain conservative invalidation
//! - unread state can still serve as an explicit invalidation signal
//! - subscript element and dictionary key tracking invalidate only the read index
//! - a custom subscriptable falls back to whole-field tracking
//! - separate `@State` properties are independent roots
//! - deferred layout callbacks retain their state owner
//! - the scope stack is thread-local on every platform

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use cove_core::scene::{Node, NodeTree, Recomposer};
use cove_core::{ReadScope, ReadScopeId, SnapshotValue, State, remove_read_scope, with_read_scope};

/// Serializes tests that push and read the process-global read-scope stack.
struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn locked<T>(body: impl FnOnce() -> T) -> T {
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        body()
    }
}

/// A body-execution tally, the Swift `Counter`.
type Counter = Rc<Cell<usize>>;

fn counter() -> Counter {
    Rc::new(Cell::new(0))
}

fn bump(counter: &Counter) {
    counter.set(counter.get() + 1);
}

fn count(counter: &Counter) -> usize {
    counter.get()
}

/// A recomposer plus the read scope one view body owns.
struct Harness {
    recomposer: Rc<Recomposer>,
    scope: ReadScope,
    scope_id: ReadScopeId,
    /// Wake count from the scope's invalidator. The invalidator must be
    /// `Send + Sync`, so it only counts; the test drains that count into the
    /// `Recomposer`, which is what `ViewGraph` does when it owns both sides.
    invalidations: Arc<AtomicUsize>,
    unwire: RefCell<Vec<Box<dyn Fn()>>>,
    /// The installed view body, re-run whenever a dependency wakes the scope.
    body: RefCell<Option<Rc<dyn Fn()>>>,
    body_counter: RefCell<Option<Counter>>,
}

impl Harness {
    fn new() -> Self {
        let recomposer = Rc::new(Recomposer::new());
        let invalidations = Arc::new(AtomicUsize::new(0));
        let counted = invalidations.clone();
        let scope = ReadScope::new(move |_field: Option<&str>| {
            counted.fetch_add(1, Ordering::Relaxed);
        });
        let scope_id = scope.id;
        Self {
            recomposer,
            scope,
            scope_id,
            invalidations,
            unwire: RefCell::new(Vec::new()),
            body: RefCell::new(None),
            body_counter: RefCell::new(None),
        }
    }

    /// Opts `state` into precise field tracking against this body's scope, the
    /// way `DynamicField::wire_read_tracking` does for `@State` members.
    fn wire<T: Send + 'static>(&self, state: &State<T>) {
        state.set_read_tracking_owner(self.scope.clone(), true);
        let owned = state.clone();
        let scope_id = self.scope_id;
        self.unwire
            .borrow_mut()
            .push(Box::new(move || owned.clear_read_tracking_owner(scope_id)));
    }

    /// One initial body evaluation, matching `graph.install(root:)`.
    fn install(&self, counter: &Counter, evaluate: Rc<dyn Fn()>) {
        *self.body.borrow_mut() = Some(evaluate);
        *self.body_counter.borrow_mut() = Some(counter.clone());
        self.evaluate();
        self.recomposer.commit_all();
    }

    /// Queues one body evaluation on the recomposer.
    fn evaluate(&self) {
        let Some(body) = self.body.borrow().clone() else {
            return;
        };
        let Some(counter) = self.body_counter.borrow().clone() else {
            return;
        };
        let scope = self.scope.clone();
        self.recomposer.invalidate(1, None, None, move || {
            with_read_scope(&scope, || {
                bump(&counter);
                body();
            });
        });
    }

    /// Runs whatever the last write queued, the Swift `recomposer.commitAll()`.
    fn commit(&self) {
        self.drain();
        self.recomposer.commit_all();
    }

    fn has_pending(&self) -> bool {
        self.drain();
        self.recomposer.has_pending()
    }

    /// Releases the state owners and the scope, the Swift
    /// `defer { graph.install(root: EmptyView()) }`.
    fn uninstall(&self) {
        self.body.borrow_mut().take();
        self.body_counter.borrow_mut().take();
        for clear in self.unwire.borrow_mut().drain(..) {
            clear();
        }
        remove_read_scope(self.scope_id);
        self.recomposer.commit_all();
    }

    fn drain(&self) {
        if self.invalidations.swap(0, Ordering::Relaxed) > 0 {
            self.evaluate();
        }
    }
}

#[derive(Clone, PartialEq)]
struct Details {
    title: String,
    other: i32,
}

impl SnapshotValue for Details {}

#[derive(Clone, PartialEq)]
struct Model {
    name: String,
    age: i32,
    details: Details,
}

impl Model {
    fn new() -> Self {
        Self {
            name: "a".into(),
            age: 0,
            details: Details {
                title: "a".into(),
                other: 0,
            },
        }
    }
}

fn model_state() -> State<Model> {
    State::new(Model::new())
}

#[test]
fn selective_and_conditional() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let model = model_state();
        let use_name = State::new(true);
        harness.wire(&model);
        harness.wire(&use_name);

        // The `use_name` branch reads `model.name`; the other reads `details`.
        let body = {
            let model = model.clone();
            let use_name = use_name.clone();
            Rc::new(move || {
                if use_name.get() {
                    let _ = model.read_field("name", |state: &Model| state.name.clone());
                } else {
                    let _ = model.read_field("details", |state: &Model| state.details.clone());
                }
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        model.update(|state| state.age += 1);
        assert!(
            !harness.has_pending(),
            "an unread field does not invalidate"
        );

        model.update(|state| state.name = "b".into());
        assert!(harness.has_pending(), "the read field invalidates");
        harness.commit();
        assert_eq!(count(&counter), 2);

        model.update(|state| state.name = "b".into());
        assert!(
            !harness.has_pending(),
            "an unchanged value does not invalidate"
        );

        use_name.set(false);
        harness.commit();
        assert_eq!(count(&counter), 3, "the branch swap re-evaluates the body");

        // The swap replaced the dependency: `name` is no longer subscribed.
        // Rust subs to the whole `details` field rather than Swift's
        // `details.title`, because a nested member read has no field selector;
        // the port therefore tracks that field more coarsely, not less.
        model.update(|state| state.name = "c".into());
        assert!(
            !harness.has_pending(),
            "the dropped `name` dependency stops invalidating"
        );

        model.update(|state| state.details.title = "b".into());
        harness.commit();
        assert_eq!(
            count(&counter),
            4,
            "the new `details` dependency invalidates"
        );

        harness.uninstall();
        model.update(|state| state.details.title = "c".into());
        assert!(
            !harness.has_pending(),
            "an uninstalled scope records nothing"
        );
    });
}

/// A value whose `PartialEq` only compares a reference's contents, the Swift
/// `OpaqueField` shape: the comparison can be blind to a mutation.
#[derive(Clone)]
struct OpaqueField {
    text: Arc<Mutex<String>>,
}

impl PartialEq for OpaqueField {
    fn eq(&self, other: &Self) -> bool {
        // Two clones can share one mutex, so read each side before comparing.
        let left = lock(&self.text).clone();
        let right = lock(&other.text).clone();
        left == right
    }
}

fn lock(value: &Mutex<String>) -> MutexGuard<'_, String> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

#[test]
fn opaque_fallback() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let shared = Arc::new(Mutex::new("a".to_string()));
        let model = State::new(OpaqueField {
            text: shared.clone(),
        });
        harness.wire(&model);

        let body = {
            let model = model.clone();
            Rc::new(move || {
                let _ = model.read_opaque("field", |state: &OpaqueField| lock(&state.text).clone());
            })
        };
        harness.install(&counter, body);

        // The Swift shape: mutate the box, then assign the replacement back to
        // the state. Both sides of `PartialEq` now read "b", so a field read
        // would consider the value unchanged; the opaque read must not.
        *lock(&shared) = "b".into();
        let replacement = model.get();
        assert!(
            replacement == model.get(),
            "the stale snapshot compares equal"
        );
        model.set(replacement);
        assert!(
            harness.has_pending(),
            "an opaque read cannot hide a mutation"
        );
        harness.commit();
        assert_eq!(count(&counter), 2);
    });
}

#[test]
fn whole_value_reads_are_conservative() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let model = model_state();
        harness.wire(&model);

        let body = {
            let model = model.clone();
            Rc::new(move || {
                let _ = model.get();
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        model.update(|state| state.age += 1);
        assert!(
            harness.has_pending(),
            "a whole-value read watches every field"
        );
        harness.commit();
        assert_eq!(count(&counter), 2);
    });
}

#[test]
fn unread_signal() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let revision = State::new(0i32);
        harness.wire(&revision);

        let body = {
            let revision = revision.clone();
            Rc::new(move || {
                let _ = revision.read_signal("revision", |state: &i32| *state);
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        revision.update(|value| *value += 1);
        harness.commit();
        assert_eq!(count(&counter), 2, "an unread signal still invalidates");
    });
}

#[test]
fn subscript_element() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let items = State::new(vec![1, 2, 3]);
        harness.wire(&items);

        let body = {
            let items = items.clone();
            Rc::new(move || {
                let _ = items.read_array_element("items", 0, |state: &Vec<i32>| state.as_slice());
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        items.update(|values| values[1] = 99);
        assert!(
            !harness.has_pending(),
            "an unread index does not invalidate"
        );

        items.update(|values| values[0] = 42);
        harness.commit();
        assert_eq!(count(&counter), 2);

        items.set(vec![9, 9, 9]);
        harness.commit();
        assert_eq!(
            count(&counter),
            3,
            "replacing the array invalidates the read index"
        );
    });
}

#[test]
fn subscript_dictionary() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let dict: State<HashMap<String, i32>> =
            State::new(HashMap::from([("a".to_string(), 1), ("b".to_string(), 2)]));
        harness.wire(&dict);

        let body = {
            let dict = dict.clone();
            Rc::new(move || {
                let _ =
                    dict.read_map_value("dict", "a".to_string(), |state: &HashMap<String, i32>| {
                        state
                    });
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        dict.update(|entries| {
            entries.insert("b".into(), 99);
        });
        assert!(!harness.has_pending(), "an unread key does not invalidate");

        dict.update(|entries| {
            entries.insert("a".into(), 42);
        });
        harness.commit();
        assert_eq!(count(&counter), 2);

        dict.set(HashMap::from([("a".to_string(), 5), ("b".to_string(), 6)]));
        harness.commit();
        assert_eq!(
            count(&counter),
            3,
            "replacing the map invalidates the read key"
        );
    });
}

#[test]
fn subscript_custom() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        // A custom subscriptable type has no element-precise selector, so the
        // whole field is snapshotted: any change to it invalidates.
        let grid = State::new(vec![1, 2, 3]);
        harness.wire(&grid);

        let body = {
            let grid = grid.clone();
            Rc::new(move || {
                let _ = grid.read_field("grid", |cells: &Vec<i32>| cells.clone());
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        grid.update(|cells| cells[1] = 99);
        harness.commit();
        assert_eq!(
            count(&counter),
            2,
            "whole-field granularity invalidates on any change"
        );

        grid.update(|cells| {
            let _ = cells.len();
        });
        assert!(
            !harness.has_pending(),
            "an unchanged field does not re-invalidate"
        );
    });
}

#[test]
fn two_states_are_independent() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let first = State::new(vec![1, 2]);
        let second = State::new(vec![3, 4]);
        harness.wire(&first);
        harness.wire(&second);

        // The read elements are `first[0]` and `second[1]`.
        let body = {
            let first = first.clone();
            let second = second.clone();
            Rc::new(move || {
                let _ = first.read_array_element("items", 0, |state: &Vec<i32>| state.as_slice());
                let _ = second.read_array_element("items", 1, |state: &Vec<i32>| state.as_slice());
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        first.update(|items| items[1] = 99);
        second.update(|items| items[0] = 99);
        assert!(
            !harness.has_pending(),
            "unread indices of either state do not invalidate"
        );
        assert_eq!(count(&counter), 1);

        second.update(|items| items[1] = 42);
        harness.commit();
        assert_eq!(
            count(&counter),
            2,
            "the second state's read element invalidates"
        );

        first.update(|items| items[0] = 7);
        harness.commit();
        assert_eq!(
            count(&counter),
            3,
            "the first state's read element invalidates"
        );
    });
}

#[test]
fn deferred_layout_retains_owner() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let body_counter = counter();
        let layout_counter = counter();
        let model = model_state();
        harness.wire(&model);

        let body = {
            let model = model.clone();
            Rc::new(move || {
                let _ = model.read_field("name", |state: &Model| state.name.clone());
            })
        };
        harness.install(&body_counter, body);

        // The layout callback reads later against the same owner scope, the way
        // `Node::layout_did_update` fires during the layout pass.
        let node = Node::new();
        let scope = harness.scope.clone();
        let layout_model = model.clone();
        let layout = layout_counter.clone();
        node.set_layout_did_update(Some(Rc::new(move |_: &Node| {
            let scope = scope.clone();
            let model = layout_model.clone();
            let layout = layout.clone();
            with_read_scope(&scope, || {
                let _ = model.read_field("name", |state: &Model| state.name.clone());
                bump(&layout);
            });
        })));
        let hook = node
            .layout_did_update()
            .expect("the layout hook is installed");
        hook(&node);
        assert_eq!(count(&layout_counter), 1);

        model.update(|state| state.age += 1);
        assert!(
            !harness.has_pending(),
            "an unread field leaves the layout dependency intact"
        );

        model.update(|state| state.name = "b".into());
        assert!(
            harness.has_pending(),
            "the layout callback retained the state field"
        );
        harness.commit();
        assert_eq!(count(&body_counter), 2);
    });
}

#[test]
fn scope_stack_is_thread_local() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let model = model_state();
        harness.wire(&model);

        // The body reads `name` on this thread, inside the installed scope.
        let body = {
            let model = model.clone();
            Rc::new(move || {
                let _ = model.read_field("name", |state: &Model| state.name.clone());
            })
        };
        harness.install(&counter, body);
        assert_eq!(count(&counter), 1);

        // A genuine OS thread starts with an empty scope stack, so a read there
        // records no dependency against this thread's scope.
        let worker_model = model.clone();
        std::thread::scope(|threads| {
            threads.spawn(move || {
                let _ = worker_model.read_field("age", |state: &Model| state.age);
            });
        });

        // `age` was only read off-scope, so writing it must not invalidate.
        model.update(|state| state.age += 1);
        assert!(
            !harness.has_pending(),
            "a read on another thread records nothing here"
        );

        // `name` was read on this thread, so it still invalidates.
        model.update(|state| state.name = "b".into());
        assert!(
            harness.has_pending(),
            "the on-thread read still owns its dependency"
        );
    });
}

#[test]
fn uninstalled_scope_stops_tracking() {
    GlobalTestLock::locked(|| {
        let harness = Harness::new();
        let counter = counter();
        let model = model_state();
        harness.wire(&model);

        let body = {
            let model = model.clone();
            Rc::new(move || {
                let _ = model.read_field("name", |state: &Model| state.name.clone());
            })
        };
        harness.install(&counter, body);

        model.update(|state| state.name = "b".into());
        assert!(
            harness.has_pending(),
            "a wired, installed scope tracks its field"
        );
        harness.commit();

        harness.uninstall();
        model.update(|state| state.name = "c".into());
        assert!(
            !harness.has_pending(),
            "an uninstalled scope records nothing"
        );

        let tree = RefCell::new(NodeTree::new());
        tree.borrow_mut().set_root(Some(Node::new()));
        assert!(
            !tree.borrow().has_render_updates(),
            "the empty install leaves no render work"
        );
    });
}
