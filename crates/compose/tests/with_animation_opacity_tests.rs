//! Translation of `Tests/CoveUIComposeTests/WithAnimationOpacityTests.swift`.
//!
//! Behaviour checklist:
//! - a plain mutation writes opacity instantly and registers nothing
//! - `with_animation` defers opacity through the scheduler, and a mid-flight
//!   frame marks the node for repaint
//! - an inner `with_animation(None)` opts out of an outer animation
//! - the default overload uses `Animation::default`
//! - `ActiveAnimationContext` is restored when the body unwinds
//! - the active context is empty outside `with_animation`
//!
//! Named difference: Swift's `withAnimation` is `rethrows` and the suite drains
//! the thrown error. Rust closures are infallible, so the analogous property —
//! the context is restored even if the body unwinds — is asserted with
//! `catch_unwind` instead of a `do/catch`.

use crate::support;
use cove_composition::Scope;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::State;
use cove_core::scene::{
    ActiveAnimationContext, Animation, AnimationCurve, AnimatorScheduler, with_animation,
};
use cove_ui_compose::modifiers::style::AppearanceExt;

use support::{DebugNode, GlobalTestLock};

fn linear(seconds: f64) -> Option<Animation> {
    Some(Animation::new(seconds, AnimationCurve::Linear))
}

struct OpacityHarness {
    alpha: State<f32>,
}

impl View for OpacityHarness {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(DebugNode::new("x").opacity(self.alpha.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("alpha", &self.alpha)]
    }
}

fn fixture() -> (
    std::rc::Rc<cove_composition::ViewGraph>,
    State<f32>,
    cove_core::scene::Node,
) {
    let graph = support::graph();
    let alpha = State::new(0.0f32);
    graph.install_view(
        AnyView::new(OpacityHarness {
            alpha: alpha.clone(),
        }),
        Default::default(),
    );
    let root = graph.tree.borrow().root().expect("root");
    let node = support::deepest_node(&root);
    (graph, alpha, node)
}

#[test]
fn plain_mutation_is_instant() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let (graph, alpha, node) = fixture();
            assert_eq!(node.opacity(), 0.0, "the harness starts transparent");

            alpha.set(1.0);
            graph.recomposer.commit_all();

            assert_eq!(
                node.opacity(),
                1.0,
                "an unwrapped mutation writes the new value at once"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "no animation is registered without with_animation"
            );
        });
    });
}

#[test]
fn animated_mutation_goes_through_scheduler() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let (graph, alpha, node) = fixture();
            graph.tree.borrow_mut().flush();
            assert_eq!(node.opacity(), 0.0, "the harness starts transparent");

            with_animation(linear(1.0), || alpha.set(1.0));
            graph.recomposer.commit_all();

            assert_eq!(node.opacity(), 0.0, "the change starts from the old value");
            assert_eq!(scheduler.active_count(), 1, "one animation is running");
            assert!(
                !node.render_dirty(),
                "committing alone does not mark the node for repaint"
            );

            scheduler.tick(0.5);
            assert_eq!(
                node.opacity(),
                0.5,
                "halfway through a linear 1s animation the opacity is midway"
            );
            assert!(
                node.render_dirty(),
                "an animating frame marks the node for repaint"
            );

            scheduler.tick(0.5);
            assert_eq!(node.opacity(), 1.0, "the animation lands on the target");
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn nil_animation_opts_out() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let (graph, alpha, node) = fixture();

            with_animation(linear(1.0), || {
                with_animation(None, || alpha.set(1.0));
            });
            graph.recomposer.commit_all();

            assert_eq!(
                node.opacity(),
                1.0,
                "the inner nil overrides the outer animation"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "an opted-out write registers nothing"
            );
        });
    });
}

#[test]
fn default_overload_animates() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let (graph, alpha, node) = fixture();
            assert_eq!(node.opacity(), 0.0, "the harness starts transparent");

            with_animation(Some(Animation::default()), || alpha.set(1.0));
            graph.recomposer.commit_all();
            assert_eq!(scheduler.active_count(), 1, "the default animation runs");

            scheduler.tick(Animation::default().duration);
            assert_eq!(node.opacity(), 1.0, "it lands after its own duration");
            assert_eq!(scheduler.active_count(), 0, "the scheduler idles again");
        });
    });
}

#[test]
fn context_is_restored_when_the_body_unwinds() {
    GlobalTestLock::locked(|| {
        let observed = std::panic::catch_unwind(|| {
            with_animation(linear(0.1), || {
                assert!(
                    ActiveAnimationContext::current().animation.is_some(),
                    "the body sees the animation it was given"
                );
                panic!("boom");
            })
        });
        assert!(observed.is_err(), "the panic propagates out of the body");
        assert!(
            ActiveAnimationContext::current().animation.is_none(),
            "the previous context is restored even on unwind"
        );
    });
}

#[test]
fn context_nil_outside() {
    GlobalTestLock::locked(|| {
        assert!(
            ActiveAnimationContext::current().animation.is_none(),
            "no animation is active outside with_animation"
        );
        assert!(
            !ActiveAnimationContext::current().disables_animations,
            "the default context does not disable animations"
        );
    });
}
