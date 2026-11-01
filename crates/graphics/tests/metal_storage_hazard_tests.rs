//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalStorageHazardTests.swift`.
//!
//! Behaviour checklist:
//! - two copy passes write the same texture while it stays in the destination
//!   state, so the access layer sees a write-after-write hazard with no state
//!   change; the second write must still win on the real GPU
//!
//! This is the orthogonal access-dependency case: Metal encodes two blit
//! encoders and its own hazard tracking orders them, so the backend must not
//! rely on a state transition to serialize the writes.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    BufferDescriptor, BufferUsage, CommandBuffer, Device, DeviceConfig, GraphicsApi,
    TextureBufferUpload, TextureDescriptor, TextureFormat, TextureSubresource, TextureUploadRegion,
    TextureUsage,
};

#[test]
fn same_state_write_after_write_orders_and_second_wins() {
    let device = match Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    }) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("no Metal device available ({error}); skipping the storage hazard test");
            return;
        }
    };

    let (width, height) = (4usize, 4usize);
    let bytes_per_row = width * 4;
    let byte_count = bytes_per_row * height;

    let target = device
        .make_texture(TextureDescriptor::new(
            width,
            height,
            TextureFormat::Rgba8Unorm,
            TextureUsage::TRANSFER_DESTINATION | TextureUsage::SAMPLED,
        ))
        .expect("destination texture");

    let red_buffer = device
        .make_buffer(BufferDescriptor::labelled(
            byte_count,
            BufferUsage::TRANSFER_SOURCE,
            "red",
        ))
        .expect("red source buffer");
    let blue_buffer = device
        .make_buffer(BufferDescriptor::labelled(
            byte_count,
            BufferUsage::TRANSFER_SOURCE,
            "blue",
        ))
        .expect("blue source buffer");

    let mut red_bytes = vec![0u8; byte_count];
    let mut blue_bytes = vec![0u8; byte_count];
    for pixel in 0..(width * height) {
        let base = pixel * 4;
        red_bytes[base] = 255;
        red_bytes[base + 3] = 255;
        blue_bytes[base + 2] = 255;
        blue_bytes[base + 3] = 255;
    }
    device
        .upload_buffer_data(red_buffer, 0, &red_bytes)
        .expect("upload red");
    device
        .upload_buffer_data(blue_buffer, 0, &blue_bytes)
        .expect("upload blue");

    device.begin_frame().expect("begin frame");
    let mut commands = CommandBuffer::new();
    commands.copy_pass(|encoder| {
        encoder.upload_buffer_to_texture(TextureBufferUpload::new(
            red_buffer,
            bytes_per_row,
            target,
            TextureUploadRegion::new(width as i64, height as i64),
        ));
    });
    commands.copy_pass(|encoder| {
        encoder.upload_buffer_to_texture(TextureBufferUpload::new(
            blue_buffer,
            bytes_per_row,
            target,
            TextureUploadRegion::new(width as i64, height as i64),
        ));
    });
    device
        .submit(
            &commands,
            cove_graphics::rhi::QueueClass::Graphics,
            &[],
            &[],
            None,
        )
        .expect("submit both copy passes");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let mut pixels = vec![0u8; byte_count];
    device
        .read_texture_data(
            target,
            width,
            height,
            bytes_per_row,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("read the hazard target back");

    assert_eq!(
        pixels[0], 0,
        "the second write must win, so the red channel is the blue buffer's zero"
    );
    assert_eq!(
        pixels[2], 255,
        "the second write must win, so the blue channel is set"
    );
    assert_eq!(pixels[3], 255, "alpha comes from the winning write");
}
