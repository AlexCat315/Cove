//! Translation of `Tests/CoveUIRuntimeTests/NativeRoundedFillTests.swift`.
//!
//! A rounded fill at fractional coverage has to be right at every density: the
//! corners must be smooth, no pixel may be brighter than the requested alpha
//! (a halo would mean double-blended edge samples), and the covered area must
//! match the analytic area of a 48x40 box with 8pt corners.
//!
//! Behaviour checklist:
//! - single-sample rounded fills have smooth corners, no halo and no alpha
//!   accumulation at each density

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::NativeDrawListRenderer;

const BACKGROUND: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// The half-transparent white the corner and area assertions measure.
const FILL_ALPHA: u8 = 128;

fn device() -> Option<Arc<Device>> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
    .map(Arc::new)
}

/// Render `list` at `side` pixels and return the BGRA8 bytes.
fn render(device: &Arc<Device>, list: &DrawList, side: usize, scale: f32) -> Vec<u8> {
    let viewport = NativeUIViewport::new([side, side], [64.0, 64.0]);
    let output = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            side,
            side,
            TextureFormat::Bgra8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ),
    )
    .expect("an offscreen target");

    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("a renderer");
    renderer.configure(TextureFormat::Bgra8Unorm, 1).expect("configuration");

    device.begin_frame().expect("begin frame");
    let mut commands = CommandBuffer::new();
    let mut frame = renderer
        .record(
            list,
            &mut commands,
            RenderColorTarget {
                texture: output.texture(),
                load_action: ColorLoadAction::Clear([0.0, 0.0, 0.0, 1.0]),
                store: true,
                resolve_texture: None,
            },
            viewport,
            None,
        )
        .expect("record");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("submit");
    frame.did_submit();
    device.end_frame();
    device.wait_until_idle().expect("idle");

    let mut pixels = vec![0u8; side * side * 4];
    device
        .read_texture_data(
            output.texture(),
            side,
            side,
            side * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("read back");
    let _ = scale;
    pixels
}

/// Byte 2 is the red channel of a BGRA8 target; the fill is white over black.
fn red(image: &[u8], side: usize, x: usize, y: usize) -> u8 {
    image[(y * side + x) * 4 + 2]
}

#[test]
fn single_sample_rounded_fills_have_smooth_corners_no_halo_and_no_alpha_accumulation() {
    let Some(device) = device() else {
        eprintln!("no Metal device available; skipping the rounded fill test");
        return;
    };

    for scale in [1.0_f32, 1.5, 2.0, 3.0] {
        let side = (64.0 * scale) as usize;
        let mut list = DrawList::default();
        list.pixel_scale = scale;
        let _ = list.add_rect(Rect::new(0.0, 0.0, 64.0, 64.0), BACKGROUND);
        let _ = list.add_rounded_rect(
            Rect::new(8.0, 8.0, 48.0, 40.0),
            8.0,
            Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: FILL_ALPHA as f32 / 255.0,
            },
        );

        let image = render(&device, &list, side, scale);
        let at = |x: f32, y: f32| red(&image, side, x as usize, y as usize);

        assert_eq!(
            at(32.0 * scale, 28.0 * scale),
            FILL_ALPHA,
            "scale {scale}: the interior is exactly the requested alpha",
        );
        assert_eq!(
            red(
                &image,
                side,
                (32.0 * scale) as usize,
                (8.0 * scale) as usize - 1
            ),
            0,
            "scale {scale}: just above the top edge stays background",
        );
        assert_eq!(
            at(32.0 * scale, 8.0 * scale),
            FILL_ALPHA,
            "scale {scale}: the first covered row is fully covered",
        );

        let reds: Vec<u8> = image.chunks_exact(4).map(|texel| texel[2]).collect();
        assert!(
            reds.iter().all(|value| *value <= FILL_ALPHA),
            "scale {scale}: no pixel may accumulate above the requested alpha",
        );
        assert!(
            reds.iter().any(|value| *value > 0 && *value < FILL_ALPHA),
            "scale {scale}: the edge is antialiased rather than hard",
        );

        // Coverage is the sum of fractional alphas; the analytic area of a
        // 48x40 box with 8pt corners loses one full corner square's worth.
        let covered: f32 = reds.iter().map(|value| *value as f32 / FILL_ALPHA as f32).sum();
        let expected = (48.0 * 40.0 - (4.0 - std::f32::consts::PI) * 8.0 * 8.0) * scale * scale;
        assert!(
            (covered - expected).abs() < 3.0 * scale * scale,
            "scale {scale}: covered area {covered} vs expected {expected}",
        );
    }
}
