//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalClearReadbackTests.swift`.
//!
//! Behaviour checklist:
//! - a render pass that clears an offscreen colour target records, submits
//!   asynchronously, and reads back the cleared colour on the real GPU
//!
//! This exercises the render encoder, load action, store action, asynchronous
//! submit plus completion, and the readback path end to end.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, RenderPassDescriptor, TextureDescriptor, TextureFormat, TextureSubresource,
    TextureUsage,
};

#[test]
fn clear_then_readback_matches() {
    let device = match Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    }) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("no Metal device available ({error}); skipping the clear/readback test");
            return;
        }
    };

    let (width, height) = (4usize, 4usize);
    let target = device
        .make_texture(TextureDescriptor::new(
            width,
            height,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
        ))
        .expect("offscreen colour target");

    let clear = [0.2f32, 0.6, 0.9, 1.0];

    device.begin_frame().expect("begin frame");
    let mut command = CommandBuffer::new();
    command.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget {
            resolve_texture: None,
            texture: target,
            load_action: ColorLoadAction::Clear(clear),
            store: true,
        }]),
        |_encoder| {},
    );
    device
        .submit(&command, QueueClass::Graphics, &[], &[], None)
        .expect("submit the clear pass");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let bytes_per_row = width * 4;
    let mut pixels = vec![0u8; bytes_per_row * height];
    device
        .read_texture_data(
            target,
            width,
            height,
            bytes_per_row,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("read the cleared target back");

    let red = pixels[0] as f32 / 255.0;
    let green = pixels[1] as f32 / 255.0;
    let blue = pixels[2] as f32 / 255.0;
    assert!(
        (red - clear[0]).abs() < 0.12,
        "red channel was {red}, expected about {}",
        clear[0]
    );
    assert!(
        (green - clear[1]).abs() < 0.12,
        "green channel was {green}, expected about {}",
        clear[1]
    );
    assert!(
        (blue - clear[2]).abs() < 0.12,
        "blue channel was {blue}, expected about {}",
        clear[2]
    );
}
