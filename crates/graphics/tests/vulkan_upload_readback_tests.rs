//! Translation of
//! `Graphics/Tests/NativeRHITests/Vulkan/VulkanUploadReadbackTests.swift`.
//!
//! Vulkan's immediate upload path — staging buffer, layout transition into a
//! device-local texture, then a copy back out — has to round-trip known pixels
//! without a swapchain. The suite also checks that recorded buffer copies survive
//! the planner's injected barrier between passes.
//!
//! Reachability: the Swift file is wrapped in
//! `#if (os(Windows) || os(Linux)) && canImport(CVulkanHeaders)` and every case
//! begins `guard VulkanBackend.isAvailable else { throw XCTSkip(...) }`. The
//! whole target is therefore gated to Windows/Linux, matching
//! `vulkan_slang_tests.rs`. On this macOS host it is built and linked but never
//! run; on a Vulkan host each case still skips when no loader/ICD is present.
//!
//! Behaviour checklist:
//! - a host without a Vulkan loader/ICD is refused rather than panicking
//! - recorded copies preserve data across a planned barrier
//! - an upload then readback round-trips known pixels

#![cfg(any(target_os = "linux", target_os = "windows"))]

use cove_graphics::{Device, DeviceConfig, GraphicsApi, is_compiled_in};

fn vulkan_available() -> bool {
    if !is_compiled_in(GraphicsApi::Vulkan) {
        return false;
    }
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    })
    .is_ok()
}

#[test]
fn absent_backend_is_refused_without_panicking() {
    if vulkan_available() {
        eprintln!("a Vulkan driver is present; the refusal case does not apply");
        return;
    }
    // Querying must be safe on a host that never built the backend, and asking
    // for it must produce an error rather than an abort.
    let result = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    });
    assert!(
        result.is_err(),
        "requesting an uncompiled Vulkan backend reports an error",
    );
}

#[test]
fn buffer_copies_preserve_data_across_a_planned_barrier() {
    if !vulkan_available() {
        eprintln!("skipping: no Vulkan loader/ICD on this host");
        return;
    }
    use cove_graphics::{Buffer, BufferDescriptor, BufferUsage, CommandBuffer, QueueClass};

    let device = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    })
    .expect("the Vulkan device builds");
    let device = std::sync::Arc::new(device);

    let usage = BufferUsage::TRANSFER_SOURCE | BufferUsage::TRANSFER_DESTINATION;
    let handles: Vec<Buffer> = (0..3)
        .map(|_| {
            device
                .make_buffer(BufferDescriptor::new(64, usage))
                .expect("the transfer buffer is created")
        })
        .collect();

    let data: Vec<u8> = (0..64_u8).collect();
    device
        .upload_buffer_data(handles[0], 0, &data)
        .expect("the source buffer uploads");

    let mut commands = CommandBuffer::new();
    commands.copy_pass(|pass| {
        pass.copy_buffer(handles[0], 0, handles[1], 0, 64);
    });
    commands.copy_pass(|pass| {
        pass.copy_buffer(handles[1], 0, handles[2], 0, 64);
    });

    device.begin_frame().expect("the frame opens");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("both copy passes submit");
    device.end_frame();
    device.wait_until_idle().expect("the device drains");

    let mut readback = vec![0_u8; 64];
    device
        .read_buffer_data(handles[2], 0, &mut readback)
        .expect("the destination buffer reads back");
    assert_eq!(
        readback, data,
        "the bytes survive two recorded copies across the injected barrier",
    );
    for handle in handles {
        device.destroy_buffer(handle);
    }
}

#[test]
fn transient_texture_uploads_preserve_pixels_across_frame_slots() {
    use cove_graphics::{
        CommandBuffer, QueueClass, TextureBufferUpload, TextureDescriptor, TextureFormat,
        TextureSubresource, TextureUploadRegion, TextureUsage,
    };

    let Ok(device) = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        frames_in_flight: 2,
        ..DeviceConfig::default()
    }) else {
        eprintln!("skipping: no Vulkan loader/ICD on this host");
        return;
    };
    let texture = device
        .make_texture(TextureDescriptor::new(
            2,
            1,
            TextureFormat::Rgba8Unorm,
            TextureUsage::TRANSFER_DESTINATION | TextureUsage::TRANSFER_SOURCE,
        ))
        .unwrap();
    let pixels = [[23, 53, 71, 255], [113, 127, 149, 255]];

    for (x, pixel) in pixels.iter().enumerate() {
        device.begin_frame().unwrap();
        let location = device.upload_transient(pixel, 4).unwrap();
        let mut upload = TextureBufferUpload::new(
            location.buffer,
            pixel.len(),
            texture,
            TextureUploadRegion::with_origin(1, 1, x as i64, 0),
        );
        upload.offset = location.offset;
        let mut commands = CommandBuffer::new();
        commands.copy_pass(|pass| pass.upload_buffer_to_texture(upload));
        device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .unwrap();
        device.end_frame();
    }
    device.wait_until_idle().unwrap();

    let mut readback = [0; 8];
    device
        .read_texture_data(
            texture,
            2,
            1,
            8,
            TextureSubresource::default(),
            &mut readback,
        )
        .unwrap();
    assert_eq!(&readback, pixels.as_flattened());
    device.destroy_texture(texture);
}

#[test]
fn multisample_clear_resolves_into_the_single_sample_texture() {
    use cove_graphics::{
        CommandBuffer, QueueClass, RenderColorTarget, RenderPassDescriptor, TextureDescriptor,
        TextureFormat, TextureSubresource, TextureUsage,
    };

    let Ok(device) = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    }) else {
        eprintln!("skipping: no Vulkan loader/ICD on this host");
        return;
    };
    let resolved_descriptor = TextureDescriptor::new(
        4,
        4,
        TextureFormat::Rgba8Unorm,
        TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
    );
    let mut multisample_descriptor = resolved_descriptor;
    multisample_descriptor.sample_count = 4;
    multisample_descriptor.usage = TextureUsage::COLOR_TARGET;
    let multisample = device.make_texture(multisample_descriptor).unwrap();
    let resolved = device.make_texture(resolved_descriptor).unwrap();
    let target = RenderColorTarget::clearing(multisample, [0.0, 1.0, 0.0, 1.0])
        .with_resolve(resolved)
        .without_store();
    let mut commands = CommandBuffer::new();
    commands.render_pass(RenderPassDescriptor::new(vec![target]), |_| {});

    device.begin_frame().unwrap();
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .unwrap();
    device.end_frame();
    device.wait_until_idle().unwrap();

    let mut readback = [0; 64];
    device
        .read_texture_data(
            resolved,
            4,
            4,
            16,
            TextureSubresource::default(),
            &mut readback,
        )
        .unwrap();
    assert_eq!(readback, [0, 255, 0, 255].repeat(16).as_slice());
}

#[test]
fn upload_then_readback_round_trips_pixels() {
    if !vulkan_available() {
        eprintln!("skipping: no native Vulkan driver on this host");
        return;
    }
    use cove_graphics::{
        TextureDescriptor, TextureFormat, TextureSubresource, TextureUploadRegion, TextureUsage,
    };

    let device = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    })
    .expect("the Vulkan device builds");

    let (width, height) = (4_usize, 4_usize);
    let bytes_per_row = width * 4;
    let texture = device
        .make_texture(TextureDescriptor::new(
            width,
            height,
            TextureFormat::Rgba8Unorm,
            TextureUsage::TRANSFER_DESTINATION
                | TextureUsage::TRANSFER_SOURCE
                | TextureUsage::SAMPLED,
        ))
        .expect("the texture is created");

    let mut uploaded = vec![0_u8; bytes_per_row * height];
    for pixel in uploaded.as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&[255, 0, 0, 255]);
    }
    device
        .upload_texture_data(
            texture,
            &uploaded,
            TextureUploadRegion::new(width as i64, height as i64),
            bytes_per_row,
            TextureSubresource::default(),
        )
        .expect("the upload succeeds");

    let mut readback = vec![0_u8; bytes_per_row * height];
    device
        .read_texture_data(
            texture,
            width,
            height,
            bytes_per_row,
            TextureSubresource::default(),
            &mut readback,
        )
        .expect("the readback succeeds");

    assert_eq!(readback[0], 255, "R survives the round trip");
    assert_eq!(readback[3], 255, "A survives the round trip");
    device.destroy_texture(texture);
}
