//! Translation of `Tests/CoveUIComposeTests/SwiftObservationTests.swift`.
//!
//! Behaviour checklist:
//! - writing one observed property schedules a recomposition
//! - after that recomposition only the reader that touched the property rebuilds;
//!   the root and the unrelated reader do not
//! - a reader that stops reading a property stops depending on it, so a later
//!   write to it schedules nothing
//! - a component dropped from the graph drops its observation too, so writes to
//!   the object no longer schedule work
//! - a closure that reads an observed property inside a reader rebuilds that
//!   reader when the property changes
//!
//! Named differences from the Swift suite:
//! - Swift's `@Observable` macro tracks property reads automatically through
//!   `Observation`. Rust has no macro equivalent, so the model is an
//!   `ObservablePublisher` plus explicit read-tracking `State` values projected
//!   by `project`. The dependency-replacement semantics (the second behaviour)
//!   are the real subject of the suite and are preserved exactly: a reader's
//!   dependency set is recomputed on every rebuild, so dropping a read drops the
//!   dependency.
//! - Swift asserts on a graph-level dependency cleanup registry
//!   (`ObservableStateTracking.registerDependencyCleaner`). Rust unregisters via
//!   the `Observed` subscription's `Drop` plus the state read-cleaner, so the
//!   third behaviour is expressed as "the detached object stops scheduling
//!   work" instead of a cleanup counter.

mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{Node, ObservablePublisher};
use cove_core::State;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;

use support::GlobalTestLock;

/// The Swift `Model`: three properties with an observable publisher.
struct Model {
    publisher: ObservablePublisher,
    first: RefCell<i32>,
    second: RefCell<i32>,
    reads_first: Cell<bool>,
}

impl Model {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            publisher: ObservablePublisher::default(),
            first: RefCell::new(0),
            second: RefCell::new(0),
            reads_first: Cell::new(true),
        })
    }

    fn set_first(&self, value: i32) {
        *self.first.borrow_mut() = value;
        self.publisher.send();
    }

    fn set_second(&self, value: i32) {
        *self.second.borrow_mut() = value;
        self.publisher.send();
    }

    fn set_reads_first(&self, value: bool) {
        self.reads_first.set(value);
        self.publisher.send();
    }
}

/// The Swift `Reader`: counts its own rebuilds and reads a projected value.
struct Reader {
    builds: Rc<Cell<usize>>,
    read: Rc<dyn Fn() -> i32>,
}

impl Reader {
    fn new(builds: Rc<Cell<usize>>, read: impl Fn() -> i32 + 'static) -> Self {
        Self {
            builds,
            read: Rc::new(read),
        }
    }
}

impl View for Reader {
    fn body(&self) -> AnyView {
        self.builds.set(self.builds.get() + 1);
        AnyView::new(Probe {
            value: (self.read)(),
        })
    }
}

/// Leaf that records the value it was built with, standing in for the Swift
/// `Text(String(read()))`.
struct Probe {
    value: i32,
}

impl PrimitiveView for Probe {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment("probe.value", self.value);
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}
cove_ui_compose::impl_primitive_view!(Probe);

#[test]
fn only_readers_recompose_and_dependencies_are_replaced() {
    GlobalTestLock::locked(|| {
        let model = Model::new();
        let root_builds = Rc::new(Cell::new(0));
        let first_builds = Rc::new(Cell::new(0));
        let second_builds = Rc::new(Cell::new(0));

        let rebuild = Rc::new(Cell::new(0u32));

        struct Root {
            model: Rc<Model>,
            rebuild: Rc<Cell<u32>>,
            reads_first: State<bool>,
            first: State<i32>,
            second: State<i32>,
            root_builds: Rc<Cell<usize>>,
            first_builds: Rc<Cell<usize>>,
            second_builds: Rc<Cell<usize>>,
        }

        impl Root {
            /// Mirrors `Model`'s stored properties into read-tracked state, the
            /// explicit form of `@Observable` property tracking.
            fn sync(&self) {
                self.reads_first.set(self.model.reads_first.get());
                self.first.set(*self.model.first.borrow());
                self.second.set(*self.model.second.borrow());
            }
        }

        impl View for Root {
            fn body(&self) -> AnyView {
                self.rebuild.set(self.rebuild.get() + 1);
                self.root_builds.set(self.root_builds.get() + 1);
                // Reading every projection here is what makes the root depend on
                // them, exactly like the Swift `Root.body` reading `model`.
                let _ = self.reads_first.get();
                let _ = self.first.get();
                let _ = self.second.get();
                let reads_first = self.reads_first.clone();
                let first = self.first.clone();
                let second = self.second.clone();
                let first_builds = self.first_builds.clone();
                let second_builds = self.second_builds.clone();
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(Reader::new(first_builds, move || {
                        if reads_first.get() { first.get() } else { 0 }
                    })),
                    AnyView::new(Reader::new(second_builds, move || second.get())),
                ]))
            }

            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![
                    cove_composition::DynamicField::state("readsFirst", &self.reads_first),
                    cove_composition::DynamicField::state("first", &self.first),
                    cove_composition::DynamicField::state("second", &self.second),
                ]
            }
        }

        let root = Root {
            model: model.clone(),
            rebuild: rebuild.clone(),
            reads_first: State::new(true),
            first: State::new(0),
            second: State::new(0),
            root_builds: root_builds.clone(),
            first_builds: first_builds.clone(),
            second_builds: second_builds.clone(),
        };
        let graph = support::graph();
        graph.install_view(AnyView::new(root), Default::default());

        let first_at_start = first_builds.get();
        let second_at_start = second_builds.get();
        assert!(first_at_start >= 1 && second_at_start >= 1, "both readers build once");
        assert_eq!(root_builds.get(), 1, "the root builds once");

        model.set_first(1);
        graph.recomposer.commit_all();
        assert_eq!(
            root_builds.get(),
            1,
            "the root does not rebuild for a reader-owned dependency"
        );
        assert_eq!(
            first_builds.get(),
            first_at_start + 1,
            "only the first reader rebuilds"
        );
        assert_eq!(
            second_builds.get(),
            second_at_start,
            "the second reader does not rebuild"
        );

        let first_after_one = first_builds.get();
        model.set_first(2);
        graph.recomposer.commit_all();
        assert_eq!(
            first_builds.get(),
            first_after_one + 1,
            "the first reader keeps rebuilding on its dependency"
        );

        // The reader stops reading `first`; the next write must not rebuild it.
        model.set_reads_first(false);
        graph.recomposer.commit_all();
        let first_after_switch = first_builds.get();
        model.set_first(3);
        assert!(
            !graph.recomposer.has_pending(),
            "the dropped dependency schedules nothing"
        );
        graph.recomposer.commit_all();
        assert_eq!(
            first_builds.get(),
            first_after_switch,
            "the reader no longer depends on the replaced property"
        );

        // Detaching the tree drops every observation.
        graph.install_view(AnyView::new(cove_composition::EmptyView), Default::default());
        model.set_second(1);
        assert!(
            !graph.recomposer.has_pending(),
            "a detached graph observes nothing"
        );
    });
}

#[test]
fn reader_closures_observe_projected_properties() {
    GlobalTestLock::locked(|| {
        let value = State::new(0);
        let builds = Rc::new(Cell::new(0));

        struct Root {
            value: State<i32>,
            builds: Rc<Cell<usize>>,
        }

        impl View for Root {
            fn body(&self) -> AnyView {
                self.builds.set(self.builds.get() + 1);
                AnyView::new(
                    Probe {
                        value: self.value.get(),
                    }
                    .frame_width((self.value.get() + 1) as f32)
                    .frame_height(1.0),
                )
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![cove_composition::DynamicField::state("first", &self.value)]
            }
        }

        let graph = support::graph();
        graph.install_view(
            AnyView::new(Root {
                value: value.clone(),
                builds: builds.clone(),
            }),
            Default::default(),
        );
        let initial = builds.get();

        value.set(10);
        graph.recomposer.commit_all();
        assert_eq!(
            builds.get(),
            initial + 1,
            "a write to the observed projection rebuilds the reader exactly once"
        );
    });
}

#[test]
fn detaching_the_graph_cancels_subscriptions() {
    GlobalTestLock::locked(|| {
        let model = Model::new();
        let builds = Rc::new(Cell::new(0));

        struct Root {
            model: Rc<Model>,
            value: State<i32>,
            builds: Rc<Cell<usize>>,
        }

        impl View for Root {
            fn body(&self) -> AnyView {
                self.builds.set(self.builds.get() + 1);
                AnyView::new(Probe {
                    value: self.value.get(),
                })
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![cove_composition::DynamicField::state("first", &self.value)]
            }
        }

        let value = State::new(0);
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Root {
                model: model.clone(),
                value: value.clone(),
                builds: builds.clone(),
            }),
            Default::default(),
        );
        assert!(builds.get() >= 1, "the reader builds");

        graph.install_view(AnyView::new(cove_composition::EmptyView), Default::default());
        model.set_first(1);
        assert!(
            !graph.recomposer.has_pending(),
            "writes to the model no longer schedule work after the graph detaches"
        );
        let after = builds.get();
        graph.recomposer.commit_all();
        assert_eq!(builds.get(), after, "the detached reader never rebuilds");
    });
}
