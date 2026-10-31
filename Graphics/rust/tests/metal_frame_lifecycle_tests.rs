//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalFrameLifecycleTests.swift`.
//!
//! Behaviour checklist:
//! - attaching a CPU profile does not change the rendered pixels, and every
//!   phase accumulates across submissions
//! - several frames in flight each submit a clear, a texture is retired while
//!   work may still be queued, and draining to idle neither crashes nor reads
//!   freed memory
//!
//! The frontend defers `destroy*` to GPU completion, so the deferred-destroy
//! case below is a use-after-free probe: it passes only if retirement really
//! waits for the retiree's frame slot.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, RenderPassDescriptor, SubmissionCpuProfile, TextureDescriptor,
    TextureFormat, TextureSubresource, TextureUsage,
};

fn metal_device(frames_in_flight: usize) -> Option<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: true,
        frames_in_flight,
        ..DeviceConfig::default()
    })
    .ok()
}

#[test]
fn cpu_profile_preserves_rendering_and_accumulates_submissions() {
    let Some(device) = metal_device(2) else {
        eprintln!("no Metal device available; skipping the CPU profile test");
        return;
    };

    let target = device
        .make_texture(TextureDescriptor::new(
            4,
            4,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("offscreen colour target");

    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget {
            resolve_texture: None,
            texture: target,
            load_action: ColorLoadAction::Clear([1.0, 0.0, 0.0, 1.0]),
            store: true,
        }]),
        |_encoder| {},
    );

    let render = |profile: Option<&mut SubmissionCpuProfile>| -> Vec<u8> {
        device.begin_frame().expect("begin frame");
        if let Err(error) = device.submit(&commands, QueueClass::Graphics, &[], &[], profile) {
            device.end_frame();
            panic!("submit the clear pass: {error}");
        }
        device.end_frame();
        device.wait_until_idle().expect("wait for idle");
        let mut bytes = vec![0u8; 64];
        device
            .read_texture_data(
                target,
                4,
                4,
                16,
                TextureSubresource::default(),
                &mut bytes,
            )
            .expect("read the cleared target back");
        bytes
    };

    let reference = render(None);
    assert_eq!(
        &reference[..4],
        &[255, 0, 0, 255],
        "the clear pass writes opaque red"
    );

    let mut profile = SubmissionCpuProfile::new(false);
    assert_eq!(
        render(Some(&mut profile)),
        reference,
        "profiling must not change the rendered pixels"
    );

    let first = [
        profile.validation_nanoseconds,
        profile.planning_nanoseconds,
        profile.encoding_nanoseconds,
        profile.queue_submit_nanoseconds,
    ];
    assert!(
        first.iter().all(|phase| *phase > 0),
        "every profiled phase records a duration, got {first:?}"
    );

    assert_eq!(
        render(Some(&mut profile)),
        reference,
        "a repeated profiled submission renders identically"
    );

    let accumulated = [
        profile.validation_nanoseconds,
        profile.planning_nanoseconds,
        profile.encoding_nanoseconds,
        profile.queue_submit_nanoseconds,
    ];
    for (index, (total, previous)) in accumulated.iter().zip(first.iter()).enumerate() {
        assert!(
            total > previous,
            "phase {index} accumulates across submissions: {total} must exceed {previous}"
        );
    }
}

#[test]
fn frame_lifecycle_and_deferred_destroy() {
    let Some(device) = metal_device(2) else {
        eprintln!("no Metal device available; skipping the frame lifecycle test");
        return;
    };

    let target = device
        .make_texture(TextureDescriptor::new(
            4,
            4,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
        ))
        .expect("frame target");

    for frame in 0..3 {
        device.begin_frame().expect("begin frame");
        let mut commands = CommandBuffer::new();
        commands.render_pass(
            RenderPassDescriptor::new(vec![RenderColorTarget {
                resolve_texture: None,
                texture: target,
                load_action: ColorLoadAction::Clear([0.0, 0.0, 0.0, 1.0]),
                store: true,
            }]),
            |_encoder| {},
        );
        device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .unwrap_or_else(|error| panic!("submit frame {frame}: {error}"));
        device.end_frame();
    }

    // Retired while earlier frames may still be in flight: the deferred destroy
    // must not touch the native texture before its frame retired.
    device.destroy_texture(target);

    device.wait_until_idle().expect("wait for idle");
}
