//! The device's binding-set cache is bounded and evicts its least recently used
//! sets. The UI renderer must never draw with a set that was evicted between two
//! frames: that failed validation as "layout mismatch" and ended the app once a
//! glass surface (a new backdrop set per frame) had been open long enough.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Point, Rect};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{
    BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSetDescriptor,
    BindingSetEntry, BindingType, BufferDescriptor, BufferUsage, CommandBuffer, Device,
    DeviceConfig, GraphicsApi, NativeDrawListRenderer, QueueClass, RenderColorTarget,
    ShaderVisibility, TextureDescriptor, TextureFormat, TextureResource, TextureUploadRegion,
    TextureUsage,
};

fn device() -> Arc<Device> {
    Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a Metal device is available"),
    )
}

fn draw(device: &Arc<Device>, renderer: &mut NativeDrawListRenderer) {
    let target = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            32,
            24,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET,
        ),
    )
    .expect("target");
    let mut list = DrawList::default();
    let _ = list.add_image_quad(
        Rect::new(0.0, 0.0, 32.0, 24.0),
        9,
        Color::WHITE,
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
    );
    device.begin_frame().expect("frame opens");
    let mut commands = CommandBuffer::new();
    renderer
        .record(
            &list,
            &mut commands,
            RenderColorTarget::clearing(target.texture(), [0.0, 0.0, 0.0, 1.0]),
            NativeUIViewport::new([32, 24], [32.0, 24.0]),
            None,
        )
        .expect("the frame records");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the frame submits with live binding sets");
    device.end_frame();
    device.wait_until_idle().expect("the GPU drains");
}

#[test]
fn a_texture_drawn_again_after_cache_eviction_gets_a_live_set() {
    let device = device();
    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("renderer");
    renderer
        .configure(TextureFormat::Rgba8Unorm, 1)
        .expect("configure");
    renderer
        .register_texture(
            9,
            vec![200_u8; 16],
            [2, 2],
            TextureUploadRegion::new(2, 2),
            TextureFormat::Rgba8Unorm,
        )
        .expect("texture");
    draw(&device, &mut renderer);

    // Fill the cache well past its capacity with unrelated sets.
    let layout = device
        .make_binding_layout(BindingLayoutDescriptor::new(vec![BindingLayoutEntry::new(
            0,
            BindingType::UniformBuffer,
            ShaderVisibility::GRAPHICS,
        )]))
        .expect("layout");
    let count = 1100;
    let buffer = device
        .make_buffer(BufferDescriptor::new(count * 256, BufferUsage::UNIFORM))
        .expect("buffer");
    for index in 0..count {
        device
            .make_binding_set(
                layout,
                BindingSetDescriptor::new(vec![BindingSetEntry::new(
                    0,
                    BindingResource::UniformBuffer {
                        buffer,
                        offset: index * 256,
                        size: Some(16),
                    },
                )]),
            )
            .expect("flood set");
    }

    draw(&device, &mut renderer);
}
