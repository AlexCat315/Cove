//! Translation of `Tests/CoveUIComposeTests/RecompositionPaintTests.swift`.
//!
//! Behaviour checklist:
//! - a draw closure keyed by an identity is reused while the identity is unchanged
//! - recomposing because an unrelated state (`tick`) changed does not repaint
//! - changing the value that feeds the draw identity repaints

use cove_composition::Scope;
use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_core::scene::{LayerAwareNodeRenderer, LayoutNode, Node};
use cove_core::{Color, DrawList, Rect, State};
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::{AnyView, PrimitiveView};

use support::{GlobalTestLock, with_test_text_environment};

/// Primitive whose draw closure is keyed by `value` and counts repaints.
#[derive(Clone)]
struct Painter {
    value: i32,
    counter: Rc<Cell<u32>>,
}

impl PrimitiveView for Painter {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn update_node(&self, node: &Node) {
        let counter = self.counter.clone();
        let value = self.value;
        node.update_draw(value, move |list, origin| {
            counter.set(counter.get() + 1);
            let _ = list.add_rect(Rect::new(origin.x, origin.y, 10.0, 10.0), Color::WHITE);
        });
    }
}
cove_ui_compose::impl_primitive_view!(Painter);

/// Harness with two observed states: `tick` triggers a recompose that does not
/// change the draw identity, `value` feeds the painter.
#[derive(Clone)]
struct Harness {
    tick: State<i32>,
    value: State<i32>,
    counter: Rc<Cell<u32>>,
}

impl PrimitiveView for Harness {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let _tick = self.tick.get();
        let value = self.value.get();
        vec![AnyView::new(
            Painter {
                value,
                counter: self.counter.clone(),
            }
            .frame(
                Some(FrameDimension::Points(10.0)),
                Some(FrameDimension::Points(10.0)),
                None,
                None,
                None,
                None,
            ),
        )]
    }
}
/// Harness is a *container* view, not a primitive: Swift's `Harness` is a plain
/// `View` whose `body` reads `@State var tick` and `value`, and reflection
/// auto-wires both so an observed write schedules a recompose.
/// `impl_primitive_view!` provides neither a `body` nor property discovery, so
/// the port has to declare the trait and publish the fields explicitly. Without
/// `dynamic_properties` the states are never subscribed and `value.set(1)`
/// silently invalidates nothing — that was the whole failure, a harness gap
/// rather than a production defect.
impl cove_composition::View for Harness {
    /// `body` reads both states, exactly like Swift's `Harness.body`. Reading
    /// `tick` is what makes an *unrelated* recompose happen without repainting.
    fn body(&self, _cx: &Scope) -> AnyView {
        let _tick = self.tick.get();
        let value = self.value.get();
        AnyView::new(
            Painter {
                value,
                counter: self.counter.clone(),
            }
            .frame(
                Some(FrameDimension::Points(10.0)),
                Some(FrameDimension::Points(10.0)),
                None,
                None,
                None,
                None,
            ),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("tick", &self.tick),
            cove_composition::DynamicField::state("value", &self.value),
        ]
    }
}

#[test]
fn unchanged_output_reuses_geometry_with_diagnostics_enabled() {
    GlobalTestLock::locked(|| {
        with_test_text_environment(|| {
            let counter = Rc::new(Cell::new(0));
            let tick = State::new(0);
            let value = State::new(0);
            let harness = Harness {
                tick: tick.clone(),
                value: value.clone(),
                counter: counter.clone(),
            };

            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            graph.compute_layout(100.0, 100.0);

            let mut renderer = LayerAwareNodeRenderer::new();
            let mut list = DrawList::default();
            for _ in 0..3 {
                renderer.render(graph.render_tree(), &mut list);
            }
            graph.tree.borrow_mut().flush();
            let paints = counter.get();

            tick.set(1);
            graph.recomposer.commit_all();
            renderer.render(graph.render_tree(), &mut list);
            assert_eq!(counter.get(), paints, "the unrelated tick does not repaint");
            assert!(
                !graph.tree.borrow().has_render_updates(),
                "no render updates remain"
            );

            value.set(1);
            graph.recomposer.commit_all();
            renderer.render(graph.render_tree(), &mut list);
            assert!(counter.get() > paints, "the new draw identity repaints");
        });
    });
}
