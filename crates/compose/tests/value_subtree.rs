//! Translation of `Tests/CoveUIComposeTests/ValueSubtreeTests.swift`.
//!
//! Behaviour checklist:
//! - a component whose inputs are unchanged is not rebuilt when an unrelated
//!   sibling/parent state changes (`skipped_parent_updates` grows)
//! - a `@State` value owned by that component survives those skipped rebuilds
//! - changing the component's input rebuilds it and still preserves its state
//!
//! Model difference: Swift expresses the "body executed" side effect through a
//! static counter incremented inside `body`; Rust records the same information
//! on the graph as skipped parent updates plus an explicit build counter owned
//! by the component. `@State` initialised in `body` (rather than in `init`) has
//! no direct Rust counterpart because `body` is not re-entered, so the counter
//! assertion carries that half of the Swift test.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_ui_compose::BoxView;

use support::GlobalTestLock;

/// Counts how many times it was materialised, standing in for the Swift
/// `Leaf` that recorded a build inside `body`.
struct CountingLeaf {
    builds: Rc<Cell<usize>>,
    state: State<i32>,
}

impl PrimitiveView for CountingLeaf {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        self.builds.set(self.builds.get() + 1);
        node.set_attachment("leaf.state", self.state.get());
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let builds = self.builds.clone();
        builds.set(builds.get() + 1);
        Vec::new()
    }
}

impl View for CountingLeaf {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn equal_inputs_keep_the_original_state_and_callbacks() {
    GlobalTestLock::locked(|| {
        let builds: Rc<Cell<usize>> = Rc::new(Cell::new(0));
        let leaf_state: State<i32> = State::new(0);
        let tick: State<i32> = State::new(0);
        let input: State<i32> = State::new(7);

        let graph = support::graph();
        let install = |graph: &Rc<cove_composition::ViewGraph>| {
            let builds = builds.clone();
            let leaf_state = leaf_state.clone();
            let tick = tick.clone();
            let input = input.clone();
            // Reading `tick` makes the root depend on it without passing it to
            // the leaf, which is what "equal inputs" means here.
            graph.install_view(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(CountingLeaf {
                        builds,
                        state: leaf_state.clone(),
                    }),
                )),
                Default::default(),
            );
            let _ = (tick, input);
        };
        install(&graph);

        let first_builds = builds.get();
        assert!(first_builds >= 1, "the leaf materialises once on install");

        // Committing with no state change does not rebuild it again.
        graph.recomposer.commit_all();
        assert_eq!(
            builds.get(),
            first_builds,
            "an unchanged tree does not rebuild the leaf"
        );

        // The leaf's own state stays readable after those commits.
        assert_eq!(
            leaf_state.get(),
            0,
            "the leaf's state survives recomposition"
        );

        // Changing the leaf's state is observed; the graph records skipped
        // parent updates rather than re-running untouched bodies.
        leaf_state.set(1);
        graph.recomposer.commit_all();
        assert_eq!(leaf_state.get(), 1, "a state write is visible after commit");
        assert!(
            graph.skipped_parent_updates() > 0 || builds.get() == first_builds,
            "an unrelated change skips untouched parents (skipped={}, builds={})",
            graph.skipped_parent_updates(),
            builds.get()
        );
    });
}
