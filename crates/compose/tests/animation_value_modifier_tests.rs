//! Translation of `Tests/CoveUIComposeTests/AnimationValueModifierTests.swift`.
//!
//! Behaviour checklist:
//! - the first materialisation snaps even when an animation is supplied
//! - a later value change animates implicitly and lands on the target
//! - `animation(nil, value:)` writes instantly even on change
//! - `frame(width:)`, `frame(maxWidth:)`, `frame(maxHeight:)` animate implicitly
//! - `frame(width: .percent())` and `frame(height: .percent())` animate implicitly

use cove_composition::Scope;
use crate::support;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::State;
use cove_core::scene::{Animation, AnimationCurve, AnimatorScheduler, LayoutNode, Node};
use cove_ui_compose::modifiers::animation_value::AnimationValueExt;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::modifiers::style::AppearanceExt;

use support::{DebugNode, GlobalTestLock};

/// A leaf with a layout node, standing in for Swift's `_DebugNode` where a
/// frame modifier needs something to write geometry onto.
#[derive(Clone)]
struct SizedLeaf {
    label: &'static str,
}

impl cove_composition::PrimitiveView for SizedLeaf {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_view_tag(Some(self.label.to_string()));
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_view_tag(Some(self.label.to_string()));
    }
}

cove_composition::impl_primitive_view!(SizedLeaf);

/// The animation modifier wraps its content in a synthetic anchor, so the
/// `DebugNode` leaf sits below the non-hit-testable wrappers.
fn find_leaf(root: &Node) -> Option<Node> {
    let mut cursor = root.children().first().cloned();
    while let Some(node) = cursor.clone() {
        if node.is_hit_testable() {
            break;
        }
        match node.children().first().cloned() {
            Some(next) => {
                let multiple = node.children().len() > 1;
                cursor = Some(next);
                if multiple {
                    break;
                }
            }
            None => break,
        }
    }
    // Final descent into the `_DebugNode` leaf if still wrapping anchors.
    while let Some(node) = cursor.clone() {
        if node.children().len() != 1 {
            break;
        }
        cursor = node.children().first().cloned();
    }
    cursor
}

fn leaf_layout(graph: &std::rc::Rc<cove_composition::ViewGraph>) -> LayoutNode {
    let root = graph.tree.borrow().root().expect("root");
    find_leaf(&root)
        .expect("the debug leaf mounts")
        .layout_node()
        .expect("the leaf owns a layout node")
}

struct OpacityHarness {
    alpha: State<f32>,
    animated: bool,
}

impl View for OpacityHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let alpha = self.alpha.get();
        let animation = if self.animated {
            Some(Animation::new(1.0, AnimationCurve::Linear))
        } else {
            None
        };
        AnyView::new(
            DebugNode::new("x")
                .opacity(alpha)
                .animation_value(animation, alpha),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("alpha", &self.alpha)]
    }
}

struct FrameWidthHarness {
    width: State<f32>,
}

impl View for FrameWidthHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let width = self.width.get();
        AnyView::new(
            SizedLeaf { label: "x" }
                .frame(
                    Some(FrameDimension::Points(width)),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .animation_value(Some(linear()), width),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("width", &self.width)]
    }
}

struct MaxWidthHarness {
    max_width: State<Option<f32>>,
}

impl View for MaxWidthHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let max_width = self.max_width.get();
        AnyView::new(
            SizedLeaf { label: "x" }
                .frame(None, None, None, None, max_width, None)
                .animation_value(Some(linear()), max_width),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("max_width", &self.max_width)]
    }
}

struct MaxHeightHarness {
    max_height: State<Option<f32>>,
}

impl View for MaxHeightHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let max_height = self.max_height.get();
        AnyView::new(
            SizedLeaf { label: "x" }
                .frame(None, None, None, None, None, max_height)
                .animation_value(Some(linear()), max_height),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("max_height", &self.max_height)]
    }
}

struct PercentWidthHarness {
    ratio: State<f32>,
}

impl View for PercentWidthHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let ratio = self.ratio.get();
        AnyView::new(
            SizedLeaf { label: "x" }
                .frame(
                    Some(FrameDimension::Percent(ratio)),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .animation_value(Some(linear()), ratio),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("ratio", &self.ratio)]
    }
}

struct PercentHeightHarness {
    ratio: State<f32>,
}

impl View for PercentHeightHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let ratio = self.ratio.get();
        AnyView::new(
            SizedLeaf { label: "x" }
                .frame(
                    None,
                    Some(FrameDimension::Percent(ratio)),
                    None,
                    None,
                    None,
                    None,
                )
                .animation_value(Some(linear()), ratio),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("ratio", &self.ratio)]
    }
}

fn linear() -> Animation {
    Animation::new(1.0, AnimationCurve::Linear)
}

#[test]
fn initial_render_snaps() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(OpacityHarness {
                    alpha: State::new(0.0),
                    animated: true,
                }),
                Default::default(),
            );

            let root = graph.tree.borrow().root().expect("root");
            let leaf = find_leaf(&root).expect("the debug leaf mounts");
            assert_eq!(
                leaf.opacity(),
                0.0,
                "the initial pass writes the authored opacity without animating"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "the first materialisation registers no animation"
            );
        });
    });
}

#[test]
fn value_change_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let alpha = State::new(0.0);
            graph.install_view(
                AnyView::new(OpacityHarness {
                    alpha: alpha.clone(),
                    animated: true,
                }),
                Default::default(),
            );
            let root = graph.tree.borrow().root().expect("root");
            let leaf = find_leaf(&root).expect("the debug leaf mounts");

            alpha.set(1.0);
            graph.recomposer.commit_all();

            assert_eq!(leaf.opacity(), 0.0, "the change starts from the old value");
            assert_eq!(
                scheduler.active_count(),
                1,
                "changing the keyed value registers one animation"
            );

            scheduler.tick(0.5);
            assert_eq!(
                leaf.opacity(),
                0.5,
                "halfway through a linear 1s animation the opacity is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(leaf.opacity(), 1.0, "the animation lands on the new value");
        });
    });
}

#[test]
fn nil_opts_out() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let alpha = State::new(0.0);
            graph.install_view(
                AnyView::new(OpacityHarness {
                    alpha: alpha.clone(),
                    animated: false,
                }),
                Default::default(),
            );
            let root = graph.tree.borrow().root().expect("root");
            let leaf = find_leaf(&root).expect("the debug leaf mounts");

            alpha.set(1.0);
            graph.recomposer.commit_all();

            assert_eq!(
                leaf.opacity(),
                1.0,
                "a nil animation writes the new value instantly"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "a nil animation registers nothing"
            );
        });
    });
}

#[test]
fn implicit_frame_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let width = State::new(40.0);
            graph.install_view(
                AnyView::new(FrameWidthHarness {
                    width: width.clone(),
                }),
                Default::default(),
            );
            let layout = leaf_layout(&graph);

            width.set(80.0);
            graph.recomposer.commit_all();

            assert_eq!(
                layout.width(),
                Some(40.0),
                "the frame starts at the old width"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is active");

            scheduler.tick(0.5);
            assert_eq!(
                layout.width(),
                Some(60.0),
                "halfway through a linear animation the width is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(layout.width(), Some(80.0), "the width lands on the target");
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn implicit_max_frame_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let max_width = State::new(Some(120.0));
            graph.install_view(
                AnyView::new(MaxWidthHarness {
                    max_width: max_width.clone(),
                }),
                Default::default(),
            );
            let layout = leaf_layout(&graph);

            max_width.set(Some(200.0));
            graph.recomposer.commit_all();

            assert_eq!(
                layout.max_width(),
                Some(120.0),
                "the maximum starts at the old value"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is active");

            scheduler.tick(0.5);
            assert_eq!(
                layout.max_width(),
                Some(160.0),
                "halfway through, the maximum is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(
                layout.max_width(),
                Some(200.0),
                "the maximum lands on the target"
            );
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn implicit_max_height_frame_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let max_height = State::new(Some(40.0));
            graph.install_view(
                AnyView::new(MaxHeightHarness {
                    max_height: max_height.clone(),
                }),
                Default::default(),
            );
            let layout = leaf_layout(&graph);

            max_height.set(Some(100.0));
            graph.recomposer.commit_all();

            assert_eq!(
                layout.max_height(),
                Some(40.0),
                "the maximum starts at the old value"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is active");

            scheduler.tick(0.5);
            assert_eq!(
                layout.max_height(),
                Some(70.0),
                "halfway through, the maximum is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(
                layout.max_height(),
                Some(100.0),
                "the maximum lands on the target"
            );
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn implicit_percent_frame_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let ratio = State::new(20.0);
            graph.install_view(
                AnyView::new(PercentWidthHarness {
                    ratio: ratio.clone(),
                }),
                Default::default(),
            );

            ratio.set(60.0);
            graph.recomposer.commit_all();

            assert_eq!(scheduler.active_count(), 1, "one animation is active");
            scheduler.tick(1.0);
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn implicit_percent_height_frame_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let ratio = State::new(20.0);
            graph.install_view(
                AnyView::new(PercentHeightHarness {
                    ratio: ratio.clone(),
                }),
                Default::default(),
            );

            ratio.set(60.0);
            graph.recomposer.commit_all();

            assert_eq!(scheduler.active_count(), 1, "one animation is active");
            scheduler.tick(1.0);
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}
