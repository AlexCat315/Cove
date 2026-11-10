//! Metal-only draw-list probe. On Windows the Metal backend is not built, so
//! requesting it would fail before the probe can measure anything.
#![cfg(target_os = "macos")]

use cove_core::{Color, DrawList, Rect};
use cove_graphics::NativeDrawListRenderer;
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{
    CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass, RenderColorTarget,
    TextureDescriptor, TextureFormat, TextureResource, TextureSubresource, TextureUsage,
};
use std::sync::Arc;

#[test]
fn probe() {
    let device = Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("device"),
    );
    println!("device ok {}", device.device_name());
    let mut list = DrawList::default();
    let _ = list.add_rect(
        Rect::new(2.0, 3.0, 8.0, 6.0),
        Color::from_rgba8(255, 0, 0, 255),
    );
    let vp = NativeUIViewport::new([32, 24], [32.0, 24.0]);
    let output = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            32,
            24,
            TextureFormat::Bgra8Unorm,
            TextureUsage::COLOR_TARGET,
        ),
    )
    .expect("tex");
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("renderer");
    renderer
        .configure(TextureFormat::Bgra8Unorm, 1)
        .expect("configure");
    device.begin_frame().expect("begin");
    let mut cmds = CommandBuffer::new();
    let frame = renderer
        .record(
            &list,
            &mut cmds,
            RenderColorTarget::clearing(output.texture(), [0.0, 0.0, 0.0, 1.0]),
            vp,
            None,
        )
        .expect("record");
    println!("stats {:?}", frame.statistics);
    device
        .submit(&cmds, QueueClass::Graphics, &[], &[], None)
        .expect("submit");
    device.end_frame();
    device.wait_until_idle().expect("idle");
    let mut data = vec![0u8; 32 * 24 * 4];
    device
        .read_texture_data(
            output.texture(),
            32,
            24,
            32 * 4,
            TextureSubresource::default(),
            &mut data,
        )
        .expect("read");
    let idx = (5 * 32 + 5) * 4;
    println!("pixel at (5,5) = {:?}", &data[idx..idx + 4]);
}
