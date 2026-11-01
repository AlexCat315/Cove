//! Translation of `Tests/CoveUIComposeTests/WithAnimationPropertiesTests.swift`.
//!
//! Behaviour checklist:
//! - `background_color` flows through `with_animation`: the node keeps the old
//!   colour until the scheduler ticks, is midway halfway and lands on the target
//! - `foreground_color` animates channel by channel the same way
//! - `corner_radius` animates from 0 through 5 to 10 across two half-second ticks
//! - writing the value a property already holds registers no controller
//! - `frame(width:)` animates the layout width 40 -> 60 -> 80
//! - `padding` animates the layout insets 0 -> 5 -> 10
//! - `frame(max_width:)` animates 120 -> 160 -> 200
//! - `frame(max_height:)` animates 40 -> 70 -> 100
//! - `frame(width: .percent())` registers a controller that outlives one tick
//!   and drains after the second
//! - `frame(height: .percent())` behaves identically on the vertical axis
//! - switching a frame axis from points to percent snaps and keeps no controller
//! - the typed `FrameDimension` width API animates points values
//! - the percent convenience setter animates a percent width to completion
//!
//! Named differences from the Swift suite:
//! - Swift's `framePercent(width:)` convenience has no Rust counterpart; the
//!   suite drives the same percent width through
//!   `frame(Some(FrameDimension::Percent(..)))`, which is the call that
//!   convenience forwards to.
//! - Swift reads the percent frame's private `__layout.frame.widthSpec`
//!   attachment. Rust's `FrameDimensionSpec` is private to the modifier, so the
//!   tests assert the measured `LayoutStyleValue::Percent` the spec writes and
//!   the scheduler lifecycle instead of attachment presence.

use cove_composition::Scope;
mod support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, PrimitiveView, View, ViewGraph};
use cove_core::scene::{
    Animation, AnimationCurve, AnimatorScheduler, LayoutNode, LayoutStyleValue, Node,
    with_animation,
};
use cove_core::{Color, State};
use cove_ui_compose::modifiers::layout::{EdgeInsets, FrameDimension, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;

use support::{DebugNode, GlobalTestLock};

/// A leaf that owns a layout node, standing in for Swift's `_DebugNode` where a
/// frame or padding modifier needs geometry to write onto.
#[derive(Clone)]
struct SizedLeaf {
    label: &'static str,
}

impl PrimitiveView for SizedLeaf {
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

fn linear() -> Option<Animation> {
    Some(Animation::new(1.0, AnimationCurve::Linear))
}

fn black() -> Color {
    Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }
}

/// The materialised leaf the harness authored, which carries the animated
/// property. The modifier wrapper rewrites the leaf's view tag, so the leaf is
/// identified structurally: it is the last node on the first-child chain.
fn leaf(graph: &Rc<ViewGraph>) -> Node {
    let root = graph.tree.borrow().root().expect("root");
    support::deepest_node(&root)
}

fn leaf_layout(graph: &Rc<ViewGraph>) -> LayoutNode {
    leaf(graph)
        .layout_node()
        .expect("the debug leaf owns a layout node")
}

fn fixture<V: View + 'static>(view: V) -> Rc<ViewGraph> {
    let graph = support::graph();
    graph.install_view(AnyView::new(view), Default::default());
    // The first pass materialises the subtree before the animated mutation.
    for _ in 0..5 {
        graph.recomposer.commit_all();
        graph.tree.borrow_mut().flush();
        graph.compute_layout(640.0, 400.0);
    }
    graph
}

struct BgHarness {
    color: State<Color>,
}

impl View for BgHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DebugNode::new("x").background_color(self.color.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("color", &self.color)]
    }
}

struct FgHarness {
    color: State<Color>,
}

impl View for FgHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DebugNode::new("x").foreground_color(self.color.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("color", &self.color)]
    }
}

struct CornerHarness {
    radius: State<f32>,
}

impl View for CornerHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DebugNode::new("x").corner_radius(self.radius.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("radius", &self.radius)]
    }
}

struct FrameWidthHarness {
    width: State<f32>,
}

impl View for FrameWidthHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.frame(
            Some(FrameDimension::Points(self.width.get())),
            None,
            None,
            None,
            None,
            None,
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("width", &self.width)]
    }
}

struct PaddingHarness {
    insets: State<EdgeInsets>,
}

impl View for PaddingHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.padding_insets(self.insets.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("insets", &self.insets)]
    }
}

struct MaxFrameHarness {
    max_width: State<Option<f32>>,
}

impl View for MaxFrameHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.frame(
            None,
            None,
            None,
            None,
            self.max_width.get(),
            None,
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("max_width", &self.max_width)]
    }
}

struct MaxHeightFrameHarness {
    max_height: State<Option<f32>>,
}

impl View for MaxHeightFrameHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.frame(
            None,
            None,
            None,
            None,
            None,
            self.max_height.get(),
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("max_height", &self.max_height)]
    }
}

struct PercentFrameHarness {
    ratio: State<f32>,
}

impl View for PercentFrameHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.frame(
            Some(FrameDimension::Percent(self.ratio.get())),
            None,
            None,
            None,
            None,
            None,
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("ratio", &self.ratio)]
    }
}

struct PercentHeightFrameHarness {
    ratio: State<f32>,
}

impl View for PercentHeightFrameHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.frame(
            None,
            Some(FrameDimension::Percent(self.ratio.get())),
            None,
            None,
            None,
            None,
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("ratio", &self.ratio)]
    }
}

struct WidthModeSwitchHarness {
    use_percent: State<bool>,
}

impl View for WidthModeSwitchHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        if self.use_percent.get() {
            AnyView::new(SizedLeaf { label: "x" }.frame(
                Some(FrameDimension::Percent(50.0)),
                None,
                None,
                None,
                None,
                None,
            ))
        } else {
            AnyView::new(SizedLeaf { label: "x" }.frame(
                Some(FrameDimension::Points(40.0)),
                None,
                None,
                None,
                None,
                None,
            ))
        }
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("use_percent", &self.use_percent)]
    }
}

struct HeightModeSwitchHarness {
    use_percent: State<bool>,
}

impl View for HeightModeSwitchHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        if self.use_percent.get() {
            AnyView::new(SizedLeaf { label: "x" }.frame(
                None,
                Some(FrameDimension::Percent(50.0)),
                None,
                None,
                None,
                None,
            ))
        } else {
            AnyView::new(SizedLeaf { label: "x" }.frame(
                None,
                Some(FrameDimension::Points(40.0)),
                None,
                None,
                None,
                None,
            ))
        }
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("use_percent", &self.use_percent)]
    }
}

struct TypedDimensionHarness {
    width: State<FrameDimension>,
}

impl View for TypedDimensionHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(SizedLeaf { label: "x" }.frame(
            Some(self.width.get()),
            None,
            None,
            None,
            None,
            None,
        ))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("width", &self.width)]
    }
}

#[test]
fn background_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let color = State::new(black());
            let graph = fixture(BgHarness {
                color: color.clone(),
            });
            let node = leaf(&graph);

            with_animation(linear(), || {
                color.set(Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                })
            });
            graph.recomposer.commit_all();
            assert_eq!(
                node.background_color().map(|color| color.r),
                Some(0.0),
                "the background starts at the authored colour"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                node.background_color().map(|color| color.r),
                Some(0.5),
                "halfway through a linear 1s animation the red channel is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(
                node.background_color().map(|color| color.r),
                Some(1.0),
                "the background lands on the target"
            );
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn foreground_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let color = State::new(black());
            let graph = fixture(FgHarness {
                color: color.clone(),
            });
            let node = leaf(&graph);

            with_animation(linear(), || {
                color.set(Color {
                    r: 1.0,
                    g: 0.5,
                    b: 0.0,
                    a: 1.0,
                })
            });
            graph.recomposer.commit_all();
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            let foreground = node.foreground_color().expect("a foreground colour is set");
            assert_eq!(
                foreground.r, 0.5,
                "halfway through, the red channel is midway"
            );
            assert_eq!(
                foreground.g, 0.25,
                "halfway through, the green channel is midway"
            );
        });
    });
}

#[test]
fn corner_radius_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let radius = State::new(0.0f32);
            let graph = fixture(CornerHarness {
                radius: radius.clone(),
            });
            let node = leaf(&graph);

            with_animation(linear(), || radius.set(10.0));
            graph.recomposer.commit_all();
            assert_eq!(node.corner_radius(), 0.0, "the radius starts at zero");
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                node.corner_radius(),
                5.0,
                "halfway through, the radius is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(node.corner_radius(), 10.0, "the radius lands on the target");
        });
    });
}

#[test]
fn equal_target_registers_nothing() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let radius = State::new(0.0f32);
            let graph = fixture(CornerHarness {
                radius: radius.clone(),
            });

            // Same value: with_animation is present but nothing changes.
            with_animation(linear(), || radius.set(0.0));
            graph.recomposer.commit_all();
            assert_eq!(
                scheduler.active_count(),
                0,
                "writing the value the property already holds registers no controller"
            );
        });
    });
}

#[test]
fn frame_width_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let width = State::new(40.0f32);
            let graph = fixture(FrameWidthHarness {
                width: width.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || width.set(80.0));
            graph.recomposer.commit_all();
            assert_eq!(
                layout.width(),
                Some(40.0),
                "the frame starts at the old width"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                layout.width(),
                Some(60.0),
                "halfway through, the width is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(layout.width(), Some(80.0), "the width lands on the target");
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn padding_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let insets = State::new(EdgeInsets::all(0.0));
            let graph = fixture(PaddingHarness {
                insets: insets.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || insets.set(EdgeInsets::all(10.0)));
            graph.recomposer.commit_all();
            assert_eq!(
                layout.attachment::<EdgeInsets>("__layout.padding.insets"),
                Some(EdgeInsets::all(0.0)),
                "the padding starts at the authored insets"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                layout.attachment::<EdgeInsets>("__layout.padding.insets"),
                Some(EdgeInsets::all(5.0)),
                "halfway through, the insets are midway"
            );

            scheduler.tick(0.5);
            assert_eq!(
                layout.attachment::<EdgeInsets>("__layout.padding.insets"),
                Some(EdgeInsets::all(10.0)),
                "the insets land on the target"
            );
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn frame_max_width_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let max_width = State::new(Some(120.0f32));
            let graph = fixture(MaxFrameHarness {
                max_width: max_width.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || max_width.set(Some(200.0)));
            graph.recomposer.commit_all();
            assert_eq!(
                layout.max_width(),
                Some(120.0),
                "the maximum starts at the old value"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

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
fn frame_max_height_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let max_height = State::new(Some(40.0f32));
            let graph = fixture(MaxHeightFrameHarness {
                max_height: max_height.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || max_height.set(Some(100.0)));
            graph.recomposer.commit_all();
            assert_eq!(
                layout.max_height(),
                Some(40.0),
                "the maximum starts at the old value"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

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
fn frame_width_ratio_dimension_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let ratio = State::new(20.0f32);
            let graph = fixture(PercentFrameHarness {
                ratio: ratio.clone(),
            });
            let layout = leaf_layout(&graph);
            assert_eq!(
                layout.style().width,
                LayoutStyleValue::Percent(20.0),
                "the percent width spec is authored on first materialisation"
            );

            with_animation(linear(), || ratio.set(60.0));
            graph.recomposer.commit_all();
            assert_eq!(
                layout.style().width,
                LayoutStyleValue::Percent(20.0),
                "the percent width starts at the old ratio"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                scheduler.active_count(),
                1,
                "the controller outlives a half-second tick"
            );

            scheduler.tick(0.5);
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
            assert_eq!(
                layout.style().width,
                LayoutStyleValue::Percent(60.0),
                "the percent width lands on the target"
            );
        });
    });
}

#[test]
fn frame_height_ratio_dimension_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let ratio = State::new(20.0f32);
            let graph = fixture(PercentHeightFrameHarness {
                ratio: ratio.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || ratio.set(60.0));
            graph.recomposer.commit_all();
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                scheduler.active_count(),
                1,
                "the controller outlives a half-second tick"
            );

            scheduler.tick(0.5);
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
            assert_eq!(
                layout.style().height,
                LayoutStyleValue::Percent(60.0),
                "the percent height lands on the target"
            );
        });
    });
}

#[test]
fn frame_width_mode_switch_snaps() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let use_percent = State::new(false);
            let graph = fixture(WidthModeSwitchHarness {
                use_percent: use_percent.clone(),
            });

            with_animation(linear(), || use_percent.set(true));
            graph.recomposer.commit_all();
            assert_eq!(
                scheduler.active_count(),
                0,
                "a points-to-percent mode change is non-interpolable and snaps"
            );
        });
    });
}

#[test]
fn frame_height_mode_switch_snaps() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let use_percent = State::new(false);
            let graph = fixture(HeightModeSwitchHarness {
                use_percent: use_percent.clone(),
            });

            with_animation(linear(), || use_percent.set(true));
            graph.recomposer.commit_all();
            assert_eq!(
                scheduler.active_count(),
                0,
                "a points-to-percent mode change is non-interpolable and snaps"
            );
        });
    });
}

#[test]
fn typed_dimension_points_animate() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let width = State::new(FrameDimension::Points(40.0));
            let graph = fixture(TypedDimensionHarness {
                width: width.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || width.set(FrameDimension::Points(80.0)));
            graph.recomposer.commit_all();
            assert_eq!(
                layout.width(),
                Some(40.0),
                "the frame starts at the old width"
            );
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(0.5);
            assert_eq!(
                layout.width(),
                Some(60.0),
                "halfway through, the width is midway"
            );

            scheduler.tick(0.5);
            assert_eq!(layout.width(), Some(80.0), "the width lands on the target");
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn frame_percent_convenience_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let ratio = State::new(20.0f32);
            let graph = fixture(PercentFrameHarness {
                ratio: ratio.clone(),
            });
            let layout = leaf_layout(&graph);

            with_animation(linear(), || ratio.set(60.0));
            graph.recomposer.commit_all();
            assert_eq!(scheduler.active_count(), 1, "one animation is running");

            scheduler.tick(1.0);
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
            assert_eq!(
                layout.style().width,
                LayoutStyleValue::Percent(60.0),
                "the percent width reaches the target ratio"
            );
        });
    });
}
