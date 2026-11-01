//! Translation of `Tests/CoveUIRuntimeTests/ShadowQuantizationTests.swift`.
//!
//! The Swift suite is `#if os(macOS)` because two of its three cases rasterise
//! through `NativeUIDrawTestContext`. This port covers the CPU-side contract that
//! does not need a device: a shadow whose alpha packs to zero must emit nothing.
//!
//! Not ported: `compositingIsPreserved` and `occludedShadowKeepsRoundedCorners`.
//! Both compare `NativeUIViewport` rasterisations across `samples` 1 and 4, which
//! needs a live Metal device and belongs with the Graphics/Hosts owner — the same
//! decision as `NativeWindowLifecycleTests`.
//!
//! Behaviour checklist:
//! - positive shadow alpha that packs to zero emits no geometry

use cove_core::scene::DrawList;
use cove_core::{Color, LayeredShadow, Rect, SurfaceShadow};

#[test]
fn transparent_steps_are_omitted() {
    let rect = Rect::new(16.0, 16.0, 96.0, 64.0);
    for opacity in [0.0_f32, 0.001, 1.0] {
        let mut list = DrawList::default();
        list.pixel_scale = 2.0;
        list.add_shadow(
            LayeredShadow::single(SurfaceShadow {
                // 0.001 alpha packs to byte 0, so every step is invisible.
                color: Some(Color {
                    r: 0.2,
                    g: 0.1,
                    b: 0.05,
                    a: 0.001,
                }),
                offset_x: 0.0,
                offset_y: 2.0,
                blur: 8.0,
            }),
            rect,
            16.0,
            opacity,
            None,
            None,
        );
        assert!(
            list.vertices().is_empty() && list.indices().is_empty() && list.batches().is_empty(),
            "opacity {opacity}: a shadow that packs to zero emits nothing",
        );
    }
}
