#![cfg(target_os = "windows")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect};
use cove_graphics::ui::NativeUIViewport;
use cove_graphics::{
    BufferDescriptor, BufferUsage, CommandBuffer, Device, DeviceConfig, GraphicsApi,
    NativeDrawListRenderer, QueueClass, RenderColorTarget, TextureBufferUpload, TextureDescriptor,
    TextureFormat, TextureResource, TextureUploadRegion, TextureUsage,
};

fn dx12_device() -> Option<Arc<Device>> {
    let config = DeviceConfig::default();
    assert_eq!(config.preferred_backends.first(), Some(&GraphicsApi::Dx12));
    let Ok(device) = Device::make(config) else {
        eprintln!("no graphics device available");
        return None;
    };
    if device.backend_api() != GraphicsApi::Dx12 {
        eprintln!("no DX12 device available");
        return None;
    }
    Some(Arc::new(device))
}

#[test]
fn dx12_resolves_and_blits_the_whole_frame_after_a_clipped_draw() {
    let Some(device) = dx12_device() else { return };
    let [width, height] = [32, 24];
    let format = TextureFormat::Bgra8UnormSrgb;
    let attachment = |usage| TextureDescriptor::new(width, height, format, usage);
    let framebuffer = TextureResource::new(
        device.clone(),
        attachment(TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED),
    )
    .expect("framebuffer");
    let presented = TextureResource::new(
        device.clone(),
        attachment(TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE),
    )
    .expect("presentation target");
    let copy = TextureResource::new(
        device.clone(),
        attachment(TextureUsage::TRANSFER_SOURCE | TextureUsage::TRANSFER_DESTINATION),
    )
    .expect("copy texture");

    let offset = 5;
    let pitch = width * 4 + 12;
    let buffer_size = offset + pitch * height;
    let readback = device
        .make_buffer(BufferDescriptor::new(
            buffer_size,
            BufferUsage::TRANSFER_DESTINATION,
        ))
        .expect("readback buffer");
    let mut list = DrawList::default();
    list.add_rect(
        Rect::new(0.0, 0.0, 16.0, 24.0),
        Color::from_rgba8(255, 0, 0, 255),
    )
    .expect("left rectangle");
    list.push_clip_rect(Rect::new(16.0, 0.0, 16.0, 24.0));
    list.add_rect(
        Rect::new(0.0, 0.0, 32.0, 24.0),
        Color::from_rgba8(0, 255, 0, 255),
    )
    .expect("clipped right rectangle");
    list.pop_clip_rect();

    let mut renderer = NativeDrawListRenderer::new(device.clone()).expect("renderer");
    renderer.configure(format, 4).expect("4× MSAA pipeline");
    let viewport = NativeUIViewport::new([width, height], [width as f32, height as f32]);
    device.begin_frame().expect("begin frame");
    let mut commands = CommandBuffer::new();
    let mut frame = renderer
        .record_presentation(
            &list,
            &mut commands,
            framebuffer.texture(),
            viewport,
            [0.0, 0.0, 0.0, 1.0],
            usize::MAX,
        )
        .expect("4× MSAA render pass");
    renderer
        .record_blit(
            framebuffer.texture(),
            &mut commands,
            RenderColorTarget::clearing(presented.texture(), [0.0, 0.0, 0.0, 1.0]),
            format,
            [width, height],
        )
        .expect("full-frame blit");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("render and blit");
    frame.did_submit();

    let mut commands = CommandBuffer::new();
    commands.copy_pass(|pass| {
        pass.copy_texture(
            presented.texture(),
            copy.texture(),
            width as i64,
            height as i64,
        );
        pass.copy_texture_to_buffer(
            copy.texture(),
            width as i64,
            height as i64,
            readback,
            offset,
            pitch,
        );
    });
    device
        .submit(&commands, QueueClass::Transfer, &[], &[], None)
        .expect("copy on the transfer queue");
    device.end_frame();

    let mut pixels = vec![0; buffer_size];
    device
        .read_buffer_data(readback, 0, &mut pixels)
        .expect("read back");
    for y in [3, height - 3] {
        let pixel = |x: usize| {
            let index = offset + y * pitch + x * 4;
            &pixels[index..index + 4]
        };
        assert_eq!(pixel(4), [0, 0, 255, 255], "unclipped left side at row {y}");
        assert_eq!(pixel(24), [0, 255, 0, 255], "clipped right side at row {y}");
    }
    device.destroy_buffer(readback);
}

#[test]
fn dx12_device_local_buffer_can_upload_a_texture_with_padded_rows() {
    let Some(device) = dx12_device() else { return };
    let buffer = device
        .make_buffer(BufferDescriptor::new(
            32,
            BufferUsage::TRANSFER_SOURCE | BufferUsage::TRANSFER_DESTINATION,
        ))
        .expect("device-local buffer");
    let texture = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            2,
            2,
            TextureFormat::Rgba8Unorm,
            TextureUsage::TRANSFER_SOURCE | TextureUsage::TRANSFER_DESTINATION,
        ),
    )
    .expect("texture");
    let pixels = [
        1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16,
    ];
    device
        .upload_buffer_data(buffer, 4, &pixels)
        .expect("CPU to GPU buffer");
    let mut round_trip = [0; 20];
    device
        .read_buffer_data(buffer, 4, &mut round_trip)
        .expect("GPU to CPU buffer");
    assert_eq!(round_trip, pixels);

    device.begin_frame().expect("begin frame");
    let mut commands = CommandBuffer::new();
    commands.copy_pass(|pass| {
        let mut upload = TextureBufferUpload::new(
            buffer,
            12,
            texture.texture(),
            TextureUploadRegion::new(2, 2),
        );
        upload.offset = 4;
        pass.upload_buffer_to_texture(upload);
    });
    device
        .submit(&commands, QueueClass::Transfer, &[], &[], None)
        .expect("GPU buffer to texture");
    device.end_frame();
    let mut result = [0; 16];
    device
        .read_texture_data(texture.texture(), 2, 2, 8, Default::default(), &mut result)
        .expect("texture readback");
    assert_eq!(&result[..8], &pixels[..8]);
    assert_eq!(&result[8..], &pixels[12..]);
    device.destroy_buffer(buffer);
}
