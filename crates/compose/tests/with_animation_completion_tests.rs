//! Translation of `Tests/CoveUIComposeTests/WithAnimationCompletionTests.swift`.
//!
//! Behaviour checklist:
//! - the callback fires on the frame its last controller finishes, exactly once
//! - a mutation that animates nothing still completes
//! - an interrupted (superseded) animation does not complete
//! - two writes coalesced into one commit share a single completion
//! - with no callback the scheduler is untouched
//! - enabling reduced motion finishes the current target and its completion

use cove_composition::Scope;
mod support;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::State;
use cove_core::scene::{
    Animation, AnimationCompletionCriteria, AnimationCurve, AnimatorScheduler, Node,
    with_animation, with_animation_completion,
};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::{DebugNode, GlobalTestLock};

fn one_second() -> Option<Animation> {
    Some(Animation::new(1.0, AnimationCurve::Linear))
}

struct Harness {
    alpha: State<f32>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DebugNode::new("x").opacity(self.alpha.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("alpha", &self.alpha)]
    }
}

struct ReducedHarness {
    alpha: State<f32>,
    reduced: State<bool>,
}

impl View for ReducedHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(DebugNode::new("reduced").opacity(self.alpha.get()))
                .reduced_motion(self.reduced.get()),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("alpha", &self.alpha),
            DynamicField::state("reduced", &self.reduced),
        ]
    }
}

struct Fixture {
    scheduler: AnimatorScheduler,
    graph: std::rc::Rc<cove_composition::ViewGraph>,
    alpha: State<f32>,
    node: Node,
}

impl Fixture {
    fn new() -> Self {
        let scheduler = AnimatorScheduler::default();
        let graph = support::graph();
        let alpha = State::new(0.0f32);
        graph.install_view(
            AnyView::new(Harness {
                alpha: alpha.clone(),
            }),
            Default::default(),
        );
        graph.tree.borrow_mut().flush();
        let root = graph.tree.borrow().root().expect("root");
        let node = support::deepest_node(&root);
        Self {
            scheduler,
            graph,
            alpha,
            node,
        }
    }

    /// A frame: commit whatever the mutation queued, then advance the clock.
    fn frame(&self, delta: f64) {
        self.graph.recomposer.commit_all();
        self.scheduler.tick(delta);
    }
}

#[test]
fn fires_when_the_last_controller_finishes() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new();
        fixture.scheduler.with_current(|| {
            let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let counter = fired.clone();
            with_animation_completion(
                one_second(),
                AnimationCompletionCriteria::LogicallyComplete,
                || fixture.alpha.set(1.0),
                move || counter.set(counter.get() + 1),
            );

            fixture.frame(0.5);
            assert_eq!(fired.get(), 0, "half a second is not enough");
            fixture.frame(0.25);
            assert_eq!(fired.get(), 0, "three quarters is not enough");
            fixture.frame(0.2);
            assert_eq!(fired.get(), 0, "still short of the target");
            fixture.frame(0.5);
            assert_eq!(fired.get(), 1, "the callback fires on the landing frame");
            fixture.frame(0.5);
            assert_eq!(fired.get(), 1, "the callback fires exactly once");
        });
    });
}

#[test]
fn fires_when_nothing_animates() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new();
        fixture.scheduler.with_current(|| {
            let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let counter = fired.clone();
            // Writing the value it already holds registers no controller.
            let current = fixture.alpha.get();
            with_animation_completion(
                one_second(),
                AnimationCompletionCriteria::LogicallyComplete,
                || fixture.alpha.set(current),
                move || counter.set(counter.get() + 1),
            );
            assert_eq!(fired.get(), 0, "nothing has committed yet");
            fixture.frame(0.1);
            assert_eq!(
                fired.get(),
                1,
                "a scope that animated nothing still completes"
            );
        });
    });
}

#[test]
fn does_not_fire_when_superseded() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new();
        fixture.scheduler.with_current(|| {
            let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let counter = fired.clone();
            with_animation_completion(
                one_second(),
                AnimationCompletionCriteria::LogicallyComplete,
                || fixture.alpha.set(1.0),
                move || counter.set(counter.get() + 1),
            );

            // One frame so the mutation commits and a controller is really
            // running — two writes before any commit coalesce into a single
            // recompose and never interrupt anything.
            fixture.frame(0.5);

            with_animation(one_second(), || fixture.alpha.set(0.25));
            fixture.frame(0.5);
            fixture.frame(2.0);
            assert_eq!(
                fired.get(),
                0,
                "a superseded animation never reports completion"
            );
        });
    });
}

#[test]
fn coalesced_writes_complete_once() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new();
        fixture.scheduler.with_current(|| {
            let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let counter = fired.clone();
            with_animation_completion(
                one_second(),
                AnimationCompletionCriteria::LogicallyComplete,
                || fixture.alpha.set(1.0),
                move || counter.set(counter.get() + 1),
            );
            // Lands before the first commit: one recompose, not an interruption.
            with_animation(one_second(), || fixture.alpha.set(0.25));
            fixture.frame(2.0);
            assert_eq!(
                fired.get(),
                1,
                "the merged animation fires the retained completion once"
            );
        });
    });
}

#[test]
fn no_callback_tracks_nothing() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new();
        fixture.scheduler.with_current(|| {
            with_animation(one_second(), || fixture.alpha.set(1.0));
            fixture.frame(0.1);
            assert_eq!(
                fixture.scheduler.active_count(),
                1,
                "the animation is still running"
            );
            fixture.frame(2.0);
            assert_eq!(
                fixture.scheduler.active_count(),
                0,
                "without a callback the scheduler still drains"
            );
        });
    });
}

#[test]
fn reduced_motion_completes() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            let alpha = State::new(0.0f32);
            let reduced = State::new(false);
            graph.install_view(
                AnyView::new(ReducedHarness {
                    alpha: alpha.clone(),
                    reduced: reduced.clone(),
                }),
                Default::default(),
            );

            let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let counter = fired.clone();
            with_animation_completion(
                one_second(),
                AnimationCompletionCriteria::Removed,
                || alpha.set(1.0),
                move || counter.set(counter.get() + 1),
            );
            graph.recomposer.commit_all();
            scheduler.tick(0.1);
            assert_eq!(fired.get(), 0, "the animation is still mid-flight");
            assert_eq!(scheduler.active_count(), 1, "one controller is running");

            reduced.set(true);
            graph.recomposer.commit_all();
            scheduler.tick(0.0);
            assert_eq!(
                fired.get(),
                1,
                "reduced motion finishes the target and reports completion"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "no controller survives the reduced-motion snap"
            );
        });
    });
}
