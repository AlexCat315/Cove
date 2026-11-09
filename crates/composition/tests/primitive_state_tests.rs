//! A primitive that declares `dynamic_properties` re-runs `children()` when that
//! state changes, wherever it is mounted: as the root, under a modifier, or as
//! a descendant of a user view that does not read the state itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_composition::{
    AnyView, DynamicField, EmptyView, LayoutDebugExt, PrimitiveView, Scope, TupleView, View,
    ViewGraph, ViewKind,
};
use cove_core::State;
use cove_core::scene::{Node, NodeTree, Recomposer};

fn locked<T>(body: impl FnOnce() -> T) -> T {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard: MutexGuard<'_, ()> = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    body()
}

type Log = Rc<RefCell<Vec<i32>>>;

/// Reads `value` in `children()` and records every evaluation.
struct Reader {
    value: State<i32>,
    log: Log,
}

impl PrimitiveView for Reader {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        self.log.borrow_mut().push(self.value.get());
        vec![AnyView::new(EmptyView)]
    }
}

impl View for Reader {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("value", &self.value)]
    }
}

/// A user view whose body never reads the reader's state, so only the reader's
/// own dependency can refresh it.
struct Host {
    revision: State<i32>,
    value: State<i32>,
    log: Log,
    bodies: Rc<Cell<usize>>,
}

impl View for Host {
    fn body(&self, _cx: &Scope) -> AnyView {
        self.bodies.set(self.bodies.get() + 1);
        let _ = self.revision.get();
        AnyView::new(TupleView::new(vec![
            AnyView::new(EmptyView),
            AnyView::new(
                AnyView::new(Reader {
                    value: self.value.clone(),
                    log: self.log.clone(),
                })
                .debug_name("reader"),
            ),
        ]))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("revision", &self.revision)]
    }
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn last(log: &Log) -> Option<i32> {
    log.borrow().last().copied()
}

fn nodes(graph: &ViewGraph) -> usize {
    fn count(node: &Node) -> usize {
        1 + node.children().iter().map(count).sum::<usize>()
    }
    graph.tree.borrow().root().map_or(0, |root| count(&root))
}

#[test]
fn a_root_primitive_follows_its_state() {
    locked(|| {
        let value = State::new(1);
        let log: Log = Rc::default();
        let graph = graph();
        graph.install_view(
            AnyView::new(Reader {
                value: value.clone(),
                log: log.clone(),
            }),
            Default::default(),
        );
        value.set(2);
        graph.recomposer.commit_all();
        assert_eq!(last(&log), Some(2));
    });
}

#[test]
fn a_primitive_under_a_root_modifier_follows_its_state() {
    locked(|| {
        let value = State::new(1);
        let log: Log = Rc::default();
        let graph = graph();
        graph.install_view(
            AnyView::new(
                AnyView::new(Reader {
                    value: value.clone(),
                    log: log.clone(),
                })
                .debug_name("wrapped"),
            ),
            Default::default(),
        );
        assert_eq!(last(&log), Some(1));
        value.set(2);
        graph.recomposer.commit_all();
        assert_eq!(last(&log), Some(2));
    });
}

#[test]
fn a_nested_primitive_refreshes_without_its_parent() {
    locked(|| {
        let revision = State::new(0);
        let value = State::new(1);
        let log: Log = Rc::default();
        let bodies = Rc::new(Cell::new(0));
        let graph = graph();
        graph.install_view(
            AnyView::new(Host {
                revision: revision.clone(),
                value: value.clone(),
                log: log.clone(),
                bodies: bodies.clone(),
            }),
            Default::default(),
        );
        let installed_nodes = nodes(&graph);
        assert_eq!(bodies.get(), 1);

        value.set(2);
        graph.recomposer.commit_all();
        assert_eq!(last(&log), Some(2), "the reader re-runs on its own");
        assert_eq!(bodies.get(), 1, "the parent body is not re-evaluated");

        // A parent update reuses the same slot instead of nesting another one.
        revision.set(1);
        graph.recomposer.commit_all();
        assert_eq!(bodies.get(), 2);
        assert_eq!(nodes(&graph), installed_nodes);

        value.set(3);
        graph.recomposer.commit_all();
        assert_eq!(
            last(&log),
            Some(3),
            "the dependency survives a parent update"
        );
    });
}

/// Mounts the reader only while `shown` is true.
struct Toggle {
    shown: State<bool>,
    value: State<i32>,
    log: Log,
}

impl View for Toggle {
    fn body(&self, _cx: &Scope) -> AnyView {
        if self.shown.get() {
            AnyView::new(Reader {
                value: self.value.clone(),
                log: self.log.clone(),
            })
        } else {
            AnyView::new(EmptyView)
        }
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("shown", &self.shown)]
    }
}

#[test]
fn an_unmounted_primitive_stops_observing() {
    locked(|| {
        let shown = State::new(true);
        let value = State::new(1);
        let log: Log = Rc::default();
        let graph = graph();
        graph.install_view(
            AnyView::new(Toggle {
                shown: shown.clone(),
                value: value.clone(),
                log: log.clone(),
            }),
            Default::default(),
        );
        shown.set(false);
        graph.recomposer.commit_all();
        let evaluations = log.borrow().len();
        value.set(5);
        graph.recomposer.commit_all();
        assert_eq!(log.borrow().len(), evaluations);
    });
}

/// Reads the surface environment through its scope.
struct EnvironmentBody {
    log: Rc<RefCell<Vec<f32>>>,
}

impl View for EnvironmentBody {
    fn body(&self, cx: &Scope) -> AnyView {
        let environment =
            cx.composition_value(&cove_composition::adaptive::SurfaceEnvironment::key());
        self.log.borrow_mut().push(environment.viewport.width);
        AnyView::new(EmptyView)
    }
}

struct EnvironmentHost {
    width: State<f32>,
    log: Rc<RefCell<Vec<f32>>>,
}

impl View for EnvironmentHost {
    fn body(&self, _cx: &Scope) -> AnyView {
        use cove_composition::adaptive::SurfaceEnvironmentExt;
        let mut environment = cove_core::Environment::default();
        environment.viewport.width = self.width.get();
        AnyView::new(
            AnyView::new(EnvironmentBody {
                log: self.log.clone(),
            })
            .surface_environment(environment),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("width", &self.width)]
    }
}

#[test]
fn a_body_reads_composition_locals_where_it_is_mounted() {
    locked(|| {
        let width = State::new(393.0);
        let log: Rc<RefCell<Vec<f32>>> = Rc::default();
        let graph = graph();
        graph.install_view(
            AnyView::new(EnvironmentHost {
                width: width.clone(),
                log: log.clone(),
            }),
            Default::default(),
        );
        assert_eq!(log.borrow().last().copied(), Some(393.0));
        width.set(841.0);
        graph.recomposer.commit_all();
        assert_eq!(
            log.borrow().last().copied(),
            Some(841.0),
            "a changed local recomposes the body with the new value"
        );
        assert_eq!(
            Scope::detached()
                .composition_value(&cove_composition::adaptive::SurfaceEnvironment::key())
                .viewport
                .width,
            cove_core::Environment::default().viewport.width,
            "a detached scope reads the default"
        );
    });
}
