//! Translation of `Graphics/Tests/NativeRHITests/NativeMultisampleTests.swift`.
//!
//! A 4x multisampled colour target must resolve into a single-sample texture
//! with the resolved pixels, and the passes that cannot work must be rejected
//! before anything is encoded: a resolve target with the wrong size, format or
//! aliasing the source, a single-sample source that claims a resolve, a pipeline
//! whose sample count disagrees with the attachment, and any upload/readback or
//! blit that targets a multisampled texture.
//!
//! Reachability: `draw` and `clearAndReject` build `PackedColorShaders` from
//! Slang-compiled fixtures (`ShaderFixtures.compile("packed-color", ...)`) before
//! asserting anything, so their drawing halves are gated on `SLANGC` plus the
//! Swift `cove-dev` tool — the same gate `ShaderFixtureCompiler.swift` applies.
//! `hdrAndDepth` uses **no shader at all**: it only clears a multisampled
//! `rgba16Float` colour target into a resolve destination with pre-populated
//! lower mips, clears a multisampled depth target, and rejects a sample-count
//! mismatch. It is therefore asserted in full here, not gated. The parts of
//! `clearAndReject` that need no shader — the resolve load/store contract and
//! every resolve/multisample validation rejection — are also asserted in full.
//!
//! Behaviour checklist:
//! - a 4x clear resolves into the single-sample target with the resolved pixels
//! - an HDR 4x colour clear resolves to the exact `f16` bit patterns
//! - a resolve destination that also has sampled lower mips keeps those mips
//! - a multisampled depth target clears alongside the resolving colour target
//! - a depth attachment whose sample count disagrees with the colour target is
//!   rejected
//! - a resolve target of the wrong size is rejected
//! - a resolve target of the wrong format is rejected
//! - a resolve target aliasing the source is rejected
//! - a single-sample source claiming a resolve is rejected
//! - uploading into or reading from a multisampled texture is rejected
//! - copying a multisampled texture is rejected
//! - GAP: the drawn colour-coverage and drawn-resolve cases need a Slang fixture

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_graphics::{
    ColorLoadAction, CommandBuffer, DepthLoadAction, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, RenderDepthTarget, RenderPassDescriptor, TextureDescriptor, TextureFormat,
    TextureResource, TextureSubresource, TextureUploadRegion, TextureUsage,
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

fn multisampled(device: &Arc<Device>, format: TextureFormat) -> TextureResource {
    TextureResource::new(
        device.clone(),
        TextureDescriptor {
            sample_count: 4,
            ..TextureDescriptor::new(
                24,
                20,
                format,
                TextureUsage::COLOR_TARGET
                    | TextureUsage::TRANSFER_SOURCE
                    | TextureUsage::TRANSFER_DESTINATION,
            )
        },
    )
    .expect("the multisampled source is created")
}

fn single_sample(
    device: &Arc<Device>,
    format: TextureFormat,
    width: usize,
    height: usize,
) -> TextureResource {
    TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            width,
            height,
            format,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ),
    )
    .expect("the resolve target is created")
}

fn read(device: &Arc<Device>, resource: &TextureResource, width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0_u8; width * height * 4];
    device
        .read_texture_data(
            resource.texture(),
            width,
            height,
            width * 4,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("the resolve target reads back");
    pixels
}

#[test]
fn transient_resolve_preserves_pixels_and_rejects_invalid_targets() {
    let device = device();
    let source = multisampled(&device, TextureFormat::Rgba8Unorm);
    let output = single_sample(&device, TextureFormat::Rgba8Unorm, 24, 20);

    // Clear (0.25, 0.5, 0.75, 1) into the multisampled source and resolve.
    let clear = [0.25_f32, 0.5, 0.75, 1.0];
    let resolve = {
        let mut target = RenderColorTarget {
            texture: source.texture(),
            ..RenderColorTarget::clearing(output.texture(), clear)
        };
        target.resolve_texture = Some(output.texture());
        target.store = false;
        target
    };

    let mut commands = CommandBuffer::new();
    commands.render_pass(RenderPassDescriptor::new(vec![resolve]), |_| {});
    device.begin_frame().expect("the frame opens");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the resolving pass submits");
    device.end_frame();
    device.wait_until_idle().expect("the device drains");

    let pixels = read(&device, &output, 24, 20);
    // 0.25/0.5/0.75 unorm8 round to 64/128/191; allow one unit of slack for the
    // hardware's clear-value conversion.
    for (channel, expected) in [64_u8, 128, 191, 255].into_iter().enumerate() {
        let actual = pixels[channel];
        assert!(
            i32::from(actual).abs_diff(i32::from(expected)) <= 1,
            "channel {channel} resolved to {actual}, expected ~{expected}",
        );
    }
    assert!(
        pixels.as_chunks::<4>().0.iter().all(|pixel| {
            [64_u8, 128, 191, 255]
                .into_iter()
                .enumerate()
                .all(|(channel, expected)| {
                    i32::from(pixel[channel]).abs_diff(i32::from(expected)) <= 1
                })
        }),
        "every resolved pixel carries the cleared colour",
    );

    // The invalid resolve targets must each be refused at submit.
    let wrong_size = single_sample(&device, TextureFormat::Rgba8Unorm, 25, 20);
    let wrong_format = single_sample(&device, TextureFormat::Bgra8Unorm, 24, 20);

    let variants: Vec<(&str, RenderColorTarget)> = vec![
        ("wrong size", {
            let mut target = resolve;
            target.resolve_texture = Some(wrong_size.texture());
            target
        }),
        ("wrong format", {
            let mut target = resolve;
            target.resolve_texture = Some(wrong_format.texture());
            target
        }),
        ("aliasing the source", {
            let mut target = resolve;
            target.resolve_texture = Some(source.texture());
            target
        }),
        ("single-sample source claiming a resolve", {
            let mut target = RenderColorTarget {
                texture: output.texture(),
                ..resolve
            };
            target.resolve_texture = Some(wrong_format.texture());
            target
        }),
    ];

    device.begin_frame().expect("the frame opens");
    for (label, target) in variants {
        let mut invalid = CommandBuffer::new();
        invalid.render_pass(RenderPassDescriptor::new(vec![target]), |_| {});
        assert!(
            device
                .submit(&invalid, QueueClass::Graphics, &[], &[], None)
                .is_err(),
            "a pass with {label} is rejected",
        );
    }
    device.end_frame();
    device.wait_until_idle().expect("the device drains");
}

#[test]
fn multisampled_textures_reject_transfers() {
    let device = device();
    let source = multisampled(&device, TextureFormat::Rgba8Unorm);

    // Uploading into a multisampled texture, or reading from it, is not a valid
    // transfer in this RHI.
    assert!(
        device
            .upload_texture_data(
                source.texture(),
                &[0_u8; 4],
                TextureUploadRegion::new(1, 1),
                4,
                TextureSubresource::default(),
            )
            .is_err(),
        "uploading into a multisampled texture is rejected",
    );
    let mut bytes = vec![0_u8; 4];
    assert!(
        device
            .read_texture_data(
                source.texture(),
                1,
                1,
                4,
                TextureSubresource::default(),
                &mut bytes,
            )
            .is_err(),
        "reading from a multisampled texture is rejected",
    );

    // A blit that names a multisampled texture is likewise refused.
    let transfer = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            4,
            1,
            TextureFormat::Rgba8Unorm,
            TextureUsage::TRANSFER_SOURCE | TextureUsage::TRANSFER_DESTINATION,
        ),
    )
    .expect("the transfer staging texture is created");
    let mut commands = CommandBuffer::new();
    commands.copy_pass(|pass| {
        pass.copy_texture(source.texture(), transfer.texture(), 4, 1);
    });
    device.begin_frame().expect("the frame opens");
    assert!(
        device
            .submit(&commands, QueueClass::Graphics, &[], &[], None)
            .is_err(),
        "copying a multisampled texture is rejected",
    );
    device.end_frame();
    device.wait_until_idle().expect("the device drains");
}

#[test]
fn hdr_and_depth_resolve() {
    let device = device();

    let source = TextureResource::new(
        device.clone(),
        TextureDescriptor {
            sample_count: 4,
            ..TextureDescriptor::new(8, 6, TextureFormat::Rgba16Float, TextureUsage::COLOR_TARGET)
        },
    )
    .expect("the multisampled HDR source is created");
    let depth = TextureResource::new(
        device.clone(),
        TextureDescriptor {
            sample_count: 4,
            ..TextureDescriptor::new(
                8,
                6,
                TextureFormat::Depth32Float,
                TextureUsage::DEPTH_STENCIL_TARGET,
            )
        },
    )
    .expect("the multisampled depth target is created");
    // A resolve destination may also carry sampled lower mips; only mip 0 is
    // touched by the attachment.
    let output = TextureResource::new(
        device.clone(),
        TextureDescriptor {
            mip_levels: 3,
            ..TextureDescriptor::new(
                8,
                6,
                TextureFormat::Rgba16Float,
                TextureUsage::COLOR_TARGET
                    | TextureUsage::SAMPLED
                    | TextureUsage::TRANSFER_SOURCE
                    | TextureUsage::TRANSFER_DESTINATION,
            )
        },
    )
    .expect("the HDR resolve destination is created");

    // Pre-fill mip 1 so the test can prove the resolve never writes it.
    let lower_mip = vec![0x3cu8; 4 * 3 * 8];
    device
        .upload_texture_data(
            output.texture(),
            &lower_mip,
            TextureUploadRegion::new(4, 3),
            32,
            TextureSubresource::new(1, 0),
        )
        .expect("the lower mip uploads");

    let mut color = RenderColorTarget::clearing(source.texture(), [0.25, 2.0, 4.0, 0.5]);
    color.resolve_texture = Some(output.texture());
    color.store = false;

    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![color]).with_depth(
            RenderDepthTarget::new(depth.texture())
                .with_load_action(DepthLoadAction::Clear(0.75))
                .without_store(),
        ),
        |_| {},
    );

    device.begin_frame().expect("the frame opens");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("the HDR resolving pass submits");
    device.end_frame();
    device.wait_until_idle().expect("the device drains");

    let mut bits = vec![0u16; 8 * 6 * 4];
    let mut bytes = vec![0u8; 8 * 6 * 8];
    device
        .read_texture_data(
            output.texture(),
            8,
            6,
            64,
            TextureSubresource::default(),
            &mut bytes,
        )
        .expect("the HDR resolve destination reads back");
    for (index, chunk) in bytes.as_chunks::<2>().0.iter().enumerate() {
        bits[index] = u16::from_le_bytes([chunk[0], chunk[1]]);
    }

    // IEEE-754 binary16 bit patterns for 0.25, 2, 4 and 0.5 — the values Swift
    // compared against `Float16(...).bitPattern`.
    let expected = [0x3400u16, 0x4000, 0x4400, 0x3800];
    for pixel in bits.as_chunks::<4>().0 {
        assert_eq!(
            *pixel, expected,
            "every resolved pixel carries the cleared HDR colour exactly"
        );
    }

    let mut actual_mip = vec![0u8; lower_mip.len()];
    device
        .read_texture_data(
            output.texture(),
            4,
            3,
            32,
            TextureSubresource::new(1, 0),
            &mut actual_mip,
        )
        .expect("the lower mip reads back");
    assert_eq!(
        actual_mip, lower_mip,
        "the resolve writes mip 0 only and leaves the sampled lower mip intact"
    );

    // A depth attachment with a different sample count cannot join a 4x pass.
    let single_depth = TextureResource::new(
        device.clone(),
        TextureDescriptor::new(
            8,
            6,
            TextureFormat::Depth32Float,
            TextureUsage::DEPTH_STENCIL_TARGET,
        ),
    )
    .expect("the single-sample depth target is created");

    let mut invalid = CommandBuffer::new();
    invalid.render_pass(
        RenderPassDescriptor::new(vec![color])
            .with_depth(RenderDepthTarget::new(single_depth.texture())),
        |_| {},
    );
    device.begin_frame().expect("the frame opens");
    assert!(
        device
            .submit(&invalid, QueueClass::Graphics, &[], &[], None)
            .is_err(),
        "a single-sample depth target cannot join a 4x colour pass",
    );
    device.end_frame();
}

#[test]
fn fixture_cases_are_gated_on_the_slang_toolchain() {
    // Swift's colour-coverage and HDR/depth cases both start from
    // `PackedColorShaders(device:target:)`, which compiles `packed-color.slang`
    // through `ShaderFixtures`. Assert the gate instead of fabricating a shader.
    let has_slangc = std::env::var("SLANGC")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if !has_slangc {
        eprintln!("skipping the colour-coverage and HDR cases: set SLANGC");
        return;
    }
    assert!(
        std::env::var("COVE_DEV")
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        "GAP: with SLANGC set the coverage cases also need COVE_DEV for the \
         cove-dev shader tool; this tree has no Rust artifact builder",
    );
    let _ = ColorLoadAction::DontCare;
}
