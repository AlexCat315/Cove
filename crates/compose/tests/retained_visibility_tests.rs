//! Translation of `Tests/CoveUIComposeTests/RetainedVisibilityTests.swift`.
//!
//! Behaviour checklist:
//! - a retained custom painter that paints its intrinsic size draws nothing
//!   while `AnimatedVisibility` is hidden and paints again when shown, for both
//!   an identity exit and a movement exit
//! - direct rendering and the layer-cached renderer agree, including after
//!   re-showing
//! - the painter keeps the same element identity across every toggle

use crate::support;
use cove_composition::Scope;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, PrimitiveView, View, ViewExt};
use cove_core::scene::{LayerAwareNodeRenderer, LayoutNode, Node};
use cove_core::{Color, DrawList, Rect, State};
use cove_ui_compose::animation::{AnimatedVisibility, Transition, TransitionEdge};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::GlobalTestLock;

/// Paints a red rectangle at its own origin, ignoring its zero frame.
#[derive(Clone)]
struct Painter {
    paints: Rc<Cell<u32>>,
}

impl PrimitiveView for Painter {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let paints = self.paints.clone();
        node.update_draw("retained-painter", move |list, origin| {
            paints.set(paints.get() + 1);
            let _ = list.add_rect(
                Rect::new(origin.x, origin.y, 40.0, 20.0),
                Color::from_rgba8(255, 0, 0, 255),
            );
        });
    }
}
cove_ui_compose::impl_primitive_view!(Painter);

/// The Swift `Harness`: one `@State` flag driving retained visibility.
struct Harness {
    visible: State<bool>,
    transition: Transition,
    paints: Rc<Cell<u32>>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let painter = AnyView::new(
            AnyView::new(Painter {
                paints: self.paints.clone(),
            })
            .frame_width(40.0)
            .frame_height(20.0)
            .debug_name("retained-painter"),
        );
        AnyView::new(
            AnimatedVisibility::new(
                self.visible.get(),
                self.transition,
                None,
                false,
                None,
                painter,
            )
            .retaining_content(true)
            .reduced_motion(true),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("visible", &self.visible)]
    }
}

fn painted_vertices(list: &DrawList) -> usize {
    list.vertices()
        .iter()
        .filter(|vertex| vertex.color >> 24 > 0)
        .count()
}

#[test]
fn retained_painters_hide_on_identity_and_movement_exits() {
    GlobalTestLock::locked(|| {
        for transition in [
            Transition::identity(),
            Transition::move_edge(TransitionEdge::Leading, 10.0),
        ] {
            let visible = State::new(false);
            let paints = Rc::new(Cell::new(0));
            let harness = Harness {
                visible: visible.clone(),
                transition,
                paints: paints.clone(),
            };
            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());

            let mut last_id = None;
            for shown in [false, true, false, true] {
                visible.set(shown);
                for _ in 0..3 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(160.0, 100.0);
                }

                let node = graph
                    .tree
                    .borrow()
                    .root()
                    .and_then(|root| support::find_named(&root, "retained-painter"))
                    .expect("the painter stays retained while hidden");
                if let Some(previous) = last_id {
                    assert_eq!(
                        node.id(),
                        previous,
                        "the painter keeps its element identity across a toggle"
                    );
                }
                last_id = Some(node.id());

                let root = graph.tree.borrow().root().expect("root");
                let mut direct = DrawList::default();
                cove_core::scene::NodeRenderer::new().render(&root, &mut direct);
                assert_eq!(
                    painted_vertices(&direct) > 0,
                    shown,
                    "direct rendering paints only while visible (shown={shown}, transition {transition:?})"
                );

                // A second render exercises the cached layer path, including
                // reuse after re-showing.
                let mut renderer = LayerAwareNodeRenderer::new();
                for pass in 0..2 {
                    let mut cached = DrawList::default();
                    renderer.render(graph.render_tree(), &mut cached);
                    assert_eq!(
                        painted_vertices(&cached) > 0,
                        shown,
                        "cached rendering paints only while visible \
                         (shown={shown}, pass={pass}, transition {transition:?})"
                    );
                }
            }
        }
    });
}
