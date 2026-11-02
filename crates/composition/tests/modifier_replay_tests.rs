//! A caller's modifier on a transparent component replays after the component
//! rebuilds its own body.
//!
//! Behaviour checklist:
//! - one local recomposition replays each modifier declaration exactly once
//! - the replay list does not grow with the number of recompositions
//! - a removed component's node is released, along with its replay list

use cove_composition::Scope;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_composition::{AnyView, DynamicField, View, ViewExt, ViewGraph, ViewModifier};
use cove_core::State;
use cove_core::scene::{Node, NodeTree, NodeWeak, Recomposer};

fn locked<T>(body: impl FnOnce() -> T) -> T {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard: MutexGuard<'_, ()> = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    body()
}

/// Counts how many times the graph applies it, and remembers where.
#[derive(Clone, Default)]
struct Probe {
    applied: Rc<Cell<usize>>,
    nodes: Rc<RefCell<Vec<NodeWeak>>>,
}

impl ViewModifier for Probe {
    fn apply_node(&self, node: &Node) {
        self.applied.set(self.applied.get() + 1);
        // The written node and the transparent component that holds the replay
        // list. Anything higher belongs to `Root`, which stays mounted.
        self.nodes.borrow_mut().push(node.downgrade());
        if let Some(component) = node.parent() {
            self.nodes.borrow_mut().push(component.downgrade());
        }
    }
}

/// Has no layout node of its own, so its caller's modifiers replay.
struct Transparent {
    value: State<usize>,
}

impl View for Transparent {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _ = self.value.get();
        AnyView::new(cove_composition::Spacer::new(0.0))
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("value", &self.value)]
    }
}

struct Root {
    shows: State<bool>,
    value: State<usize>,
    probe: Probe,
}

impl View for Root {
    fn body(&self, _cx: &Scope) -> AnyView {
        if !self.shows.get() {
            return AnyView::new(cove_composition::EmptyView);
        }
        AnyView::new(
            Transparent {
                value: self.value.clone(),
            }
            .modifier(self.probe.clone()),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("shows", &self.shows)]
    }
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

#[test]
fn each_recomposition_replays_a_modifier_once() {
    locked(|| {
        let graph = graph();
        let value = State::new(0);
        let probe = Probe::default();
        let applied = probe.applied.clone();
        graph.install(
            Root {
                shows: State::new(true),
                value: value.clone(),
                probe,
            },
            Default::default(),
        );
        graph.recomposer.commit_all();

        // Unbounded growth doubles the work per pass, so forty passes never end.
        for pass in 1..=40 {
            let before = applied.get();
            value.set(pass);
            graph.recomposer.commit_all();
            assert!(
                applied.get() - before <= 2,
                "pass {pass} applied the modifier {} times",
                applied.get() - before
            );
        }
    });
}

#[test]
fn a_removed_component_releases_its_node() {
    locked(|| {
        let graph = graph();
        let shows = State::new(true);
        let value = State::new(0);
        let probe = Probe::default();
        let nodes = probe.nodes.clone();
        graph.install(
            Root {
                shows: shows.clone(),
                value: value.clone(),
                probe,
            },
            Default::default(),
        );
        graph.recomposer.commit_all();
        value.set(1);
        graph.recomposer.commit_all();
        assert!(!nodes.borrow().is_empty(), "the modifier ran");

        shows.set(false);
        graph.recomposer.commit_all();
        let alive = nodes
            .borrow()
            .iter()
            .filter(|weak| Node::upgrade(weak).is_some())
            .count();
        assert_eq!(alive, 0, "{alive} handles still reach a live node");
    });
}
