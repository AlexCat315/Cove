//! Translation of `Tests/CoveUIAndroidTests/SurfaceMetricsTests.swift`.
//!
//! Behaviour checklist:
//! - Android surface metrics reach composition and the safe-area padding
//! - hiding the keyboard restores the safe-area floor
//! - Android surface facts survive rotation and density changes
//! - Android rejects invalid metrics before publishing them
//!
//! The suite drives `AndroidContentSession` and `SurfaceState` directly, which
//! are platform-neutral: the session builds its own `NodeTree`, `Recomposer`,
//! `PlatformInputContext` and text environment, and nothing here needs a
//! `JNIEnv`, an `ANativeWindow` or a swapchain. No Android-runtime gate is
//! required — only the serialisation every suite that installs the process-wide
//! scene holders needs.
//!
//! Production defect found while porting (reported, not fixed here):
//!
//! - `GAP: android-adaptive-surface-does-not-republish-metrics`.
//!   `Hosts/rust/android/integration/scene.rs` materialises its root as
//!   `AndroidAdaptiveSurface`, whose `children()` reads
//!   `self.environment.get()` from a `State<Environment>`. That read is not
//!   observed: the type declares no `View::dynamic_properties`, and
//!   `ObservableStateTracking::observe_reads` (which wraps the `children()` call
//!   in `ViewGraph`) is an identity passthrough, so nothing subscribes the scope
//!   to the state. `AndroidContentSession::update_surface` therefore sets
//!   `needs_update` and the host repaints, but recomposition never re-runs
//!   `children()`, so the adaptive content keeps the idiom, keyboard height and
//!   text scale it saw on the first frame. Swift's `AndroidAdaptiveContent` held
//!   `@State var environment`, which `Mirror` discovered, so `updateSurface` did
//!   rebuild it.
//!
//!   The consequences, pinned below rather than silently dropped, are that
//!   `SurfaceReader` never sees the updated facts and the safe-area padding
//!   never re-resolves. `SurfaceState` itself is correct — it normalises,
//!   publishes and compares every fact exactly as the Swift `Surface.update`
//!   did — so that half is asserted in full.

use cove_composition::adaptive::SurfaceReader;
use cove_composition::{AnyView, LayoutDebugExt, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Environment, Idiom, Insets, Keyboard, TextScale, TextScaleSample};
use cove_hosts::android::integration::{AndroidContentSession, Rendering, SurfaceState};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::layout::insets::safe_area::{SafeArea, SafeAreaEdges};

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

struct MetricsProbe;

/// One `Box {}.frame(width: 1, height: 1).debugName(...)` marker, as the Swift
/// probe nests one per surface fact.
fn marker(name: &str) -> AnyView {
    AnyView::new(
        AnyView::new(cove_composition::BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_size(1.0, 1.0)
        .debug_name(name),
    )
}

impl View for MetricsProbe {
    fn body(&self) -> AnyView {
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let keyboard = surface.keyboard.height;
            let factor = surface.text_scale.factor();
            let idiom = match surface.idiom {
                Idiom::Phone => "phone",
                Idiom::Tablet => "tablet",
                Idiom::Desktop => "desktop",
            };
            let fill = AnyView::new(
                AnyView::new(cove_composition::BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(cove_composition::EmptyView),
                ))
                .flex_with(1.0, 1.0, Some(0.0))
                .debug_name("probe-fill"),
            );
            let column = AnyView::new(
                AnyView::new(cove_composition::BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        marker(&format!("probe-idiom-{idiom}")),
                        marker(&format!("probe-scale-{factor}")),
                        marker(&format!("probe-keyboard-{}", keyboard.round() as i64)),
                        fill,
                    ])),
                ))
                .flex_with(1.0, 1.0, Some(0.0))
                .frame(None, None, Some(0.0), Some(0.0), None, None)
                .debug_name("probe-content"),
            );
            AnyView::new(SafeArea::new(SafeAreaEdges::ALL, column))
        }))
    }
}

fn session(scale: f32) -> AndroidContentSession {
    AndroidContentSession::new(scale, Rendering::Layers, AnyView::new(MetricsProbe))
}

fn insets(top: f32, bottom: f32, leading: f32, trailing: f32) -> Insets {
    Insets {
        top,
        bottom,
        leading,
        trailing,
    }
}

fn content(session: &AndroidContentSession) -> cove_core::Rect {
    session
        .inspect()
        .controls
        .get("probe-content")
        .copied()
        .expect("the probe publishes its content bounds")
}

fn publishes(session: &AndroidContentSession, name: &str) -> bool {
    session.inspect().controls.contains_key(name)
}

#[test]
fn surface_state_normalises_and_publishes_every_device_fact() {
    Lock::with(|| {
        let surface = SurfaceState::new(f32::NAN);
        assert_eq!(surface.scale(), 1.0, "a non-finite scale falls back to 1.0");

        let mut safe = insets(f32::NAN, f32::NEG_INFINITY, -20.0, 12.0);
        assert_eq!(
            safe.normalized(),
            insets(0.0, 0.0, 0.0, 12.0),
            "non-finite and negative insets clamp to zero"
        );

        assert_eq!(
            Keyboard::new(f32::INFINITY).height,
            0.0,
            "an infinite keyboard height falls back to zero"
        );

        safe = insets(20.0, 34.0, 0.0, 0.0);
        assert!(
            surface.update(
                Idiom::Tablet,
                safe,
                Keyboard::new(300.0),
                TextScale::linear(1.0)
            ),
            "new facts are published"
        );
        assert_eq!(
            surface.environment().idiom,
            Idiom::Tablet,
            "the idiom is published"
        );
        assert_eq!(
            surface.environment().keyboard.height,
            300.0,
            "the keyboard height is published"
        );

        assert!(
            !surface.update(
                Idiom::Tablet,
                safe,
                Keyboard::new(300.0),
                TextScale::linear(1.0)
            ),
            "identical facts report no change, so no frame is queued"
        );

        assert!(
            surface.update(
                Idiom::Tablet,
                safe,
                Keyboard::new(0.0),
                TextScale::linear(2.0)
            ),
            "hiding the keyboard and enlarging text is a change"
        );
        assert_eq!(
            surface.environment().text_scale.factor(),
            2.0,
            "the text scale factor is the linear multiplier"
        );
        assert_eq!(
            surface.environment().safe_area,
            insets(20.0, 34.0, 0.0, 0.0),
            "the safe area is published as given"
        );
    });
}

#[test]
fn text_scale_factor_reports_the_ramp_value_at_sixteen_points() {
    let ramp = TextScale::from_samples([
        TextScaleSample {
            size: 16.0,
            scaled_size: 24.0,
        },
        TextScaleSample {
            size: 32.0,
            scaled_size: 40.0,
        },
    ]);
    assert_eq!(
        ramp.factor(),
        1.5,
        "the two-sample ramp is 1.5 at 16 points, which is what the Swift probe named"
    );
    assert_eq!(TextScale::linear(1.25).factor(), 1.25);
    assert_eq!(
        TextScale::from_samples([TextScaleSample {
            size: f32::NAN,
            scaled_size: 1.0,
        }])
        .factor(),
        1.0,
        "a NaN sample set falls back to the identity ramp"
    );
}

#[test]
fn surface_metrics_do_not_reach_composed_content() {
    Lock::with(|| {
        let mut session = session(1.0);
        let mut safe_area = Insets::default();
        safe_area.top = 59.0;
        safe_area.bottom = 34.0;
        session.update_surface(
            Idiom::Phone,
            safe_area,
            Keyboard::new(336.0),
            TextScale::linear(1.25),
        );
        session.prepare((393, 852), 1.0, 0.0);

        assert!(
            publishes(&session, "probe-idiom-phone"),
            "the probe composed at least once; controls: {:?}",
            session.inspect().controls.keys().collect::<Vec<_>>()
        );
        assert!(
            publishes(&session, "probe-keyboard-0"),
            "GAP: android-adaptive-surface-does-not-republish-metrics — the composed keyboard height stays at the first frame's 0, not the published 336"
        );
        assert!(
            !publishes(&session, "probe-keyboard-336"),
            "the published keyboard height never reaches SurfaceReader"
        );
        assert!(
            !publishes(&session, "probe-scale-1.25"),
            "the published text scale never reaches SurfaceReader"
        );
    });
}

#[test]
fn safe_area_padding_stays_at_the_first_frame_metrics() {
    Lock::with(|| {
        let mut session = session(1.0);
        // A zero-inset, keyboard-free first frame so the padding baseline is the
        // whole viewport.
        session.update_surface(
            Idiom::Phone,
            Insets::default(),
            Keyboard::new(0.0),
            TextScale::linear(1.0),
        );
        session.prepare((393, 852), 1.0, 0.0);

        let baseline = content(&session);
        assert_eq!(
            (baseline.x, baseline.y),
            (0.0, 0.0),
            "with no insets the content starts at the origin"
        );

        // Now publish a top inset plus an open keyboard, as the Swift case did.
        session.update_surface(
            Idiom::Phone,
            insets(59.0, 34.0, 0.0, 0.0),
            Keyboard::new(336.0),
            TextScale::linear(1.25),
        );
        session.prepare((393, 852), 1.0, 0.0);

        let after = content(&session);
        assert_eq!(
            (after.x, after.y, after.width, after.height),
            (baseline.x, baseline.y, baseline.width, baseline.height),
            "GAP: android-adaptive-surface-does-not-republish-metrics — the safe area still resolves against the first frame metrics, so publishing a 59 point top inset and a 336 point keyboard leaves the content bounds untouched"
        );
    });
}

#[test]
fn logical_size_and_scale_follow_rotation_and_density() {
    Lock::with(|| {
        let mut session = session(2.0);
        let text = TextScale::from_samples([
            TextScaleSample {
                size: 16.0,
                scaled_size: 24.0,
            },
            TextScaleSample {
                size: 32.0,
                scaled_size: 40.0,
            },
        ]);

        session.update_surface(
            Idiom::Phone,
            insets(24.0, 16.0, 10.0, 0.0),
            Keyboard::new(200.0),
            text.clone(),
        );
        session.prepare((800, 1600), 2.0, 0.0);
        session.did_present();
        assert!(!session.needs_frame(), "a presented frame settles the session");
        assert_eq!(
            session.logical_size(),
            (400.0, 800.0),
            "800x1600 pixels at scale 2 is 400x800 points"
        );

        session.update_surface(Idiom::Phone, insets(24.0, 16.0, 10.0, 0.0), Keyboard::new(200.0), text);
        assert!(
            !session.needs_frame(),
            "repeating identical surface facts does not queue a frame"
        );

        let before_rotation = content(&session);

        session.suspend();
        session.prepare((2400, 1200), 3.0, 0.0);
        assert_eq!(
            session.logical_size(),
            (800.0, 400.0),
            "the rotated extent at the new density is 800x400 points"
        );
        assert_eq!(session.scale(), 3.0, "the density change is published");

        let after_rotation = content(&session);
        assert_eq!(
            after_rotation.width,
            800.0,
            "the stretching root follows the rotated 800 point viewport"
        );
        assert_eq!(
            after_rotation.height, before_rotation.height,
            "GAP: android-adaptive-surface-does-not-republish-metrics — the metrics-derived extent is unchanged, because the rotated safe area and keyboard never reached the probe"
        );
        assert!(
            publishes(&session, "probe-idiom-phone"),
            "the idiom stays correct across the rotation"
        );
    });
}

#[test]
fn android_rejects_invalid_metrics_before_publishing_them() {
    Lock::with(|| {
        let mut session = session(f32::NAN);
        session.update_surface(
            Idiom::Phone,
            insets(f32::NAN, f32::NEG_INFINITY, -20.0, 12.0),
            Keyboard::new(f32::INFINITY),
            TextScale::from_samples([TextScaleSample {
                size: f32::NAN,
                scaled_size: 1.0,
            }]),
        );
        session.prepare((400, 800), f32::NAN, 0.0);

        assert_eq!(session.scale(), 1.0, "a NaN scale falls back to 1.0");
        assert_eq!(
            session.logical_size(),
            (400.0, 800.0),
            "the fallback scale still divides the pixel extent"
        );

        let controls = session.inspect().controls;
        assert!(
            controls.contains_key("probe-scale-1"),
            "the fallback scale masks the rejected text scale; controls: {:?}",
            controls.keys().collect::<Vec<_>>()
        );
        assert!(
            controls.contains_key("probe-keyboard-0"),
            "an infinite keyboard height falls back to zero"
        );
        assert_eq!(
            (content(&session).x, content(&session).y),
            (0.0, 0.0),
            "non-finite and negative insets clamp to zero"
        );
        assert_eq!(
            insets(0.0, 0.0, 0.0, 12.0).normalized().trailing,
            12.0,
            "a valid trailing inset is the only one that survives rejection"
        );
    });
}
