//! Translation of `Tests/Headless/Tests/CoveUISceneTests/ObservationTests.swift`.
//!
//! Behaviour checklist:
//! - state registration is explicit, lazy and follows scope lifetime
//! - observed values track binding writes and the timeline records actual work
//! - the timeline is bounded and new captures keep sequence identity
//!
//! The DevTools wire-payload half of the original is not ported: `DevToolsSession`,
//! `DevToolsEnvelope` and `TimingFramePayload` live in the DevTools crate, which is
//! outside this test's crate. Everything below exercises the scene-side registry
//! and timeline those payloads carry.

use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_core::DrawList;
use cove_core::scene::NodeRenderer;
use cove_core::scene::{NodeTree, PerformanceTimeline, RecompositionReason, StateRegistry};

struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn locked<T>(body: impl FnOnce() -> T) -> T {
        thread_local! {
            static HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        if HELD.get() {
            return body();
        }
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                HELD.set(false);
            }
        }
        HELD.set(true);
        let _release = Release;
        body()
    }
}

#[test]
fn registration_is_lazy_and_deduplicated() {
    GlobalTestLock::locked(|| {
        let mut registry = StateRegistry::new();
        let reads = std::rc::Rc::new(RefCell::new(0));
        let counter = reads.clone();
        let provider = registry
            .register(
                Some("host".to_string()),
                "host",
                "String",
                None,
                move || {
                    *counter.borrow_mut() += 1;
                    "x".repeat(2000)
                },
            )
            .expect("the provider registers");

        assert_eq!(
            registry.descriptors().len(),
            1,
            "one provider is registered"
        );
        assert_eq!(
            *reads.borrow(),
            0,
            "registration does not evaluate the value"
        );

        // The same id twice collapses to one read; an unknown id yields nothing.
        let values = registry.values(&[provider.clone(), provider.clone(), "missing".to_string()]);
        assert_eq!(*reads.borrow(), 1, "a duplicated id reads once");
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].summary.chars().count(),
            1024,
            "the summary truncates to 1024 chars"
        );
        assert!(values[0].truncated, "truncation is reported");

        registry.unregister(&provider);
        assert!(
            registry.values(&[provider]).is_empty(),
            "an unregistered provider reads nothing"
        );
    });
}

#[test]
fn observed_values_track_writes() {
    GlobalTestLock::locked(|| {
        // A provider whose value the test writes through, standing in for a
        // `@State` member exposed to DevTools.
        let value = std::rc::Rc::new(std::cell::Cell::new(3));
        let mut registry = StateRegistry::new();
        let read_value = value.clone();
        let provider = registry
            .register(
                Some("scope.count".to_string()),
                "count",
                "Int",
                Some("scope".to_string()),
                move || read_value.get().to_string(),
            )
            .expect("the provider registers");

        let descriptor = registry
            .descriptors()
            .into_iter()
            .next()
            .expect("a descriptor exists");
        assert_eq!(descriptor.name, "count");
        assert_eq!(descriptor.scope_id.as_deref(), Some("scope"));

        assert_eq!(
            registry.values(std::slice::from_ref(&provider))[0].summary,
            "3",
            "the initial value is read"
        );

        // A write is visible to the next read; registration is unchanged.
        value.set(9);
        assert_eq!(
            registry.values(&[provider])[0].summary,
            "9",
            "the write reaches the reader"
        );
        assert_eq!(registry.descriptors().len(), 1, "writes do not re-register");
    });
}

#[test]
fn timeline_is_bounded_and_keeps_sequence_identity() {
    GlobalTestLock::locked(|| {
        let mut timeline = PerformanceTimeline::new(2);
        timeline.set_enabled(true);

        for _ in 0..3 {
            let start = timeline.begin();
            timeline.end(start, "draw", "Draw", None, &[]);
        }

        assert_eq!(
            timeline.events().len(),
            2,
            "the ring keeps only its capacity"
        );
        assert_eq!(
            timeline.dropped(),
            1,
            "the oldest capture is reported dropped"
        );
        assert_eq!(
            timeline
                .events_after(2)
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            [3],
            "sequences continue past the evicted entries",
        );
        assert_eq!(
            timeline
                .events_after(0)
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            [2, 3],
        );
        assert!(
            timeline.events_after(3).is_empty(),
            "nothing follows the newest capture"
        );

        let latest = timeline.events().last().expect("a capture exists").sequence;

        // Re-enabling resets the buffer and the drop count but not the sequence.
        timeline.set_enabled(false);
        timeline.set_enabled(true);
        assert!(
            timeline.events().is_empty(),
            "re-enabling clears the buffer"
        );
        assert_eq!(timeline.dropped(), 0, "re-enabling clears the drop count");

        let start = timeline.begin();
        timeline.end(start, "layout", "Layout", None, &[]);
        assert!(
            timeline.events()[0].sequence > latest,
            "a later capture keeps a larger sequence",
        );
    });
}

#[test]
fn disabled_timeline_records_nothing() {
    GlobalTestLock::locked(|| {
        let mut timeline = PerformanceTimeline::new(16);
        assert!(
            timeline.begin().is_none(),
            "a disabled timeline hands out no start token"
        );
        timeline.set_enabled(true);
        assert!(
            timeline.begin().is_some(),
            "an enabled timeline hands out a start token"
        );
        timeline.set_enabled(false);
        let before = timeline.events().len();
        timeline.end(None, "draw", "Draw", None, &[]);
        assert_eq!(
            timeline.events().len(),
            before,
            "a disabled timeline records nothing"
        );
    });
}

#[test]
fn node_tree_timeline_captures_paint_work() {
    GlobalTestLock::locked(|| {
        let tree = NodeTree::new();
        let root = cove_core::scene::Node::new();
        root.set_frame(cove_core::Rect::new(0.0, 0.0, 10.0, 10.0));
        root.set_background_color(Some(cove_core::Color::WHITE));
        let mut tree = tree;
        tree.set_root(Some(root.clone()));
        assert!(
            tree.timeline.borrow().events().is_empty(),
            "a fresh tree captures nothing"
        );

        tree.timeline.borrow_mut().set_enabled(true);
        let start = tree.timeline.borrow().begin();
        let mut list = DrawList::default();
        NodeRenderer.render(&root, &mut list);
        let reasons: Vec<RecompositionReason> = Vec::new();
        tree.timeline
            .borrow_mut()
            .end(start, "draw", "Paint", None, &reasons);

        let events = tree.timeline.borrow().events();
        assert_eq!(events.len(), 1, "the paint pass is captured");
        assert_eq!(events[0].phase, "draw");
        assert_eq!(events[0].name, "Paint");
        assert!(events[0].start_ms >= 0.0 && events[0].duration_ms >= 0.0);
        assert!(
            events[0].duration_ms.is_finite(),
            "the duration is a finite measurement"
        );
    });
}
