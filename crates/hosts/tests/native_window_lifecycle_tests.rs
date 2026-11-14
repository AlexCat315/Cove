//! Translation of `Tests/CoveUITests/NativeWindowLifecycleTests.swift`.
//!
//! Behaviour checklist:
//! - the renderer starts unconfigured and closed after `close`
//! - suspending presentation clears the swapchain and target memory
//! - drawing while suspended presents nothing
//! - resuming does not allocate a swapchain until the next draw
//! - target memory follows the latest configured extent
//!
//! Named difference (documented, not silently dropped): the Swift suite drove a
//! live `CAMetalLayer` on an `NSWindow` so `configure` could create a real
//! swapchain, then asserted `estimatedTargetBytes == side*side*4*5` after each
//! resume. Creating that layer needs `objc2-quartz-core`, which this crate does
//! not depend on, and `AppWindowRenderer::configure` requires a
//! `NativeRenderSurface` the test process cannot fabricate. The assertions here
//! therefore cover the state machine and the no-swapchain/no-presentation
//! guarantees, which is the half that was regressing; the swapchain-backed
//! memory accounting stays behind the host integration path.

#![cfg(target_os = "macos")]

use cove_core::scene::DrawList;
use cove_core::{Color, Rect};
use cove_graphics::rhi::{Device, DeviceConfig, GraphicsApi};
use cove_graphics::ui::NativeDrawListRenderer;
use cove_hosts::desktop::application::memory::AppWindowRendererState;
use cove_hosts::desktop::application::{AppWindowRendering, ApplicationConfiguration};
use cove_hosts::desktop::window::native_renderer::{
    AppWindowRenderSettings, AppWindowRenderer, NativeAppWindowRenderer,
};

fn window_renderer() -> Option<NativeAppWindowRenderer> {
    let device = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: true,
        frames_in_flight: 3,
        ..DeviceConfig::default()
    })
    .ok()
    .map(std::sync::Arc::new)?;
    let renderer = NativeDrawListRenderer::new(device.clone()).ok()?;
    Some(NativeAppWindowRenderer::new(
        device,
        renderer,
        AppWindowRenderSettings::resolve(
            &ApplicationConfiguration::default(),
            AppWindowRendering::default(),
        ),
    ))
}

fn window_list() -> DrawList {
    let mut list = DrawList::default();
    let _ = list.add_rect(Rect::new(0.0, 0.0, 64.0, 64.0), Color::WHITE);
    list
}

#[test]
fn renderer_starts_unconfigured_without_swapchain_or_memory() {
    let Some(window) = window_renderer() else {
        eprintln!("no Metal device available; skipping the native window lifecycle test");
        return;
    };

    assert_eq!(
        window.state(),
        AppWindowRendererState::Unconfigured,
        "a freshly built renderer has no surface"
    );
    let snapshot = window.memory_snapshot();
    assert!(!snapshot.has_swapchain, "no swapchain before configure");
    assert_eq!(
        snapshot.estimated_target_bytes, 0,
        "no target memory before configure"
    );
    assert!(
        !window.is_configured(),
        "the renderer reports itself unconfigured"
    );
}

#[test]
fn drawing_before_configure_presents_nothing() {
    let Some(mut window) = window_renderer() else {
        eprintln!("no Metal device available; skipping the native window lifecycle test");
        return;
    };
    let mut list = window_list();

    assert!(
        !window
            .draw(&mut list, (64.0, 64.0), None)
            .expect("draw before configure"),
        "an unconfigured window cannot present"
    );
    assert_eq!(
        window.memory_snapshot().estimated_target_bytes,
        0,
        "drawing without a surface allocates nothing"
    );
}

#[test]
fn suspend_and_resize_without_a_surface_stay_safe() {
    let Some(mut window) = window_renderer() else {
        eprintln!("no Metal device available; skipping the native window lifecycle test");
        return;
    };
    let mut list = window_list();

    window
        .set_presentation_active(false)
        .expect("suspend an unconfigured window");
    let snapshot = window.memory_snapshot();
    assert!(!snapshot.has_swapchain, "there is no swapchain to release");
    assert_eq!(snapshot.estimated_target_bytes, 0);

    assert!(
        !window
            .draw(&mut list, (64.0, 64.0), None)
            .expect("draw while suspended"),
        "a suspended window draws without presenting"
    );

    window
        .set_presentation_active(true)
        .expect("resume an unconfigured window");
    assert!(
        !window.memory_snapshot().has_swapchain,
        "resuming alone never allocates a swapchain"
    );

    window
        .resize((80, 80), false)
        .expect("resize without a surface");
    assert!(
        !window.memory_snapshot().has_swapchain,
        "resizing without a surface allocates nothing"
    );
    assert_eq!(
        window.memory_snapshot().estimated_target_bytes,
        0,
        "a surface-less renderer owns no target memory at any size"
    );
}

#[test]
fn close_returns_to_a_closed_state_without_memory() {
    let Some(mut window) = window_renderer() else {
        eprintln!("no Metal device available; skipping the native window lifecycle test");
        return;
    };

    window.close().expect("close");
    let snapshot = window.memory_snapshot();
    assert_eq!(snapshot.state, AppWindowRendererState::Closed);
    assert!(!snapshot.has_swapchain, "closing releases the swapchain");
    assert_eq!(
        snapshot.estimated_target_bytes, 0,
        "a closed renderer owns no target memory"
    );
    assert!(
        window.close().is_ok(),
        "closing twice is idempotent, matching Swift's `try? close()` deferral"
    );
}
