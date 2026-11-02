//! Translation of `Tests/CoveUIComposeTests/Animation/SequenceLifecycleTests.swift`.
//!
//! Behaviour checklist:
//! - an untriggered keyframe track rests at its timeline's initial value
//! - an untriggered phase sequence rests at its first phase
//! - reduced motion at mount does not start either sequence
//! - after a trigger both sequences complete and release their scheduler entry
//! - the completed value survives turning reduced motion back off
//!
//! Model difference: `PhaseAnimator` requires an `Eq` phase, and `f32` is not
//! `Eq`, so the Swift `[0, 0.5, 1]` opacity phases are expressed as whole
//! percentages and divided back to an opacity in the content closure. The
//! observable behaviour — resting at 0 and completing at 1 — is unchanged.

use cove_composition::Scope;
use crate::support;

use cove_composition::{AnyView, LayoutDebugExt, TupleView, View};
use cove_core::State;
use cove_core::scene::{
    Align, AnimationCurve, AnimationKeyframe, AnimatorScheduler, FlexDirection, Justify,
    KeyframeTimeline, Node, Wrap,
};
use cove_ui_compose::BoxView;
use cove_ui_compose::animation::keyframes::track::KeyframeAnimator;
use cove_ui_compose::animation::phases::sequence::PhaseAnimator;
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::{DebugNode, GlobalTestLock};

/// The Swift harness: two sequences keyed off one revision counter, both
/// rendered as debug-named tracks so the test can read their opacity.
struct Harness {
    revision: State<i32>,
    reduced: State<bool>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let revision = self.revision.get();
        let reduced = self.reduced.get();

        let track = AnyView::new(KeyframeAnimator::new(
            KeyframeTimeline::new(
                0.0f32,
                vec![AnimationKeyframe::new(1.0f32, 0.4, AnimationCurve::Linear)],
            ),
            revision,
            false,
            |value: f32| {
                AnyView::new(DebugNode::new("track"))
                    .opacity(value)
                    .debug_name("lifecycle-track")
            },
        ));

        let phase = AnyView::new(PhaseAnimator::new(
            vec![0i32, 50, 100],
            revision,
            false,
            |value: i32| {
                AnyView::new(DebugNode::new("phase"))
                    .opacity(value as f32 / 100.0)
                    .debug_name("lifecycle-phase")
            },
        ));

        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![track, phase])),
            ))
            .reduced_motion(reduced),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("revision", &self.revision),
            cove_composition::DynamicField::state("reduced", &self.reduced),
        ]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn collect_root(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn named(nodes: &[Node], name: &str) -> Node {
    nodes
        .iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("node {name} is mounted"))
}

fn settle(graph: &cove_composition::ViewGraph) {
    for _ in 0..4 {
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 200.0);
    }
}

#[test]
fn untriggered_sequences_rest_and_completed_values_survive_reduced_motion() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let revision = State::new(0i32);
            let reduced = State::new(false);
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness {
                    revision: revision.clone(),
                    reduced: reduced.clone(),
                }),
                Default::default(),
            );
            settle(&graph);

            let nodes = collect_root(&graph);
            let track = named(&nodes, "lifecycle-track");
            let phase = named(&nodes, "lifecycle-phase");
            assert_eq!(track.opacity(), 0.0, "an untriggered track stays at 0");
            assert_eq!(
                phase.opacity(),
                0.0,
                "an untriggered sequence stays at phase 0"
            );

            reduced.set(true);
            settle(&graph);
            let nodes = collect_root(&graph);
            assert_eq!(
                named(&nodes, "lifecycle-track").opacity(),
                0.0,
                "turning reduced motion on does not start an untriggered track"
            );
            assert_eq!(
                named(&nodes, "lifecycle-phase").opacity(),
                0.0,
                "turning reduced motion on does not start an untriggered sequence"
            );

            revision.set(1);
            settle(&graph);
            let nodes = collect_root(&graph);
            assert_eq!(
                named(&nodes, "lifecycle-track").opacity(),
                1.0,
                "reduced motion jumps the track straight to its final keyframe"
            );
            assert_eq!(
                named(&nodes, "lifecycle-phase").opacity(),
                1.0,
                "reduced motion jumps the sequence straight to its last phase"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "a reduced-motion sequence registers no controller"
            );

            reduced.set(false);
            settle(&graph);
            assert_eq!(
                scheduler.active_count(),
                0,
                "leaving reduced motion starts nothing"
            );
            assert!(
                !graph.recomposer.has_pending(),
                "the completed sequence is idle"
            );
            let nodes = collect_root(&graph);
            assert_eq!(
                named(&nodes, "lifecycle-track").opacity(),
                1.0,
                "the completed track value survives"
            );
            assert_eq!(
                named(&nodes, "lifecycle-phase").opacity(),
                1.0,
                "the completed sequence value survives"
            );
        });
    });
}

#[test]
fn reduced_motion_from_mount_keeps_both_sequences_at_rest() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness {
                    revision: State::new(0i32),
                    reduced: State::new(true),
                }),
                Default::default(),
            );
            settle(&graph);
            let nodes = collect_root(&graph);
            assert_eq!(
                named(&nodes, "lifecycle-track").opacity(),
                0.0,
                "a reduced-motion mount leaves the track at rest"
            );
            assert_eq!(
                named(&nodes, "lifecycle-phase").opacity(),
                0.0,
                "a reduced-motion mount leaves the sequence at rest"
            );
            assert_eq!(scheduler.active_count(), 0, "nothing animates at rest");
        });
    });
}
