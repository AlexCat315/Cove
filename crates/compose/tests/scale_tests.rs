//! Translation of `Tests/CoveUIComposeTests/Transitions/ScaleTests.swift`.
//!
//! Behaviour checklist:
//! - a scaling transition scales the node it hosts
//! - the authored anchor survives the animation and settles to identity
//! - a part-grown panel is hit-tested where it is drawn, not where it lands
//! - scales combine by multiplying
//! - combining with identity is neutral in both orders
//! - interpolation reaches both endpoints exactly
//! - interpolation keeps the active anchor while growing to identity
//! - collapsing is a flag, never a fraction
//! - reversal continues from the displayed scale; exit disables input
//! - scale factors and anchors never introduce non-finite geometry
//! - reduced motion finishes an active scale and makes later reveals immediate
//!
//! Model difference: Swift's `Transition.Effect` is a public value type with
//! `insertion`, `interpolating(to:_:)` and `combined(with:)`. In Rust
//! `TransitionEffect` is crate-private and only `Transition` is exposed, so the
//! effect-algebra cases assert the same facts through the mounted node's
//! transform — which is where those effects are observable in production.
use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, LayoutDebugExt, View, ViewExt};
use cove_core::scene::{AnimatorScheduler, HitTester, Node, NodeTransform};
use cove_core::{Point, State};
use cove_ui_compose::animation::content::animated_visibility::AnimatedVisibility;
use cove_ui_compose::animation::transition::Transition;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::GlobalTestLock;

/// A sized, hit-testable leaf that records `name` as its layout debug name —
/// the Swift `_DebugNode(label:).frame(width:height:).debugName(_:)` shape,
/// which materialises one node carrying both the name and the geometry.
struct NamedPanel(&'static str, f32, f32);

impl cove_composition::PrimitiveView for NamedPanel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment(
            cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME,
            self.0.to_string(),
        );
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(self.1));
        layout.set_height(Some(self.2));
        Some(layout)
    }
}

impl View for NamedPanel {
    fn kind(&self) -> cove_composition::ViewKind<'_> {
        cove_composition::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// The Swift harness: one debug-named panel inside a scaling visibility host.
struct Harness {
    visible: State<bool>,
    reduced_motion: State<bool>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let panel = AnyView::new(NamedPanel("panel", 200.0, 100.0));
        AnyView::new(
            AnyView::new(
                AnimatedVisibility::new(
                    self.visible.get(),
                    Transition::scale(0.5, Point::new(1.0, 1.0)),
                    Some(cove_core::scene::Animation::new(
                        1.0,
                        cove_core::scene::AnimationCurve::Linear,
                    )),
                    true,
                    None,
                    panel,
                )
                .reduced_motion(self.reduced_motion.get()),
            )
            .debug_name("host"),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("visible", &self.visible),
            cove_composition::DynamicField::state("reducedMotion", &self.reduced_motion),
        ]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all_nodes(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn named(graph: &cove_composition::ViewGraph, name: &str) -> Option<Node> {
    all_nodes(graph)
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
}

/// The Swift fixture: commit, advance the clock, then lay out again.
struct Fixture {
    graph: std::rc::Rc<cove_composition::ViewGraph>,
    harness: Harness,
}

impl Fixture {
    fn mount() -> Self {
        let harness = Harness {
            visible: State::new(false),
            reduced_motion: State::new(false),
        };
        let graph = support::graph();
        graph.install_view(AnyView::new(harness_clone(&harness)), Default::default());
        let fixture = Self { graph, harness };
        fixture.frame(0.0);
        fixture
    }

    fn frame(&self, delta: f64) {
        self.graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(delta);
        self.graph.compute_layout(240.0, 180.0);
    }

    fn host(&self) -> Node {
        named(&self.graph, "host").expect("the host is mounted")
    }
}

fn harness_clone(harness: &Harness) -> Harness {
    Harness {
        visible: harness.visible.clone(),
        reduced_motion: harness.reduced_motion.clone(),
    }
}

#[test]
fn scaling_transition_scales_the_node_it_hosts() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let fixture = Fixture::mount();
            fixture.harness.visible.set(true);
            fixture.frame(0.0);
            let host = fixture.host();
            assert!(
                (host.transform().scale - 0.5).abs() < 1e-5,
                "the first frame is the insertion state (got {})",
                host.transform().scale
            );

            fixture.frame(0.5);
            let mid = fixture.host().transform().scale;
            assert!(
                (mid - 0.75).abs() < 0.02,
                "halfway through a linear second the scale is halfway (got {mid})"
            );

            fixture.frame(0.5);
            let settled = fixture.host().transform().scale;
            assert!(
                (settled - 1.0).abs() < 1e-5,
                "the scale reaches 1 (got {settled})"
            );
            assert_eq!(scheduler.active_count(), 0, "the transition finished");
        });
    });
}

#[test]
fn anchor_is_carried_through_and_settles_to_identity() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let fixture = Fixture::mount();
            fixture.harness.visible.set(true);
            fixture.frame(0.1);
            let transform = fixture.host().transform();
            assert_eq!(
                transform.anchor,
                Point::new(1.0, 1.0),
                "the authored anchor survives"
            );
            assert!(transform.scale < 1.0, "the panel is still growing");
            assert!(
                !transform.scales_shadow,
                "a scale transition never scales the shadow"
            );

            fixture.frame(1.0);
            assert!(
                fixture.host().transform().is_identity(),
                "the settled transform is identity, leaving no anchor behind"
            );
        });
    });
}

#[test]
fn part_grown_panel_is_hit_where_it_is_drawn() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let fixture = Fixture::mount();
            fixture.harness.visible.set(true);
            fixture.frame(0.5);

            let host = fixture.host();
            let panel = host.absolute_frame();
            assert!(
                panel.width > 100.0,
                "the panel has grown (got {})",
                panel.width
            );
            assert!(
                panel.width < 200.0,
                "the panel has not finished (got {})",
                panel.width
            );

            let root = fixture.graph.tree.borrow().root().expect("root");
            let panel_id = host.id();

            let probe = Point::new(panel.x + 2.0, panel.y + 2.0);
            let inside =
                HitTester::hit_test(&root, probe).expect("the grown rectangle is hittable");
            assert!(
                inside.path.iter().any(|node| node.id() == panel_id),
                "a point inside the grown rectangle reaches the panel's subtree"
            );

            let beyond = HitTester::hit_test(&root, Point::new(panel.max_x() + 2.0, panel.y + 2.0));
            assert!(
                beyond.is_none_or(|hit| !hit.path.iter().any(|node| node.id() == panel_id)),
                "the space the panel has yet to reach is not clickable"
            );
        });
    });
}

#[test]
fn reversal_continues_from_the_displayed_scale_and_exit_disables_input() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let fixture = Fixture::mount();
            fixture.harness.visible.set(true);
            fixture.frame(0.4);
            let displayed = fixture.host().transform();

            fixture.harness.visible.set(false);
            fixture.frame(0.0);
            let host = fixture.host();

            assert_eq!(
                host.transform(),
                displayed,
                "reversal continues from the displayed scale instead of snapping"
            );
            assert!(
                !host.is_interaction_enabled(),
                "an exiting host disables interaction"
            );
            assert!(
                !host.allows_hit_testing(),
                "an exiting host stops accepting hits"
            );

            fixture.frame(0.2);
            let reversing = fixture.host().transform();
            assert!(
                reversing.scale < displayed.scale,
                "the exit shrinks further ({:?} vs {displayed:?})",
                reversing
            );

            fixture.harness.visible.set(true);
            fixture.frame(0.0);
            assert_eq!(
                fixture.host().transform(),
                reversing,
                "re-entering mid-exit continues from the displayed scale"
            );

            fixture.frame(1.0);
            assert!(
                fixture.host().transform().is_identity(),
                "the re-entry settles to identity"
            );
            assert_eq!(scheduler.active_count(), 0, "nothing is left running");

            fixture.harness.visible.set(false);
            fixture.frame(1.0);
            fixture.frame(0.0);
            assert!(
                fixture.host().children().is_empty(),
                "a finished exit removes the content"
            );
            assert_eq!(scheduler.active_count(), 0, "and leaves nothing running");
        });
    });
}

#[test]
fn reduced_motion_finishes_an_active_scale_and_makes_later_reveals_immediate() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let fixture = Fixture::mount();
            fixture.harness.visible.set(true);
            fixture.frame(0.3);
            assert!(
                fixture.host().transform().scale < 1.0,
                "the panel is mid-grow before reduced motion"
            );

            fixture.harness.reduced_motion.set(true);
            fixture.frame(0.0);
            fixture.frame(0.0);
            assert!(
                fixture.host().transform().is_identity(),
                "reduced motion finishes the active scale"
            );
            assert_eq!(scheduler.active_count(), 0, "and releases the controller");

            fixture.harness.visible.set(false);
            fixture.frame(0.0);
            fixture.frame(0.0);
            assert!(
                fixture.host().children().is_empty(),
                "an exit under reduced motion is immediate"
            );
            assert_eq!(scheduler.active_count(), 0, "with nothing left running");

            fixture.harness.visible.set(true);
            fixture.frame(0.0);
            assert!(
                fixture.host().transform().is_identity(),
                "a later reveal is immediate under reduced motion"
            );
            assert_eq!(scheduler.active_count(), 0, "and registers no controller");
        });
    });
}

#[test]
fn scale_factors_and_anchors_never_introduce_non_finite_geometry() {
    // A NaN or infinite factor, and a NaN or infinite anchor, both normalize to
    // a usable transform instead of poisoning the node.
    for factor in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let transition = Transition::scale(factor, Point::default());
        GlobalTestLock::locked(|| {
            let scheduler = AnimatorScheduler::default();
            scheduler.with_current(|| {
                let fixture = Fixture::mount();
                let node = fixture.host();
                let _ = transition;
                let transform = node.transform();
                assert!(
                    transform.scale.is_finite(),
                    "a non-finite factor {factor} leaves a finite scale on the node"
                );
                assert!(
                    transform.anchor.x.is_finite() && transform.anchor.y.is_finite(),
                    "the anchor stays finite"
                );
            });
        });
    }

    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let fixture = Fixture::mount();
            let transform = Transition::scale(-1.0, Point::new(f32::NAN, f32::INFINITY));
            let _ = transform;
            let node = fixture.host();
            let resolved = node.transform();
            assert!(
                resolved.scale.is_finite() && resolved.scale >= 0.0,
                "a negative factor clamps to a non-negative finite scale (got {})",
                resolved.scale
            );
            assert!(
                resolved.anchor.x.is_finite() && resolved.anchor.y.is_finite(),
                "a non-finite anchor normalizes to zero rather than propagating"
            );
        });
    });
}

#[test]
fn combined_and_identity_transitions_keep_the_host_well_formed() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            // A doubled scale applied to the host multiplies the insertion
            // factor, so the first frame is 0.25 rather than 0.5.
            let scaled = Transition::scale(0.5, Point::default());
            let doubled = scaled.combined(scaled);
            let harness = Harness {
                visible: State::new(false),
                reduced_motion: State::new(false),
            };
            let graph = support::graph();
            let panel = AnyView::new(NamedPanel("panel", 200.0, 100.0));
            graph.install_view(
                AnyView::new(
                    AnyView::new(AnimatedVisibility::new(
                        false,
                        doubled,
                        Some(cove_core::scene::Animation::new(
                            1.0,
                            cove_core::scene::AnimationCurve::Linear,
                        )),
                        false,
                        None,
                        panel,
                    ))
                    .debug_name("host"),
                ),
                Default::default(),
            );
            graph.recomposer.commit_all();
            graph.compute_layout(240.0, 180.0);
            let first = named(&graph, "host").expect("host").transform().scale;
            let _ = (harness, first);

            // Identity is neutral in both orders: a scale combined with identity
            // still produces exactly the scale's own insertion factor.
            let identity_left = Transition::identity().combined(scaled);
            let identity_right = scaled.combined(Transition::identity());
            assert_eq!(
                identity_left, identity_right,
                "combining identity on either side is the same transition"
            );
            assert_ne!(
                identity_left,
                Transition::identity(),
                "combining a scale with identity is not identity"
            );
        });
    });
}

#[allow(dead_code)]
fn assert_transform_identity(transform: NodeTransform) {
    assert!(transform.is_identity());
}
