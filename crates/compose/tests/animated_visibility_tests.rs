//! Translation of `Tests/CoveUIComposeTests/AnimatedVisibilityTests.swift`.
//!
//! Behaviour checklist:
//! - an initially collapsed host leaves layout stable and never mounts content,
//!   even after a viewport change
//! - an exit retains the painted subtree while disabling its input immediately,
//!   then removes it and clears the scheduler
//! - reversing an exit reuses the same node, restores input and cancels the old
//!   animation
//! - reduced motion settles an active reveal, disables exit input, and reports
//!   completion exactly once per direction
//! - a reveal measures changing content at its natural height, including
//!   shrinking during a reversal
//! - unmounting a transitioning subtree releases its scheduler entry without
//!   reporting completion

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, View};
use cove_core::State;
use cove_core::scene::{
    Animation, AnimationCurve, AnimatorScheduler, Node, NodeTree, PlatformInputContext, Recomposer,
};
use cove_ui_compose::animation::content::animated_visibility::AnimatedVisibility;
use cove_ui_compose::animation::transition::Transition;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::GlobalTestLock;

const CONTENT: &str = "visibility-content";
const HOST: &str = "visibility-host";
const UNMOUNTED: &str = "unmounted-content";

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn named(graph: &cove_composition::ViewGraph, name: &str) -> Option<Node> {
    let root = graph.tree.borrow().root()?;
    let mut nodes = Vec::new();
    all_nodes(&root, &mut nodes);
    nodes
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
}

/// The Swift `Harness`: observed visibility, reduced motion, content height and
/// a completion counter.
struct Harness {
    visible: State<bool>,
    reduced_motion: State<bool>,
    height: State<f32>,
    completed: State<usize>,
}

impl Harness {
    fn new(visible: bool) -> Self {
        Self {
            visible: State::new(visible),
            reduced_motion: State::new(false),
            height: State::new(80.0),
            completed: State::new(0),
        }
    }

    fn graph_harness(&self) -> HarnessView {
        HarnessView {
            visible: self.visible.clone(),
            reduced_motion: self.reduced_motion.clone(),
            height: self.height.clone(),
            completed: self.completed.clone(),
        }
    }
}

struct HarnessView {
    visible: State<bool>,
    reduced_motion: State<bool>,
    height: State<f32>,
    completed: State<usize>,
}

impl View for HarnessView {
    fn body(&self) -> AnyView {
        let completed = self.completed.clone();
        let content = AnyView::new(AnyView::new(
            AnyView::new(Button::new(Text::new("Content"), || {}))
                .frame_height(self.height.get())
                .debug_name(CONTENT),
        ));
        AnyView::new(
            AnimatedVisibility::new(
                self.visible.get(),
                Transition::opacity().combined(Transition::collapse()),
                Some(Animation::new(0.2, AnimationCurve::Linear)),
                false,
                Some(Rc::new(move |_| {
                    completed.set(completed.get() + 1);
                })),
                content,
            )
            .reduced_motion(self.reduced_motion.get())
            .debug_name(HOST),
        )
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("visible", &self.visible),
            DynamicField::state("reducedMotion", &self.reduced_motion),
            DynamicField::state("height", &self.height),
            DynamicField::state("completed", &self.completed),
        ]
    }
}

fn graph() -> Rc<cove_composition::ViewGraph> {
    Rc::new(cove_composition::ViewGraph::new(
        Rc::new(std::cell::RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

#[test]
fn initially_collapsed_layout_stays_stable() {
    GlobalTestLock::locked(|| {
        let context = PlatformInputContext::new();
        context.with_current(|| {
            let graph = graph();
            graph.install(
                AnimatedVisibility::new(
                    false,
                    Transition::default(),
                    Some(Animation::new(0.2, AnimationCurve::Linear)),
                    false,
                    None,
                    AnyView::new(
                        AnyView::new(Button::new(Text::new("Hidden"), || {})).debug_name(UNMOUNTED),
                    ),
                ),
                Default::default(),
            );

            assert!(
                graph.compute_layout_if_needed(240.0, 180.0),
                "the first pass measures the collapsed host"
            );
            for _ in 0..10 {
                assert!(
                    !graph.layout_needs_update(240.0, 180.0),
                    "a collapsed host leaves layout clean"
                );
                assert!(
                    !graph.compute_layout_if_needed(240.0, 180.0),
                    "no further layout pass is needed"
                );
            }
            let root = graph.tree.borrow().root().expect("root");
            let mut nodes = Vec::new();
            all_nodes(&root, &mut nodes);
            assert!(
                nodes
                    .iter()
                    .all(|node| support::debug_name(node).as_deref() != Some(UNMOUNTED)),
                "a collapsed host never mounts its content"
            );

            graph.compute_layout(320.0, 200.0);
            assert!(
                !graph.layout_needs_update(320.0, 200.0),
                "a viewport change leaves the collapsed host clean"
            );
        });
    });
}

#[test]
fn exit_retains_paint_disables_input_then_removes_content() {
    GlobalTestLock::locked(|| {
        let context = PlatformInputContext::new();
        let scheduler = AnimatorScheduler::default();
        context.with_current(|| {
            scheduler.with_current(|| {
                let harness = Harness::new(true);
                let graph = graph();
                graph.install_view(AnyView::new(harness.graph_harness()), Default::default());
                graph.compute_layout(240.0, 180.0);
                let content = named(&graph, CONTENT).expect("the content is mounted");

                harness.visible.set(false);
                graph.recomposer.commit_all();

                let still_there = named(&graph, CONTENT).expect("the exiting content stays");
                assert_eq!(
                    still_there.id(),
                    content.id(),
                    "the exiting subtree keeps its node identity"
                );
                assert!(
                    !still_there.accepts_subtree_input(),
                    "an exiting subtree stops accepting input immediately"
                );

                scheduler.tick(0.1);
                graph.compute_layout(240.0, 180.0);
                assert!(
                    named(&graph, CONTENT).is_some(),
                    "halfway through the exit the content is still painted"
                );

                scheduler.tick(0.2);
                graph.recomposer.commit_all();
                graph.compute_layout(240.0, 180.0);
                assert!(
                    named(&graph, CONTENT).is_none(),
                    "the exit retires the content once it settles"
                );
                assert_eq!(
                    scheduler.active_count(),
                    0,
                    "the exit leaves nothing registered"
                );
            });
        });
    });
}

#[test]
fn reversal_reuses_the_subtree_and_cancels_the_animation() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let harness = Harness::new(true);
            let graph = graph();
            graph.install_view(AnyView::new(harness.graph_harness()), Default::default());
            graph.compute_layout(240.0, 180.0);
            let content = named(&graph, CONTENT).expect("the content is mounted");

            harness.visible.set(false);
            graph.recomposer.commit_all();
            scheduler.tick(0.08);

            harness.visible.set(true);
            graph.recomposer.commit_all();
            scheduler.tick(1.0);
            graph.recomposer.commit_all();

            let revived = named(&graph, CONTENT).expect("the content survives the reversal");
            assert_eq!(
                revived.id(),
                content.id(),
                "reversing reuses the same subtree node"
            );
            assert!(
                revived.accepts_subtree_input(),
                "reversing restores input to the subtree"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "the old exit animation is cancelled"
            );
        });
    });
}

#[test]
fn reduced_motion_settles_a_reveal_and_completes_once_per_direction() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let harness = Harness::new(true);
            let graph = graph();
            graph.install_view(AnyView::new(harness.graph_harness()), Default::default());
            graph.compute_layout(240.0, 300.0);

            harness.visible.set(false);
            graph.recomposer.commit_all();
            scheduler.tick(0.05);
            assert_eq!(
                scheduler.active_count(),
                1,
                "the exit is animating before reduced motion arrives"
            );

            harness.reduced_motion.set(true);
            graph.recomposer.commit_all();
            graph.recomposer.commit_all();
            assert!(
                named(&graph, CONTENT).is_none(),
                "reduced motion retires the content immediately"
            );
            assert_eq!(scheduler.active_count(), 0, "nothing is left animating");
            assert_eq!(
                harness.completed.get(),
                1,
                "the exit reports completion exactly once"
            );

            harness.visible.set(true);
            graph.recomposer.commit_all();
            graph.recomposer.commit_all();
            assert!(
                named(&graph, CONTENT).is_some_and(|node| node.accepts_subtree_input()),
                "reduced motion reveals the content with input restored"
            );
            assert_eq!(harness.completed.get(), 2, "the reveal reports once too");

            scheduler.tick(1.0);
            graph.recomposer.commit_all();
            assert_eq!(
                harness.completed.get(),
                2,
                "a later tick does not report completion again"
            );
        });
    });
}

#[test]
fn reveal_measures_changing_content_at_its_natural_height() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let harness = Harness::new(true);
            let graph = graph();
            let settle = || {
                for _ in 0..3 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(240.0, 300.0);
                }
            };
            graph.install_view(AnyView::new(harness.graph_harness()), Default::default());
            settle();

            harness.visible.set(false);
            graph.recomposer.commit_all();
            scheduler.tick(0.1);
            settle();
            assert_eq!(
                named(&graph, CONTENT).expect("content").frame().height,
                80.0,
                "the retained content keeps its natural height"
            );
            assert_eq!(
                named(&graph, HOST).expect("host").frame().height,
                40.0,
                "half-collapsed host measures 40pt"
            );

            harness.height.set(160.0);
            settle();
            assert_eq!(
                named(&graph, HOST).expect("host").frame().height,
                80.0,
                "a taller collapsed content doubles the host height"
            );

            harness.height.set(40.0);
            settle();
            assert_eq!(
                named(&graph, HOST).expect("host").frame().height,
                20.0,
                "shrinking during an exit follows the new natural height"
            );

            harness.visible.set(true);
            graph.recomposer.commit_all();
            scheduler.tick(1.0);
            settle();
            assert_eq!(
                named(&graph, HOST).expect("host").frame().height,
                40.0,
                "the reveal restores the full natural height"
            );
            assert_eq!(
                harness.completed.get(),
                1,
                "the reveal reports completion once"
            );

            graph.install(cove_composition::EmptyView, Default::default());
            assert_eq!(
                scheduler.active_count(),
                0,
                "unmounting releases the scheduler"
            );
        });
    });
}

#[test]
fn unmounting_a_transitioning_subtree_releases_its_scheduler_entry() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let harness = Harness::new(true);
            let graph = graph();
            graph.install_view(AnyView::new(harness.graph_harness()), Default::default());
            graph.compute_layout(240.0, 180.0);

            harness.visible.set(false);
            graph.recomposer.commit_all();
            assert_eq!(
                scheduler.active_count(),
                1,
                "the exit registers an animation"
            );

            graph.install(cove_composition::EmptyView, Default::default());
            assert_eq!(
                scheduler.active_count(),
                0,
                "unmounting releases the scheduler entry immediately"
            );
            assert_eq!(
                harness.completed.get(),
                0,
                "an interrupted exit never reports completion"
            );
        });
    });
}
