//! Translation of `Tests/CoveUIComposeTests/SpringInterruptionTests.swift`.
//!
//! Behaviour checklist:
//! - a spring reports the velocity it is travelling at
//! - the velocity at release is what the caller asked for
//! - a physical spring converges without moving its equilibrium to pin an
//!   endpoint — the endpoint anchor applies to a release from rest only
//! - reversing mid-flight does not stall the value
//! - seeding a successor with the outgoing rate moves it further
//!
//! Name mapping: `AnimationController(from:to:animation:initialVelocity:_:)` is
//! `AnimationController::new(from, to, animation, initial_velocity, apply)`,
//! `cornerRadius(_)` is `corner_radius(..)`, and `SpringSolution.position(at:)`
//! threads an explicit `start`/`velocity`/`target`.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, View};
use cove_core::State;
use cove_core::scene::{
    Animation, AnimationController, AnimationCurve, AnimatorScheduler, AnyAnimationController,
    SpringSolution, with_animation,
};
use cove_ui_compose::modifiers::style::AppearanceExt;

use support::{DebugNode, GlobalTestLock};

const RESPONSE: f32 = 0.4;

fn bouncy(allows_overshoot: bool) -> Animation {
    Animation::new(
        RESPONSE as f64,
        AnimationCurve::Spring {
            response: RESPONSE,
            damping_fraction: 0.55,
        },
    )
    .with_overshoot(allows_overshoot)
}

#[test]
fn reports_velocity() {
    let controller = AnimationController::new(0.0f32, 1.0f32, bouncy(false), 0.0, |_| {});
    assert_eq!(
        controller.progress_velocity(),
        0.0,
        "nothing has moved yet, so there is nothing to hand over"
    );

    controller.tick(0.1);
    let first = controller.progress_velocity();
    assert!(first > 0.0, "the spring is travelling (got {first})");

    controller.tick(0.1);
    assert_ne!(
        controller.progress_velocity(),
        first,
        "the rate changes as the spring settles"
    );
}

#[test]
fn released_with_velocity() {
    let solution = SpringSolution::new(RESPONSE, 0.7);
    assert!(
        (solution.velocity(0.0, 0.0, 2.5, 1.0) - 2.5).abs() < 1e-3,
        "at t = 0 the reported velocity is the requested one"
    );
    assert!(
        (solution.velocity(0.0, 0.0, -2.5, 1.0) + 2.5).abs() < 1e-3,
        "a negative release keeps its sign"
    );
}

#[test]
fn normalization_applies_to_rest_only() {
    let solution = SpringSolution::new(RESPONSE, 0.55);
    assert!(
        (solution.position(RESPONSE, 0.0, 0.0, 1.0) - 1.0).abs() > 1e-4,
        "at exactly one response a rest release has not arrived yet"
    );
    assert!(
        (solution.position(RESPONSE * 5.0, 0.0, 0.0, 1.0) - 1.0).abs() < 1e-4,
        "it converges on the target"
    );

    // Released with a shove it is somewhere else on its way, and must not be
    // yanked onto the same schedule.
    let shoved = solution.position(RESPONSE, 0.0, 2.0, 1.0);
    assert_ne!(
        shoved, 1.0,
        "a seeded spring is not pinned to the rest-release arrival instant"
    );
    let later = solution.position(RESPONSE * 3.0, 0.0, 2.0, 1.0);
    assert!(
        (later - 1.0).abs() < (shoved - 1.0).abs(),
        "it still converges rather than diverging ({later} vs {shoved})"
    );
}

struct Harness {
    radius: State<f32>,
}

impl View for Harness {
    fn body(&self) -> AnyView {
        AnyView::new(DebugNode::new("x").corner_radius(self.radius.get()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("radius", &self.radius)]
    }
}

struct Fixture {
    scheduler: AnimatorScheduler,
    graph: std::rc::Rc<cove_composition::ViewGraph>,
    radius: State<f32>,
    node: cove_core::scene::Node,
}

impl Fixture {
    fn new() -> Self {
        let scheduler = AnimatorScheduler::default();
        let graph = support::graph();
        let radius = State::new(0.0f32);
        graph.install_view(
            AnyView::new(Harness {
                radius: radius.clone(),
            }),
            Default::default(),
        );
        graph.tree.borrow_mut().flush();
        let root = graph.tree.borrow().root().expect("root");
        let node = support::deepest_node(&root);
        Self {
            scheduler,
            graph,
            radius,
            node,
        }
    }

    fn frame(&self, delta: f64) {
        self.graph.recomposer.commit_all();
        self.scheduler.tick(delta);
    }
}

#[test]
fn reversal_does_not_stall() {
    GlobalTestLock::locked(|| {
        let fixture = Fixture::new();
        fixture.scheduler.with_current(|| {
            with_animation(Some(bouncy(true)), || fixture.radius.set(60.0));
            fixture.frame(0.1);
            let rising = fixture.node.corner_radius();
            assert!(rising > 0.0, "the first frame is travelling (got {rising})");

            with_animation(Some(bouncy(true)), || fixture.radius.set(0.0));
            fixture.frame(0.0001);

            let after_reverse = fixture.node.corner_radius();
            assert!(
                after_reverse > rising,
                "the value keeps moving in the outgoing direction rather than \
                 jumping back (rising {rising}, after reverse {after_reverse})"
            );
            assert!(
                after_reverse > 0.0,
                "the reversal does not snap to zero (got {after_reverse})"
            );

            fixture.frame(0.2);
            assert!(
                fixture.node.corner_radius() < rising,
                "the value has turned around and is falling"
            );
        });
    });
}

#[test]
fn seeding_moves_the_successor_further() {
    let outgoing = AnimationController::new(0.0f32, 1.0f32, bouncy(true), 0.0, |_| {});
    outgoing.tick(0.2);
    let rate = outgoing.progress_velocity();
    assert!(
        rate > 0.0,
        "the outgoing controller is travelling (got {rate})"
    );

    let last = Rc::new(Cell::new(0.0f32));
    let first_slice = |velocity: f32| -> f32 {
        let sink = last.clone();
        let controller =
            AnimationController::new(0.0f32, 1.0f32, bouncy(true), velocity, move |value| {
                sink.set(value)
            });
        controller.tick(0.05);
        last.get()
    };

    last.set(0.0);
    let from_rest = first_slice(0.0);
    last.set(0.0);
    let inherited = first_slice(rate);

    assert!(
        inherited > from_rest,
        "seeding the successor with the outgoing rate moves it further \
         (rest {from_rest}, inherited {inherited})"
    );
    assert!(
        inherited < 1.0,
        "the seeded successor has not already arrived (got {inherited})"
    );
}
