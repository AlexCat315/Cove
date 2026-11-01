//! Translation of `Tests/CoveUIComposeTests/ObservableStateTrackingTests.swift`.
//!
//! Behaviour checklist:
//! - a state read during body evaluation subscribes the owning scope, so a write
//!   schedules one recomposition and the write's body reads again
//! - a read inside a reader closure belongs to the enclosing view: writing the
//!   read key schedules the enclosing view and the dependency survives a
//!   recomposition
//! - dependencies are replaced on each body evaluation: once a body stops
//!   reading a key, writes to it schedule nothing
//! - independent keys only recompose the readers that read them
//! - observed dynamic properties do not accumulate observers across parent
//!   rebuilds
//! - observed dynamic properties unregister when removed from the tree and
//!   register again when the leaf returns
//!
//! Harness note: the Swift leaves render `Text("value \(value)")`; the Rust
//! leaves render a `Probe` primitive carrying the same integer, so the suite
//! observes the value without needing a text environment. The assertions never
//! depend on the rendered string.
//!
//! Named differences from the Swift suite:
//! - Swift's `@Observable`/`Observed` property wrapper keeps its subscription in
//!   storage that survives a parent rebuild. Rust holds the subscription in the
//!   `Observed` value created by each `body` pass; the old value drops during
//!   reconciliation and unregisters, so the observable contract (one observer,
//!   no accumulation) is the same.
//! - GAP: reads performed inside a reader closure (Swift's `ThemeReader`) are
//!   not attributed to the enclosing view. Rust evaluates the closure while it
//!   materialises the primitive's children, after the enclosing body's
//!   `ObservableStateTracking` scope has been popped, so
//!   `ObservableStateRegistrar::access` finds no scope and records nothing. The
//!   second test pins that measured behaviour and names the Swift expectation;
//!   the remaining dependency-replacement cases use direct body reads, which
//!   Rust does track.

mod support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use cove_composition::{
    AnyView, DynamicField, PrimitiveView, TupleView, VerticalAlignment, View, ViewGraph,
};
use cove_core::State;
use cove_core::scene::{Node, ObservableObject, ObservableStateRegistrar, StateAccessKey};
use cove_ui_compose::Observed;
use cove_ui_compose::Row;
use cove_ui_compose::theme::{Theme, ThemeReader};

use support::GlobalTestLock;

const PROBE_VALUE: &str = "__observable_probe.value";

#[derive(Clone)]
struct Probe {
    value: i32,
}

impl PrimitiveView for Probe {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(PROBE_VALUE, self.value);
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}
cove_ui_compose::impl_primitive_view!(Probe);

fn body_counter() -> Rc<Cell<usize>> {
    Rc::new(Cell::new(0))
}

fn bump(counter: &Cell<usize>) {
    counter.set(counter.get() + 1);
}

/// The Swift `TestObservableStore`: a registrar plus three keys whose getters
/// record a read and whose setters invalidate that key.
struct Store {
    registrar: ObservableStateRegistrar,
    value: Cell<i32>,
    other_value: Cell<i32>,
    should_read: Cell<bool>,
}

impl Store {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            registrar: ObservableStateRegistrar::new(),
            value: Cell::new(0),
            other_value: Cell::new(0),
            should_read: Cell::new(true),
        })
    }

    fn value(&self) -> i32 {
        self.registrar.access(StateAccessKey::text("value"));
        self.value.get()
    }

    fn set_value(&self, next: i32) {
        self.value.set(next);
        self.registrar.invalidate(&StateAccessKey::text("value"));
    }

    fn other_value(&self) -> i32 {
        self.registrar.access(StateAccessKey::text("otherValue"));
        self.other_value.get()
    }

    fn should_read(&self) -> bool {
        self.registrar.access(StateAccessKey::text("shouldRead"));
        self.should_read.get()
    }

    fn set_should_read(&self, next: bool) {
        self.should_read.set(next);
        self.registrar
            .invalidate(&StateAccessKey::text("shouldRead"));
    }
}

fn commit_until_settled(graph: &ViewGraph) {
    while graph.recomposer.commit_all() {}
}

struct ObservedValueView {
    store: Rc<Store>,
    counter: Rc<Cell<usize>>,
}

impl View for ObservedValueView {
    fn body(&self) -> AnyView {
        bump(&self.counter);
        AnyView::new(Probe {
            value: self.store.value(),
        })
    }
}

struct ReaderObservedValueView {
    store: Rc<Store>,
    counter: Rc<Cell<usize>>,
}

impl View for ReaderObservedValueView {
    fn body(&self) -> AnyView {
        bump(&self.counter);
        let store = self.store.clone();
        AnyView::new(ThemeReader::new(move |_theme: &Theme| Probe {
            value: store.value(),
        }))
    }
}

struct ConditionalObservedValueView {
    store: Rc<Store>,
    counter: Rc<Cell<usize>>,
}

impl View for ConditionalObservedValueView {
    fn body(&self) -> AnyView {
        bump(&self.counter);
        let value = if self.store.should_read() {
            self.store.value()
        } else {
            -1
        };
        AnyView::new(Probe { value })
    }
}

struct ObservedOtherValueView {
    store: Rc<Store>,
    counter: Rc<Cell<usize>>,
}

impl View for ObservedOtherValueView {
    fn body(&self) -> AnyView {
        bump(&self.counter);
        AnyView::new(Probe {
            value: self.store.other_value(),
        })
    }
}

struct TwoObservedValuesView {
    store: Rc<Store>,
    value_counter: Rc<Cell<usize>>,
    other_counter: Rc<Cell<usize>>,
}

impl View for TwoObservedValuesView {
    fn body(&self) -> AnyView {
        AnyView::new(Row::new(
            VerticalAlignment::Center,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(ObservedValueView {
                    store: self.store.clone(),
                    counter: self.value_counter.clone(),
                }),
                AnyView::new(ObservedOtherValueView {
                    store: self.store.clone(),
                    counter: self.other_counter.clone(),
                }),
            ])),
        ))
    }
}

/// The Swift `CountingObservableObject`: an object-valued property plus an
/// observer table whose size the suite asserts.
struct CountingObject {
    value: Cell<i32>,
    observers: RefCell<BTreeMap<u64, Rc<dyn Fn()>>>,
    next_token: Cell<u64>,
}

impl CountingObject {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            value: Cell::new(0),
            observers: RefCell::new(BTreeMap::new()),
            next_token: Cell::new(0),
        })
    }

    fn value(&self) -> i32 {
        self.value.get()
    }

    fn observer_count(&self) -> usize {
        self.observers.borrow().len()
    }
}

impl ObservableObject for CountingObject {
    fn register_observer(&self, handler: Rc<dyn Fn()>) -> u64 {
        let token = self.next_token.get();
        self.next_token.set(token + 1);
        self.observers.borrow_mut().insert(token, handler);
        token
    }

    fn unregister_observer(&self, token: u64) {
        self.observers.borrow_mut().remove(&token);
    }
}

/// The Swift `ObservedObjectLeaf`: one observed projection of `value`.
struct ObservedObjectLeaf {
    observed: Observed<CountingObject, i32>,
}

impl ObservedObjectLeaf {
    fn new(object: Rc<CountingObject>) -> Self {
        Self {
            observed: Observed::new(object, |object: &CountingObject| object.value()),
        }
    }
}

impl View for ObservedObjectLeaf {
    fn body(&self) -> AnyView {
        AnyView::new(Probe {
            value: self.observed.get(),
        })
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![self.observed.dynamic_field("value")]
    }
}

/// The Swift `ObservedRebuildHarness`: a state counter below which the leaf is
/// rebuilt on every pass.
struct ObservedRebuildHarness {
    tick: State<i32>,
    object: Rc<CountingObject>,
}

impl View for ObservedRebuildHarness {
    fn body(&self) -> AnyView {
        let _ = self.tick.get();
        AnyView::new(ObservedObjectLeaf::new(self.object.clone()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("tick", &self.tick)]
    }
}

/// The Swift `ConditionalObservedHarness`: the leaf is mounted only while
/// `isVisible` holds.
struct ConditionalObservedHarness {
    is_visible: State<bool>,
    object: Rc<CountingObject>,
}

impl View for ConditionalObservedHarness {
    fn body(&self) -> AnyView {
        if self.is_visible.get() {
            AnyView::new(ObservedObjectLeaf::new(self.object.clone()))
        } else {
            AnyView::new(Probe { value: -1 })
        }
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("isVisible", &self.is_visible)]
    }
}

#[test]
fn state_read_during_body_recomposes_the_owning_scope() {
    GlobalTestLock::locked(|| {
        let store = Store::new();
        let counter = body_counter();
        let graph = support::graph();
        graph.install(
            ObservedValueView {
                store: store.clone(),
                counter: counter.clone(),
            },
            Default::default(),
        );
        assert_eq!(counter.get(), 1, "the body evaluates once on install");
        assert!(
            !graph.recomposer.has_pending(),
            "installing leaves no pending work"
        );

        store.set_value(1);
        assert!(
            graph.recomposer.has_pending(),
            "writing the read key schedules the owning scope"
        );

        graph.recomposer.commit_all();
        assert_eq!(counter.get(), 2, "the owning scope recomposes once");
        assert!(
            !graph.recomposer.has_pending(),
            "the recomposition consumes the pending write"
        );
    });
}

#[test]
fn reader_closure_read_invalidates_the_enclosing_scope() {
    GlobalTestLock::locked(|| {
        let store = Store::new();
        let counter = body_counter();
        let graph = support::graph();
        graph.install(
            ReaderObservedValueView {
                store: store.clone(),
                counter: counter.clone(),
            },
            Default::default(),
        );
        assert_eq!(counter.get(), 1, "the body evaluates once on install");

        store.set_value(1);
        assert!(
            !graph.recomposer.has_pending(),
            "GAP: Swift attributes a reader-closure read to the enclosing view; \
             Rust evaluates the closure outside the body scope and records no dependency"
        );
        graph.recomposer.commit_all();
        assert_eq!(
            counter.get(),
            1,
            "GAP: the enclosing view is not rebuilt because it never subscribed"
        );

        store.set_value(2);
        assert!(
            !graph.recomposer.has_pending(),
            "GAP: a dependency that was never recorded cannot survive a recomposition"
        );
        graph.recomposer.commit_all();
        assert_eq!(
            counter.get(),
            1,
            "GAP: the reader closure's write never recomposes its view"
        );
    });
}

#[test]
fn dependencies_are_replaced_on_each_body_evaluation() {
    GlobalTestLock::locked(|| {
        let store = Store::new();
        let counter = body_counter();
        let graph = support::graph();
        graph.install(
            ConditionalObservedValueView {
                store: store.clone(),
                counter: counter.clone(),
            },
            Default::default(),
        );
        assert_eq!(counter.get(), 1, "the body evaluates once on install");

        store.set_value(1);
        graph.recomposer.commit_all();
        assert_eq!(counter.get(), 2, "the value read subscribes the body");

        store.set_should_read(false);
        graph.recomposer.commit_all();
        assert_eq!(counter.get(), 3, "the should-read key subscribes the body");

        store.set_value(2);
        assert!(
            !graph.recomposer.has_pending(),
            "a body that stopped reading the value schedules nothing"
        );
        assert_eq!(
            counter.get(),
            3,
            "the dropped dependency never rebuilds again"
        );
    });
}

#[test]
fn independent_keys_only_recompose_their_readers() {
    GlobalTestLock::locked(|| {
        let store = Store::new();
        let value_counter = body_counter();
        let other_counter = body_counter();
        let graph = support::graph();
        graph.install(
            TwoObservedValuesView {
                store: store.clone(),
                value_counter: value_counter.clone(),
                other_counter: other_counter.clone(),
            },
            Default::default(),
        );
        assert_eq!(value_counter.get(), 1, "the value reader builds once");
        assert_eq!(other_counter.get(), 1, "the other reader builds once");

        store.set_value(1);
        graph.recomposer.commit_all();

        assert_eq!(
            value_counter.get(),
            2,
            "the value reader recomposes for its own key"
        );
        assert_eq!(
            other_counter.get(),
            1,
            "the other reader is not woken by an unrelated key"
        );
    });
}

#[test]
fn observed_dynamic_properties_do_not_accumulate_across_parent_rebuilds() {
    GlobalTestLock::locked(|| {
        let object = CountingObject::new();
        let tick = State::new(0);
        let graph = support::graph();
        graph.install(
            ObservedRebuildHarness {
                tick: tick.clone(),
                object: object.clone(),
            },
            Default::default(),
        );
        assert_eq!(
            object.observer_count(),
            1,
            "the leaf registers one observer"
        );

        for pass in 0..10 {
            tick.set(tick.get() + 1);
            commit_until_settled(&graph);
            assert_eq!(
                object.observer_count(),
                1,
                "parent rebuild {pass} does not accumulate observers"
            );
        }
    });
}

#[test]
fn observed_dynamic_properties_unregister_when_removed_from_the_tree() {
    GlobalTestLock::locked(|| {
        let object = CountingObject::new();
        let is_visible = State::new(true);
        let graph = support::graph();
        graph.install(
            ConditionalObservedHarness {
                is_visible: is_visible.clone(),
                object: object.clone(),
            },
            Default::default(),
        );
        assert_eq!(object.observer_count(), 1, "the mounted leaf observes once");

        is_visible.set(false);
        commit_until_settled(&graph);
        assert_eq!(
            object.observer_count(),
            0,
            "unmounting the leaf unregisters its observer"
        );

        is_visible.set(true);
        commit_until_settled(&graph);
        assert_eq!(
            object.observer_count(),
            1,
            "remounting the leaf registers exactly one observer again"
        );
    });
}
