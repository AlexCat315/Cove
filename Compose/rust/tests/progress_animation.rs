//! Translation of `Tests/CoveUIComposeTests/ProgressAnimationTests.swift`.
//!
//! Behaviour checklist:
//! - both progress controls interpolate their value through the animator
//! - the node stays layout-clean while its paint is dirty during the animation
//! - an explicit nil animation context snaps the value instead of animating
//! - switching to indeterminate and back snaps to each new value
//! - reduced motion snaps rather than animating

mod support;

use cove_composition::PrimitiveView;
use cove_core::scene::{ActiveAnimationContext, NodeTree};
use cove_ui_compose::primitives::feedback::{ProgressCircle, ProgressValue, ProgressView};

use support::GlobalTestLock;

#[test]
fn both_progress_controls_interpolate_values() {
    GlobalTestLock::locked(|| {
        let scheduler = cove_core::scene::AnimatorScheduler::default();
        scheduler.with_current(|| {
            for circular in [false, true] {
                let (initial, updated): (Box<dyn PrimitiveView>, Box<dyn PrimitiveView>) =
                    if circular {
                        (
                            Box::new(ProgressCircle::new(Some(0.2))),
                            Box::new(ProgressCircle::new(Some(0.8))),
                        )
                    } else {
                        (
                            Box::new(ProgressView::new(Some(0.2))),
                            Box::new(ProgressView::new(Some(0.8))),
                        )
                    };

                let node = initial.make_node();
                initial.update_node(&node);
                let presentation = node
                    .first_resource::<ProgressValue>()
                    .expect("the presentation resource is installed");
                assert_eq!(presentation.value(), 0.2, "the initial value is presented");

                updated.update_node(&node);
                assert_eq!(
                    presentation.value(),
                    0.2,
                    "the update animates rather than snapping"
                );

                let mut tree = NodeTree::new();
                tree.set_root(Some(node.clone()));
                tree.flush();
                cove_core::scene::AnimatorScheduler::current().tick(0.04);
                assert!(!node.is_dirty(), "the animation does not dirty layout");
                assert!(node.render_dirty(), "the animation repaints");
                let mid = presentation.value();
                assert!(
                    mid > 0.2 && mid < 0.8,
                    "mid-animation value {mid} sits strictly between the endpoints"
                );

                cove_core::scene::AnimatorScheduler::current().tick(1.0);
                assert_eq!(presentation.value(), 0.8, "the animation lands exactly");
            }
            assert!(
                !cove_core::scene::AnimatorScheduler::current().has_active_animations(),
                "no animation outlives the tick that finished it"
            );
        });
    });
}

#[test]
fn disabled_animation_and_mode_changes_snap() {
    GlobalTestLock::locked(|| {
        let scheduler = cove_core::scene::AnimatorScheduler::default();
        scheduler.with_current(|| {
            let start = ProgressView::new(Some(0.2));
            let node = start.make_node();
            start.update_node(&node);

            ActiveAnimationContext::with(None, None, None, || {
                ProgressView::new(Some(0.9)).update_node(&node);
            });
            let presentation = node
                .first_resource::<ProgressValue>()
                .expect("the presentation resource is installed");
            assert_eq!(
                presentation.value(),
                0.9,
                "a nil animation context snaps to the new value"
            );

            // Indeterminate then determinate again: each switch snaps.
            ProgressView::new(None).update_node(&node);
            ProgressView::new(Some(0.4)).update_node(&node);
            assert_eq!(
                presentation.value(),
                0.4,
                "returning from indeterminate adopts the new value"
            );

            // Reduced motion snaps.
            node.set_composition_value(&cove_core::scene::ReducedMotionEnvironment::key(), true);
            ProgressView::new(Some(0.7)).update_node(&node);
            assert_eq!(
                presentation.value(),
                0.7,
                "reduced motion snaps instead of animating"
            );
        });
    });
}
