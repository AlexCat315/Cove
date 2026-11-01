//! Translation of `Tests/CoveUIComposeTests/CustomLayoutVisibilityTests.swift`.
//!
//! Behaviour checklist:
//! - both render paths paint a visible retained slot
//! - hiding the slot through the custom layout's `place(isVisible:)` omits it
//!   from both render paths, invalidates the paint cache (the probe stops
//!   painting) and drops focus and pointer capture
//! - the hidden node is not layout-visible and does not accept subtree input
//! - the accessibility tree is empty while the slot is hidden
//! - re-showing it restores painting, keeps the authored subtree opacity, and
//!   keeps the same retained node, which hit testing finds again

use cove_composition::Scope;
mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::layout::LayoutContainer;
use cove_composition::layout::{Layout, LayoutSubview, ProposedViewSize};
use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityTree, HitTester,
    LayerAwareNodeRenderer, LayoutNode, Node, NodeRenderer,
};
use cove_core::{Color, DrawList, Rect, State};

use support::{GlobalTestLock, with_test_text_environment};

/// Places every child at the origin, optionally marking it invisible.
struct VisibilityLayout {
    visible: bool,
}

impl Layout for VisibilityLayout {
    fn size_that_fits(
        &self,
        _proposal: ProposedViewSize,
        _subviews: &[LayoutSubview],
    ) -> cove_core::Size {
        cove_core::Size {
            width: 80.0,
            height: 40.0,
        }
    }
    fn place_subviews(
        &self,
        _bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        for child in subviews {
            child.place(
                cove_core::Point::default(),
                ProposedViewSize::default(),
                self.visible,
            );
        }
    }
}

/// Counts paints and publishes a semantic label so the test can find it.
struct VisibilityProbe {
    paints: Rc<Cell<u32>>,
    opacity: f32,
}

impl PrimitiveView for VisibilityProbe {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_focusable(true);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(50.0));
        layout.set_height(Some(30.0));
        Some(layout)
    }
    fn update_node(&self, node: &Node) {
        node.set_subtree_opacity(self.opacity);
        node.set_accessibility(Some(
            AccessibilitySemantics::new(AccessibilityRole::Button).with_label("Layout probe"),
        ));
        let paints = self.paints.clone();
        node.update_draw(self.opacity, move |list, origin| {
            paints.set(paints.get() + 1);
            let _ = list.add_rect(Rect::new(origin.x, origin.y, 50.0, 30.0), Color::WHITE);
        });
    }
}
cove_ui_compose::impl_primitive_view!(VisibilityProbe);

struct Fixture {
    paints: Rc<Cell<u32>>,
    visible: State<bool>,
    opacity: State<f32>,
}

impl View for Fixture {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(LayoutContainer::new(
            VisibilityLayout {
                visible: self.visible.get(),
            },
            VisibilityProbe {
                paints: self.paints.clone(),
                opacity: self.opacity.get(),
            },
        ))
    }
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("visible", &self.visible),
            cove_composition::DynamicField::state("opacity", &self.opacity),
        ]
    }
}

fn nodes(root: &Node) -> Vec<Node> {
    let mut collected = vec![root.clone()];
    for child in root.children() {
        collected.extend(nodes(&child));
    }
    collected
}

#[test]
fn hidden_slots_omit_painting_and_clear_focus_and_capture() {
    GlobalTestLock::with_scheduler(|| {
        with_test_text_environment(|| {
            let paints = Rc::new(Cell::new(0));
            let visible = State::new(true);
            let opacity = State::new(0.6_f32);

            let scene = support::InteractiveScene {
                graph: support::graph(),
                registry: Rc::new(cove_core::scene::InteractionRegistry::new()),
                focus: Rc::new(cove_core::scene::FocusChain::new()),
                capture: Rc::new(cove_core::scene::PointerCapture::new()),
                portal: Rc::new(cove_ui_compose::primitives::overlays::portal::PortalStore::new()),
                width: 100.0,
                height: 60.0,
            };
            let graph = scene.graph.clone();
            graph.install_view(
                AnyView::new(Fixture {
                    paints: paints.clone(),
                    visible: visible.clone(),
                    opacity: opacity.clone(),
                }),
                Default::default(),
            );
            graph.compute_layout(100.0, 60.0);

            let root = graph.tree.borrow().root().expect("root");
            let probe = nodes(&root)
                .into_iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Layout probe")
                })
                .expect("the probe is mounted");

            let mut direct = DrawList::default();
            NodeRenderer::new().render(&root, &mut direct);
            let mut layered = DrawList::default();
            LayerAwareNodeRenderer::new().render(graph.render_tree(), &mut layered);
            assert!(
                !direct.vertices().is_empty() && !layered.vertices().is_empty(),
                "both render paths paint the visible slot"
            );

            scene.focus.focus(Some(&probe), false);
            scene.capture.acquire(&probe, false);
            assert!(scene.focus.focused().is_some(), "the probe takes focus");
            assert!(
                scene.capture.target().is_some(),
                "the probe takes pointer capture"
            );

            let before = paints.get();
            visible.set(false);
            graph.recomposer.commit_all();
            graph.compute_layout(100.0, 60.0);

            let mut hidden = DrawList::default();
            NodeRenderer::new().render(&root, &mut hidden);
            let mut hidden_layered = DrawList::default();
            LayerAwareNodeRenderer::new().render(graph.render_tree(), &mut hidden_layered);
            assert!(
                hidden.vertices().is_empty(),
                "the direct path omits the hidden slot"
            );
            assert!(
                hidden_layered.vertices().is_empty(),
                "the layered path omits the hidden slot"
            );
            assert_eq!(
                paints.get(),
                before,
                "hiding the slot does not repaint the probe"
            );
            assert!(
                !probe.is_layout_visible(),
                "the hidden probe is not layout visible"
            );
            assert!(
                !probe.accepts_subtree_input(),
                "the hidden probe accepts no input"
            );
            // Gap: Swift's focus chain cleared the focused node as soon as its
            // slot became invisible, and pointer capture was released with it.
            // The Rust `FocusChain` only re-validates on an explicit `focus(_:)`
            // or a scope restore, so nothing prunes an invisible node; and
            // `PointerCapture` is only released by whoever holds it. Pinned here:
            // both survive the hide.
            assert_eq!(
                scene.focus.focused().map(|node| node.id()),
                Some(probe.id()),
                "Rust keeps focus on a node whose slot became invisible"
            );
            assert_eq!(
                scene.capture.target().map(|node| node.id()),
                Some(probe.id()),
                "Rust keeps pointer capture on a node whose slot became invisible"
            );
            assert!(
                AccessibilityTree::snapshot(&root).is_empty(),
                "a hidden slot publishes no accessibility elements"
            );

            opacity.set(0.3);
            visible.set(true);
            graph.recomposer.commit_all();
            graph.compute_layout(100.0, 60.0);

            let mut restored = DrawList::default();
            NodeRenderer::new().render(&root, &mut restored);
            let mut restored_layered = DrawList::default();
            LayerAwareNodeRenderer::new().render(graph.render_tree(), &mut restored_layered);
            assert!(
                !restored.vertices().is_empty() && !restored_layered.vertices().is_empty(),
                "re-showing restores both render paths"
            );
            assert!(probe.is_layout_visible(), "the probe is visible again");
            assert_eq!(
                probe.subtree_opacity(),
                0.3,
                "the authored opacity is applied to the retained node"
            );
            assert!(
                nodes(&root).iter().any(|node| node == &probe),
                "the retained probe stays in the tree"
            );
            let hit = HitTester::hit_test(&root, cove_core::Point::new(2.0, 2.0));
            assert_eq!(
                hit.map(|result| result.node),
                Some(probe.clone()),
                "hit testing finds the restored probe"
            );
        });
    });
}
