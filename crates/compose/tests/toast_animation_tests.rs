//! Translation of `Tests/CoveUIComposeTests/ToastAnimationTests.swift`.
//!
//! Behaviour checklist:
//! - a posted card fades in over its entry animation and finishes fully opaque
//! - dismissal removes the card from the controller, opens no modal input, and
//!   releases focus from the dismiss button
//! - the exit animation fades the card back out and the scheduler quiesces
//! - reduced motion snaps in both directions and leaves no exit clocks
//! - reposting during an exit rejects the stale completion, and rapid changes
//!   keep a bounded tree

use crate::support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_platform::{InputEvent, MouseMotionEvent};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStore, PortalStoreHolder};
use cove_ui_compose::primitives::overlays::toast::{
    NotificationHost, ToastController, ToastNotification, ToastPresentation,
};
use cove_ui_compose::theme::tokens::ReducedMotionViewExt;

use support::GlobalTestLock;

const WIDTH: f32 = 720.0;
const HEIGHT: f32 = 820.0;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

fn frame(graph: &cove_composition::ViewGraph, delta: f64) {
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(WIDTH, HEIGHT);
    }
    cove_core::scene::AnimatorScheduler::current().tick(delta);
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(WIDTH, HEIGHT);
    }
}

/// Installs the host and returns the graph plus its controller.
fn scene(reduced_motion: bool) -> (Rc<cove_composition::ViewGraph>, Rc<ToastController>) {
    let graph = support::graph();
    let controller = ToastController::new(1);
    graph.install_view(
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(NotificationHost::new(
                controller.clone(),
            ))))
            .frame_size(WIDTH, HEIGHT)
            .reduced_motion(reduced_motion),
        ),
        Default::default(),
    );
    (graph, controller)
}

/// Effective opacity: the node's own times every ancestor's.
fn opacity(node: &Node) -> f32 {
    node.opacity() * node.parent().map_or(1.0, |parent| opacity(&parent))
}

/// The rendered title node, if the card mounted.
fn title(graph: &cove_composition::ViewGraph, text: &str) -> Option<Node> {
    all(graph).into_iter().find(|node| {
        node.layout_node()
            .and_then(|layout| layout.text_inputs())
            .is_some_and(|inputs| inputs.text == text)
    })
}

#[test]
fn a_posted_card_fades_in_and_dismissal_releases_the_portal() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            PortalStoreHolder::set_current(Some(Rc::new(PortalStore::new())));
            let (graph, controller) = scene(false);
            let scheduler = cove_core::scene::AnimatorScheduler::current();

            controller.post(ToastNotification::new("Saved").id("saved").duration(None));
            frame(&graph, 0.0);
            let saved = title(&graph, "Saved").expect("posting mounts the card");
            assert_eq!(opacity(&saved), 0.0, "the card starts transparent");
            frame(&graph, 0.08);
            let mid = opacity(&saved);
            assert!(
                mid > 0.0 && mid < 1.0,
                "the entry fade is in flight ({mid})"
            );
            frame(&graph, 1.0);
            assert_eq!(opacity(&saved), 1.0, "the entry fade finishes opaque");

            let dismiss = all(&graph)
                .into_iter()
                .find(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Dismiss notification")
                })
                .expect("the card has a dismiss button");
            controller.dismiss("saved");
            frame(&graph, 0.0);
            assert!(
                controller.notifications().is_empty(),
                "dismissal clears the controller"
            );
            assert!(
                !PortalStoreHolder::current().entries().is_empty(),
                "the portal stays for the exit animation"
            );
            assert!(
                !dismiss.accepts_subtree_input(),
                "the exiting card takes no input"
            );
            frame(&graph, 0.08);
            let exiting = opacity(&saved);
            assert!(
                exiting > 0.0 && exiting < 1.0,
                "the exit fade is in flight ({exiting})"
            );
            frame(&graph, 1.0);
            assert!(
                PortalStoreHolder::current().entries().is_empty(),
                "the portal is released after the exit"
            );
            assert!(
                !scheduler.has_active_animations(),
                "no animation outlives the exit"
            );
            PortalStoreHolder::set_current(Some(Rc::new(PortalStore::new())));
        });
    });
}

#[test]
fn hovering_the_card_pauses_its_countdown() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let controller = ToastController::new(1);
            let host_controller = controller.clone();
            support::InteractiveScene::run(WIDTH, HEIGHT, |scene| {
                scene.install(AnyView::new(
                    AnyView::new(LayerRoot::new(AnyView::new(NotificationHost::new(
                        host_controller.clone(),
                    ))))
                    .frame_size(WIDTH, HEIGHT),
                ));
                // A long duration so the mounting settles cannot elapse it.
                controller.post(
                    ToastNotification::new("Pinned")
                        .id("pinned")
                        .duration(Some(100.0)),
                );
                scene.settle(2, 0.1);
                assert!(controller.needs_clock(), "a fresh card counts down");

                let title = scene
                    .first(|node| {
                        node.layout_node()
                            .and_then(|layout| layout.text_inputs())
                            .is_some_and(|inputs| inputs.text == "Pinned")
                    })
                    .expect("posting mounts the card");
                let frame = title.absolute_frame();
                // One persistent dispatcher: hover state lives on the
                // dispatcher, so a fresh one per event would forget the entry.
                let dispatcher = scene.dispatcher();
                dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                    x: frame.x + frame.width / 2.0,
                    y: frame.y + frame.height / 2.0,
                    delta_x: 0.0,
                    delta_y: 0.0,
                    kind: Default::default(),
                }));
                assert!(
                    !controller.needs_clock(),
                    "hovering anywhere on the card pauses its countdown"
                );

                dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                    x: 2.0,
                    y: 2.0,
                    delta_x: 0.0,
                    delta_y: 0.0,
                    kind: Default::default(),
                }));
                assert!(
                    controller.needs_clock(),
                    "leaving the card resumes the countdown"
                );
            });
        });
    });
}

#[test]
fn reduced_motion_leaves_no_exit_clocks() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            PortalStoreHolder::set_current(Some(Rc::new(PortalStore::new())));
            let (graph, controller) = scene(true);
            let scheduler = cove_core::scene::AnimatorScheduler::current();

            controller.post(ToastNotification::new("Saved").id("saved").duration(None));
            frame(&graph, 0.0);
            let saved = title(&graph, "Saved").expect("posting mounts the card");
            assert_eq!(opacity(&saved), 1.0, "reduced motion snaps the card in");
            assert!(!scheduler.has_active_animations());

            controller.clear();
            frame(&graph, 0.0);
            assert!(
                PortalStoreHolder::current().entries().is_empty(),
                "reduced motion removes the card at once"
            );
            assert!(
                !scheduler.has_active_animations(),
                "clearing leaves no exit clocks"
            );
            PortalStoreHolder::set_current(Some(Rc::new(PortalStore::new())));
        });
    });
}

#[test]
fn reposting_during_exit_rejects_the_stale_completion() {
    let presentation = ToastPresentation::new();
    let original = ToastNotification::new("First").id("same");
    let first = presentation.update(std::slice::from_ref(&original), 2);
    assert_eq!(first.len(), 1, "the first post mounts one card");
    assert!(first[0].is_visible, "the card starts visible");

    let exit = presentation.update(&[], 2);
    assert_eq!(exit.len(), 1, "the dismissed card is retained for its exit");
    let exit_revision = exit[0].revision;
    assert!(!exit[0].is_visible, "the retained card is marked exiting");

    let updated = ToastNotification::new("Updated").id("same");
    let _ = presentation.update(std::slice::from_ref(&updated), 2);
    presentation.remove("same", exit_revision);

    let live = presentation.update(std::slice::from_ref(&updated), 2);
    assert_eq!(live.len(), 1, "the re-post survives the stale removal");
    assert!(live[0].is_visible, "the re-posted card is visible");
    assert_eq!(
        live[0].notification.title, "Updated",
        "the re-posted card carries its new title"
    );
}

#[test]
fn rapid_changes_keep_a_bounded_tree() {
    let presentation = ToastPresentation::new();
    for index in 0..100 {
        let notification = ToastNotification::new("New").id(index.to_string());
        let entries = presentation.update(std::slice::from_ref(&notification), 2);
        assert!(
            entries.len() <= 3,
            "update {index} left {} entries, the tree must stay bounded",
            entries.len()
        );
    }
}
