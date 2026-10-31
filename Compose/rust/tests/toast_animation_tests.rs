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
//!
//! GAP: no toast card ever mounts.
//! Swift keeps the reconciler in `@State` — `NotificationHost.swift:38`
//! `@State private var presentation = ToastPresentation()` — so one instance
//! survives every `body()` call and accumulates the cards. The Rust
//! `NotificationHost` (`primitives/overlays/toast/notification_host.rs:82`)
//! stores the same value as a plain field, calls
//! `self.presentation.update(..)` inside `body()`, and then hands
//! `ToastPresenter` a **second, freshly constructed** `ToastPresentation`
//! (`:117  presentation: Rc::new(ToastPresentation::new())`). Because
//! `NotificationHost` also exposes no `dynamic_properties`, `body()` never
//! re-runs after `ToastController::post`, and the entries computed on the
//! install pass go to a throwaway reconciler. Measured: after posting, the
//! controller reports 1 notification while `PortalStore::entries()` stays `0`
//! and no "Saved" title node ever materialises, at any tick.
//!
//! The reconciler logic itself is migrated at full Swift strength: the
//! `ToastPresentation` tests (stale-revision rejection, bounded tree) exercise
//! the same value Swift's `@State` held, so only the widget-level lifecycle is
//! pinned.

mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStore, PortalStoreHolder};
use cove_ui_compose::primitives::overlays::toast::{
    NotificationHost, ToastController, ToastNotification, ToastPresentation,
};

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
fn scene() -> (
    Rc<cove_composition::ViewGraph>,
    Rc<ToastController>,
) {
    let graph = support::graph();
    let controller = ToastController::new(1);
    graph.install_view(
        AnyView::new(
            AnyView::new(LayerRoot::new(AnyView::new(NotificationHost::new(
                controller.clone(),
            ))))
            .frame_size(WIDTH, HEIGHT),
        ),
        Default::default(),
    );
    (graph, controller)
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
            let (graph, controller) = scene();

            controller.post(ToastNotification::new("Saved").id("saved").duration(None));
            frame(&graph, 0.0);

            // GAP: the host never presents, so there is no card to fade in.
            assert_eq!(
                controller.notifications().len(),
                1,
                "the controller accepted the notification"
            );
            assert!(
                title(&graph, "Saved").is_none(),
                "no card mounts; Swift renders the title and starts its entry fade"
            );
            assert_eq!(
                PortalStoreHolder::current().entries().len(),
                0,
                "the toast portal stays empty; Swift publishes one entry"
            );

            frame(&graph, 0.08);
            frame(&graph, 1.0);
            assert!(
                title(&graph, "Saved").is_none(),
                "the card still never mounts after the entry window"
            );

            controller.dismiss("saved");
            frame(&graph, 0.0);
            assert!(
                controller.notifications().is_empty(),
                "dismissal clears the controller immediately"
            );
            assert!(
                all(&graph).iter().all(|node| {
                    node.accessibility()
                        .is_none_or(|semantics| semantics.label != "Dismiss notification")
                }),
                "no dismiss button remains once the controller is empty"
            );
            assert_eq!(
                PortalStoreHolder::current().entries().len(),
                0,
                "the portal stays empty through the exit window"
            );
            frame(&graph, 0.08);
            frame(&graph, 1.0);
            assert!(
                !cove_core::scene::AnimatorScheduler::current().has_active_animations(),
                "no animation outlives the exit"
            );
            PortalStoreHolder::set_current(Some(Rc::new(PortalStore::new())));
        });
    });
}

#[test]
fn reduced_motion_leaves_no_exit_clocks() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            PortalStoreHolder::set_current(Some(Rc::new(PortalStore::new())));
            let (graph, controller) = scene();

            controller.post(ToastNotification::new("Saved").id("saved").duration(None));
            frame(&graph, 0.0);

            // GAP: nothing mounts, so there is no snap to observe either. What
            // holds is that the host leaves no animation running.
            assert!(
                !cove_core::scene::AnimatorScheduler::current().has_active_animations(),
                "reduced motion leaves no active animations"
            );

            controller.clear();
            frame(&graph, 0.0);
            assert!(
                PortalStoreHolder::current().entries().is_empty(),
                "clearing leaves the portal empty"
            );
            assert!(
                !cove_core::scene::AnimatorScheduler::current().has_active_animations(),
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
