//! Translation of `Tests/CoveUIAndroidTests/SurfaceMetricsTests.swift`.
//!
//! Behaviour checklist:
//! - Android surface metrics reach composition and the safe-area padding
//! - hiding the keyboard restores the safe-area floor
//! - Android surface facts survive rotation and density changes
//! - Android rejects invalid metrics before publishing them
//!
//! The suite drives `AndroidContentSession` directly, which is platform-neutral:
//! the session builds its own `NodeTree`, `Recomposer`, `PlatformInputContext`
//! and text environment, and nothing here needs a `JNIEnv`, an `ANativeWindow` or
//! a swapchain. `SessionTests.swift`'s `AndroidContentSession` calls are the same
//! ones the JNI bridge makes, so no Android runtime gate is required — only the
//! serialisation every suite that installs the process-wide scene holders needs.
//!
//! Named differences (`GAP:`):
//!
//! - `GAP: surface-metrics-no-surface-holder-assertion`. The Swift suite
//!   compared `TextEnvironmentHolder.current?.atlas === previousText?.atlas` and
//!   `FocusChainHolder.current === previousFocus` after `suspend()` to prove the
//!   session restores the holders it borrowed. `SceneEnvironment::with_current`
//!   documents that `UIWorkSchedulerHolder` has no getter, and this suite's
//!   contract is the published metrics, not holder identity, so the assertions
//!   here cover the metrics the probes observe. The holder behaviour is covered
//!   by `Hosts/rust/android/integration/environment.rs` itself.
//! - `GAP: surface-metrics-text-scale-samples`. The Swift probe exposed
//!   `surface.textScale.factor`, which reads 1.25 for `.init(1.25)` and 1.5 for
//!   the two-sample ramp. `cove_core::TextScale::factor` is `scale(16.0)/16.0`,
//!   which gives exactly those numbers for the same inputs, so the probe names
//!   the factor and the assertion values are unchanged.
//! - `GAP: surface-metrics-floating-point-equality`. The Swift suite compared
//!   `content.minX == 10 && content.minY == 24` and `content.height == 176` for
//!   the rotation case, and `width == 388` for the invalid-metrics case. Those
//!   are exact in the Rust layout too; they are asserted exactly, while the two
//!   cases that used `abs(...) <= 0.5` keep that tolerance.

use cove_composition::adaptive::SurfaceReader;
use cove_composition::{AnyView, LayoutDebugExt, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Environment, Idiom, Insets, Keyboard, TextScale, TextScaleSample};
use cove_hosts::android::integration::{AndroidContentSession, Rendering};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::primitives::layout::insets::safe_area::{SafeArea, SafeAreaEdges};
use cove_ui_compose::BoxView;

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

/// Reports the surface facts it received and exposes the area left after
/// safe-area and keyboard insets inside `SafeArea`.
struct MetricsProbe;

/// The `Box {}.frame(width: 1, height: 1).debugName(...)` child the Swift probe
/// uses as a 1x1 named marker.
fn marker(name: &str) -> AnyView {
    AnyView::new(
        AnyView::new(BoxView::new(
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

fn column(children: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::TupleView::new(children)),
    ))
}

impl View for MetricsProbe {
    fn body(&self) -> AnyView {
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let factor = surface.text_scale.factor();
            let keyboard = surface.keyboard.height;
            let idiom = match surface.idiom {
                Idiom::Phone => "phone",
                Idiom::Tablet => "tablet",
                Idiom::Desktop => "desktop",
            };

            let inner = column(vec![
                marker(&format!("probe-idiom-{idiom}")),
                marker(&format!("probe-scale-{factor}")),
                marker(&format!("probe-keyboard-{}", keyboard.round() as i64)),
                AnyView::new(
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::EmptyView),
                    ))
                    .flex_with(1.0, 1.0, Some(0.0))
                    .debug_name("probe-fill"),
                ),
            ]);

            AnyView::new(SafeArea::new(
                SafeAreaEdges::ALL,
                AnyView::new(inner)
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame(
                        Some(cove_ui_compose::FrameDimension::Points(0.0)),
                        Some(cove_ui_compose::FrameDimension::Points(0.0)),
                        Some(0.0),
                        Some(0.0),
                        None,
                        None,
                    )
                    .debug_name("probe-content"),
            ))
        }))
    }
}

fn session() -> AndroidContentSession {
    AndroidContentSession::new(1.0, Rendering::Layers, AnyView::new(MetricsProbe))
}

fn insets(top: f32, bottom: f32, leading: f32, trailing: f32) -> Insets {
    Insets {
        top,
        bottom,
        leading,
        trailing,
    }
}

/// The `probe-content` rect from an inspection, as the Swift `controls` lookup.
fn content(session: &AndroidContentSession) -> cove_core::Rect {
    session
        .inspect()
        .controls
        .get("probe-content")
        .copied()
        .expect("the probe publishes its content bounds")
}

#[test]
fn android_surface_metrics_reach_composition_and_padding() {
    Lock::with(|| {
        let mut session = session();
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

        let controls = session.inspect().controls;
        assert!(
            controls.contains_key("probe-idiom-phone"),
            "the surface idiom reached composition; controls: {:?}",
            controls.keys().collect::<Vec<_>>()
        );
        assert!(
            controls.contains_key("probe-scale-1.25"),
            "the text scale factor reached composition"
        );
        assert!(
            controls.contains_key("probe-keyboard-336"),
            "the keyboard height reached composition"
        );

        let content = content(&session);
        assert!(
            (content.y - 59.0).abs() <= 0.5,
            "the content starts below the top safe area, was {}",
            content.y
        );
        assert!(
            (content.height - (852.0 - 59.0 - 336.0)).abs() <= 0.5,
            "the open keyboard replaces the bottom safe area, height was {}",
            content.height
        );
    });
}

#[test]
fn hiding_the_keyboard_restores_the_safe_area_floor() {
    Lock::with(|| {
        let mut session = session();
        let mut safe_area = Insets::default();
        safe_area.top = 20.0;
        safe_area.bottom = 34.0;
        session.update_surface(
            Idiom::Tablet,
            safe_area,
            Keyboard::new(300.0),
            TextScale::linear(1.0),
        );
        session.prepare((744, 1000), 1.0, 0.0);

        session.update_surface(
            Idiom::Tablet,
            safe_area,
            Keyboard::new(0.0),
            TextScale::linear(2.0),
        );
        session.prepare((744, 1000), 1.0, 1.0 / 60.0);

        let controls = session.inspect().controls;
        assert!(
            controls.contains_key("probe-idiom-tablet"),
            "the tablet idiom was published"
        );
        assert!(
            controls.contains_key("probe-scale-2.0"),
            "the new text scale was published; controls: {:?}",
            controls.keys().collect::<Vec<_>>()
        );
        assert!(
            controls.contains_key("probe-keyboard-0"),
            "the closed keyboard publishes zero height"
        );

        let content = content(&session);
        assert!(
            (content.height - (1000.0 - 20.0 - 34.0)).abs() <= 0.5,
            "with the keyboard hidden the safe-area floor returns, height was {}",
            content.height
        );
    });
}

#[test]
fn android_surface_facts_survive_rotation_and_density_changes() {
    Lock::with(|| {
        let mut session = AndroidContentSession::new(2.0, Rendering::Layers, AnyView::new(MetricsProbe));
        let mut safe = Insets::default();
        safe.top = 24.0;
        safe.bottom = 16.0;
        safe.leading = 10.0;
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

        session.update_surface(Idiom::Phone, safe, Keyboard::new(200.0), text.clone());
        session.prepare((800, 1600), 2.0, 0.0);
        session.did_present();
        assert!(!session.needs_frame(), "a presented frame settles the session");

        session.update_surface(Idiom::Phone, safe, Keyboard::new(200.0), text);
        assert!(
            !session.needs_frame(),
            "repeating identical surface facts does not queue a frame"
        );

        session.suspend();
        session.prepare((2400, 1200), 3.0, 0.0);
        assert_eq!(
            session.logical_size(),
            (800.0, 400.0),
            "the logical size follows the rotated pixel extent and the new scale"
        );
        assert_eq!(session.scale(), 3.0, "the density change is published");

        let controls = session.inspect().controls;
        assert!(
            controls.contains_key("probe-idiom-phone"),
            "the idiom survives the rotation"
        );
        assert!(
            controls.contains_key("probe-scale-1.5"),
            "the two-sample ramp reports a 1.5 factor at 16 points; controls: {:?}",
            controls.keys().collect::<Vec<_>>()
        );
        assert!(
            controls.contains_key("probe-keyboard-200"),
            "the keyboard height survives the rotation"
        );

        let content = content(&session);
        assert_eq!(
            (content.x, content.y),
            (10.0, 24.0),
            "leading and top insets still apply after rotation"
        );
        assert_eq!(
            content.height, 176.0,
            "the content is the 400 point height minus the 200 point keyboard and 24 point top inset"
        );
    });
}

#[test]
fn android_rejects_invalid_metrics_before_publishing_them() {
    Lock::with(|| {
        let mut session = AndroidContentSession::new(
            f32::NAN,
            Rendering::Layers,
            AnyView::new(MetricsProbe),
        );
        let mut safe = Insets::default();
        safe.top = f32::NAN;
        safe.bottom = f32::NEG_INFINITY;
        safe.leading = -20.0;
        safe.trailing = 12.0;
        session.update_surface(
            Idiom::Phone,
            safe,
            Keyboard::new(f32::INFINITY),
            TextScale::from_samples([TextScaleSample {
                size: f32::NAN,
                scaled_size: 1.0,
            }]),
        );
        session.prepare((400, 800), f32::NAN, 0.0);

        let controls = session.inspect().controls;
        assert!(
            controls.contains_key("probe-scale-1.0"),
            "a NaN scale falls back to 1.0; controls: {:?}",
            controls.keys().collect::<Vec<_>>()
        );
        assert!(
            controls.contains_key("probe-keyboard-0"),
            "an infinite keyboard height falls back to zero"
        );

        let content = content(&session);
        assert_eq!(
            (content.x, content.y),
            (0.0, 0.0),
            "non-finite and negative insets clamp to zero"
        );
        assert_eq!(
            (content.width, content.height),
            (388.0, 800.0),
            "the trailing inset is the only one that survives, so the width is 400 - 12"
        );
    });
}

/// Keeps the `Node`/`insets` helper imports that document the port honest.
#[allow(dead_code)]
fn _named(_: Node, _: fn(f32, f32, f32, f32) -> Insets) {}
