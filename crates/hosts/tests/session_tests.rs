//! Translation of `Tests/CoveUIAndroidTests/SessionTests.swift`.
//!
//! Behaviour checklist:
//! - the Android host retains content but releases input across surface
//!   lifetimes: repeated prepares produce the same draw list, a pointer click
//!   mutates the retained content state, the host reports frame needs until it
//!   presents, suspending drops focus, and a surface recreation at a different
//!   density keeps the content and its state.
//!
//! The suite drives `AndroidContentSession` directly, which is platform-neutral:
//! the session builds its own `NodeTree`, `Recomposer`, `PlatformInputContext`
//! and text environment, and nothing here needs a `JNIEnv`, an `ANativeWindow` or
//! a swapchain. The calls are the ones the JNI bridge makes, so no
//! Android-runtime gate is required — only the serialisation every suite that
//! installs the process-wide scene holders needs.
//!

use cove_composition::Scope;
use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::State;
use cove_hosts::android::integration::{AndroidContentSession, Rendering};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::text::Text;

/// Serialises the suites that install the process-wide scene holders.
struct Lock;

impl Lock {
    fn with<T>(body: impl FnOnce() -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        body()
    }
}

/// The Swift `Content` view: a stateful counter button.
struct Content {
    count: State<i32>,
}

impl View for Content {
    fn body(&self, _cx: &Scope) -> AnyView {
        let read = self.count.clone();
        let bump = self.count.clone();
        AnyView::new(
            Button::new(Text::new(format!("Count {}", read.get())), move || {
                bump.set(bump.get() + 1)
            })
            .debug_name("increment")
            .frame_size(120.0, 48.0),
        )
    }
}

fn session(scale: f32, count: &State<i32>) -> AndroidContentSession {
    AndroidContentSession::new(
        scale,
        Rendering::Layers,
        AnyView::new(Content {
            count: count.clone(),
        }),
    )
}

#[test]
fn android_host_retains_content_but_releases_input_across_surface_lifetimes() {
    Lock::with(|| {
        let count = State::new(0);
        let mut session = session(1.0, &count);

        let mut first = session.prepare((200, 100), 1.0, 0.0);
        let index_count = first.indices().len();
        assert!(index_count > 0, "the first frame draws geometry");
        first.reset();

        let next = session.prepare((200, 100), 1.0, 0.0);
        assert_eq!(
            next.indices().len(),
            index_count,
            "an unchanged surface reproduces the same draw list"
        );

        let bounds = *session
            .inspect()
            .controls
            .get("increment")
            .expect("the counter button is inspectable by name");
        assert_eq!(
            (bounds.width, bounds.height),
            (120.0, 48.0),
            "the button keeps its authored frame"
        );

        session.did_submit(1);
        session.presentation_finished(1, true, 0.0, None);
        assert!(
            !session.needs_frame(),
            "presenting clears the pending frame request"
        );

        let x = bounds.x + bounds.width / 2.0;
        let y = bounds.y + bounds.height / 2.0;
        session.pointer(0, x, y);
        session.pointer(1, x, y);
        assert_eq!(
            count.get(),
            1,
            "the click ran the retained content's action"
        );
        assert!(
            session.needs_frame(),
            "input queues a frame so the new state is painted"
        );

        session.prepare((200, 100), 1.0, 1.0 / 120.0);
        session.did_submit(2);
        session.presentation_finished(2, true, 0.0, None);

        // The press releases with a short spring, so the click settles over a
        // few frames; the Swift suite allowed up to 120 for trailing animation.
        let mut settled_after = 0;
        while session.needs_frame() && settled_after < 120 {
            session.prepare((200, 100), 1.0, 1.0 / 60.0);
            session.did_submit(100 + settled_after);
            session.presentation_finished(100 + settled_after, true, 0.0, None);
            settled_after += 1;
        }
        assert!(
            !session.needs_frame(),
            "the session settles within 120 frames (took {settled_after})"
        );

        session.suspend();
        assert_eq!(
            session.focused_id(),
            None,
            "suspending releases input focus"
        );

        session.invalidate();
        session.prepare((400, 200), 2.0, 0.0);
        session.did_submit(4);
        session.presentation_finished(4, true, 0.0, None);

        assert_eq!(
            count.get(),
            1,
            "surface recreation keeps the content and its state"
        );
        assert_eq!(
            session.logical_size(),
            (200.0, 100.0),
            "400x200 pixels at scale 2 is the original 200x100 points"
        );
    });
}

#[test]
fn surface_recreation_at_a_new_scale_publishes_the_new_density() {
    Lock::with(|| {
        let count = State::new(0);
        let mut session = session(1.0, &count);
        session.prepare((200, 100), 1.0, 0.0);
        session.did_submit(5);
        session.presentation_finished(5, true, 0.0, None);
        assert_eq!(session.scale(), 1.0, "the session starts at the host scale");

        session.suspend();
        session.prepare((300, 150), 3.0, 0.0);
        session.did_submit(6);
        session.presentation_finished(6, true, 0.0, None);
        assert_eq!(session.scale(), 3.0, "the new density is published");
        assert_eq!(
            session.logical_size(),
            (100.0, 50.0),
            "300x150 pixels at scale 3 is 100x50 points"
        );
        assert_eq!(
            count.get(),
            0,
            "recreating the surface does not re-run the content's action"
        );

        let bounds = *session
            .inspect()
            .controls
            .get("increment")
            .expect("the button survives the recreation");
        assert_eq!(
            (bounds.width, bounds.height),
            (120.0, 48.0),
            "the content keeps its authored geometry across the density change"
        );
    });
}

#[test]
fn pointer_input_is_ignored_while_it_is_not_finite() {
    Lock::with(|| {
        let count = State::new(0);
        let mut session = session(1.0, &count);
        session.prepare((200, 100), 1.0, 0.0);
        session.did_submit(7);
        session.presentation_finished(7, true, 0.0, None);

        let bounds = *session
            .inspect()
            .controls
            .get("increment")
            .expect("the counter button is inspectable");
        let x = bounds.x + bounds.width / 2.0;

        session.pointer(0, f32::NAN, 0.0);
        session.pointer(1, f32::NAN, 0.0);
        assert_eq!(
            count.get(),
            0,
            "a non-finite pointer position is dropped before it can click"
        );

        session.pointer(0, x, bounds.y + bounds.height / 2.0);
        session.pointer(1, x, bounds.y + bounds.height / 2.0);
        assert_eq!(
            count.get(),
            1,
            "a finite press inside the button still runs its action"
        );
    });
}
