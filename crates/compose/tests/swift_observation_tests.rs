//! Translation of `Tests/CoveUIComposeTests/SwiftObservationTests.swift`.
//!
//! Behaviour checklist:
//! - writing one observed property schedules a recomposition
//! - after that recomposition only the reader that projects the property
//!   rebuilds; the root and the unrelated reader do not
//! - a second write to the same property rebuilds that reader again
//! - when a reader's projection stops depending on a property, writing that
//!   property schedules nothing
//! - a graph that detaches cancels its subscriptions, so later writes schedule
//!   nothing and no reader rebuilds
//! - a closure that reads an observed projection inside a reader rebuilds that
//!   reader when the projection changes
//!
//! Named difference from the Swift suite:
//! - Swift's `@Observable` macro records property reads automatically. Rust has
//!   no macro equivalent, so the model implements `ObservableObject` over an
//!   `ObservablePublisher` and each reader holds an `Observed` projection. That
//!   is the same contract the suite is about: a reader subscribes to exactly the
//!   properties its projection touches, a rebuild recomputes the projection, and
//!   a projection that stops touching a property stops waking the reader. The
//!   Swift suite's dependency-cleaner counter becomes the observable outcome
//!   ("writes schedule nothing") rather than an internal counter.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{Node, ObservableObject, ObservablePublisher};
use cove_ui_compose::Observed;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;

use support::GlobalTestLock;

/// The Swift `Model`: `first`, `second` and `readsFirst`.
struct Model {
    publisher: ObservablePublisher,
    first: Cell<i32>,
    second: Cell<i32>,
    reads_first: Cell<bool>,
}

impl Model {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            publisher: ObservablePublisher::default(),
            first: Cell::new(0),
            second: Cell::new(0),
            reads_first: Cell::new(true),
        })
    }

    fn set_first(&self, value: i32) {
        self.first.set(value);
        self.publisher.send();
    }

    fn set_second(&self, value: i32) {
        self.second.set(value);
        self.publisher.send();
    }

    fn set_reads_first(&self, value: bool) {
        self.reads_first.set(value);
        self.publisher.send();
    }
}

impl ObservableObject for Model {
    fn register_observer(&self, handler: Rc<dyn Fn()>) -> u64 {
        self.publisher.register_observer(handler)
    }
    fn unregister_observer(&self, token: u64) {
        self.publisher.unregister_observer(token);
    }
}

/// Leaf that records the projected value it was built with.
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

/// The Swift `Reader`: counts its own rebuilds and reads its projection.
struct Reader {
    builds: Rc<Cell<usize>>,
    observed: Observed<Model, i32>,
}

impl View for Reader {
    fn body(&self) -> AnyView {
        self.builds.set(self.builds.get() + 1);
        AnyView::new(Probe {
            value: self.observed.get(),
        })
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![self.observed.dynamic_field("value")]
    }
}

fn projection(first: &Model) -> i32 {
    if first.reads_first.get() {
        first.first.get()
    } else {
        0
    }
}

fn second_projection(model: &Model) -> i32 {
    model.second.get()
}

#[test]
fn only_readers_recompose_and_dependencies_are_replaced() {
    GlobalTestLock::locked(|| {
        let model = Model::new();
        let root_builds = Rc::new(Cell::new(0));
        let first_builds = Rc::new(Cell::new(0));
        let second_builds = Rc::new(Cell::new(0));

        struct Root {
            first: Observed<Model, i32>,
            second: Observed<Model, i32>,
            root_builds: Rc<Cell<usize>>,
            first_builds: Rc<Cell<usize>>,
            second_builds: Rc<Cell<usize>>,
        }

        impl View for Root {
            fn body(&self) -> AnyView {
                self.root_builds.set(self.root_builds.get() + 1);
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(Reader {
                        builds: self.first_builds.clone(),
                        observed: self.first.clone(),
                    }),
                    AnyView::new(Reader {
                        builds: self.second_builds.clone(),
                        observed: self.second.clone(),
                    }),
                ]))
            }
        }

        let graph = support::graph();
        graph.install_view(
            AnyView::new(Root {
                first: Observed::new(model.clone(), projection),
                second: Observed::new(model.clone(), second_projection),
                root_builds: root_builds.clone(),
                first_builds: first_builds.clone(),
                second_builds: second_builds.clone(),
            }),
            Default::default(),
        );

        assert_eq!(root_builds.get(), 1, "the root builds once");
        let first_at_start = first_builds.get();
        let second_at_start = second_builds.get();
        assert_eq!(first_at_start, 1, "the first reader builds once");
        assert_eq!(second_at_start, 1, "the second reader builds once");

        model.set_first(1);
        assert!(
            graph.recomposer.has_pending(),
            "writing an observed property schedules a recomposition"
        );
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

        model.set_first(2);
        graph.recomposer.commit_all();
        assert_eq!(
            first_builds.get(),
            first_at_start + 2,
            "the first reader keeps rebuilding on its own property"
        );
        assert_eq!(
            second_builds.get(),
            second_at_start,
            "the second reader is still untouched"
        );

        // The projection stops reading `first`; later writes must not wake it.
        model.set_reads_first(false);
        graph.recomposer.commit_all();
        let first_after_switch = first_builds.get();
        model.set_first(3);
        assert!(
            !graph.recomposer.has_pending(),
            "a property the projection no longer reads schedules nothing"
        );
        graph.recomposer.commit_all();
        assert_eq!(
            first_builds.get(),
            first_after_switch,
            "the reader no longer depends on the dropped property"
        );

        // Detaching the tree drops every subscription.
        graph.install_view(
            AnyView::new(cove_composition::EmptyView),
            Default::default(),
        );
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
        let model = Model::new();
        let builds = Rc::new(Cell::new(0));

        struct Root {
            observed: Observed<Model, i32>,
            builds: Rc<Cell<usize>>,
        }

        impl View for Root {
            fn body(&self) -> AnyView {
                self.builds.set(self.builds.get() + 1);
                let width = (self.observed.get() + 1) as f32;
                AnyView::new(
                    Probe {
                        value: self.observed.get(),
                    }
                    .frame_width(width)
                    .frame_height(1.0),
                )
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![self.observed.dynamic_field("first")]
            }
        }

        let graph = support::graph();
        graph.install_view(
            AnyView::new(Root {
                observed: Observed::new(model.clone(), |model: &Model| model.first.get()),
                builds: builds.clone(),
            }),
            Default::default(),
        );
        assert_eq!(builds.get(), 1, "the reader builds once");

        model.set_first(10);
        graph.recomposer.commit_all();
        assert_eq!(
            builds.get(),
            2,
            "a closure reading the projection rebuilds the reader exactly once"
        );
    });
}

#[test]
fn detaching_the_graph_cancels_subscriptions() {
    GlobalTestLock::locked(|| {
        let model = Model::new();
        let builds = Rc::new(Cell::new(0));

        struct Root {
            observed: Observed<Model, i32>,
            builds: Rc<Cell<usize>>,
        }

        impl View for Root {
            fn body(&self) -> AnyView {
                self.builds.set(self.builds.get() + 1);
                AnyView::new(Probe {
                    value: self.observed.get(),
                })
            }
            fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
                vec![self.observed.dynamic_field("first")]
            }
        }

        let observed = Observed::new(model.clone(), |model: &Model| model.first.get());
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Root {
                observed,
                builds: builds.clone(),
            }),
            Default::default(),
        );
        assert_eq!(builds.get(), 1, "the reader builds once");

        graph.install_view(
            AnyView::new(cove_composition::EmptyView),
            Default::default(),
        );
        model.set_first(1);
        assert!(
            !graph.recomposer.has_pending(),
            "writes no longer schedule work after the graph detaches"
        );
        let after = builds.get();
        graph.recomposer.commit_all();
        assert_eq!(builds.get(), after, "the detached reader never rebuilds");
    });
}
