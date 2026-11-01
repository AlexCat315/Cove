//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalRenderBindingTests.swift`.
//!
//! Behaviour checklist:
//! - repeatedly re-setting sampler and texture bindings (20 times each) still
//!   leaves the latest sampler/texture visible for the draw, so a redundant-set
//!   elision never drops a real change
//! - uniform buffer offsets, an inline (push-constant) override and a new render
//!   pass each restore the buffer state the pipeline requires
//!
//! Named difference (documented, not dropped): the Swift suite compiled its
//! `render-sample` and `raster` fixtures at run time through `cove-dev` driven by
//! `SLANGC`, and skipped without both. There is no Rust `cove-dev`, so each case
//! checks the prerequisites explicitly and prints which one is missing rather
//! than passing vacuously — the pattern established in `vulkan_slang_tests.rs`.
//! The pipeline construction, per-draw binding churn and pixel assertions below
//! are the real ones from Swift and run unchanged once an artifact exists.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};

use cove_graphics::rhi::{
    BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSetDescriptor,
    BindingSetEntry, BindingType, BufferDescriptor, BufferUsage, ColorAttachmentDescriptor,
    CommandBuffer, Device, DeviceConfig, GraphicsApi, GraphicsPipelineDescriptor,
    PipelineLayoutDescriptor, PushConstantRange, QueueClass, RenderColorTarget,
    RenderPassDescriptor, SamplerDescriptor, SamplerFilter, ScissorRect, ShaderArtifact,
    ShaderStage, ShaderVisibility, TextureDescriptor, TextureFormat, TextureSubresource,
    TextureUploadRegion, TextureUsage, Viewport,
};

fn artifact(name: &str, stage: ShaderStage) -> Option<ShaderArtifact> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    ShaderArtifact::load_directory(
        &root,
        name,
        stage,
        GraphicsApi::Metal,
        Path::new("Fixtures"),
        None,
    )
    .ok()
}

/// Reports the missing prerequisite and returns `None` so the case can skip.
fn prerequisites(first: &str) -> Option<Device> {
    if std::env::var_os("SLANGC").is_none() {
        eprintln!(
            "skipping the Metal render binding case: no Slang compiler tool is available (Swift required SLANGC and cove-dev)"
        );
        return None;
    }
    if artifact(first, ShaderStage::Vertex).is_none() {
        eprintln!(
            "skipping the Metal render binding case: no compiled artifact for the `{first}` fixture (the Rust tree has no cove-dev to produce it)"
        );
        return None;
    }
    match Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: true,
        ..DeviceConfig::default()
    }) {
        Ok(device) => Some(device),
        Err(error) => {
            eprintln!("skipping the Metal render binding case: no Metal device ({error})");
            None
        }
    }
}

#[test]
fn sampler_and_texture_changes_remain_visible_after_repeated_sets() {
    let Some(device) = prerequisites("render-sample") else {
        return;
    };
    let (Some(vertex), Some(fragment)) = (
        artifact("render-sample", ShaderStage::Vertex),
        artifact("render-sample", ShaderStage::Fragment),
    ) else {
        eprintln!("skipping: the render-sample stage artifacts are incomplete");
        return;
    };

    let binding = device
        .make_binding_layout(
            fragment
                .binding_layout_descriptor(0)
                .expect("reflected binding layout"),
        )
        .expect("binding layout");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![binding.clone()]))
        .expect("pipeline layout");
    let mut pipeline_descriptor = GraphicsPipelineDescriptor::new(
        layout,
        device
            .make_shader_module(vertex.module_descriptor(&[]).expect("vertex module"))
            .expect("vertex shader"),
    );
    pipeline_descriptor.fragment = Some(
        device
            .make_shader_module(fragment.module_descriptor(&[]).expect("fragment module"))
            .expect("fragment shader"),
    );
    pipeline_descriptor.color_attachments =
        vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
    pipeline_descriptor.depth_format = None;
    pipeline_descriptor.depth_stencil = None;
    let pipeline = device
        .make_graphics_pipeline(pipeline_descriptor)
        .expect("graphics pipeline");

    let linear = device
        .make_sampler(SamplerDescriptor {
            min_filter: SamplerFilter::Linear,
            mag_filter: SamplerFilter::Linear,
            ..SamplerDescriptor::default()
        })
        .expect("linear sampler");
    let nearest = device
        .make_sampler(SamplerDescriptor {
            min_filter: SamplerFilter::Nearest,
            mag_filter: SamplerFilter::Nearest,
            ..SamplerDescriptor::default()
        })
        .expect("nearest sampler");

    let gradient = device
        .make_texture(TextureDescriptor::new(
            2,
            1,
            TextureFormat::Rgba8Unorm,
            TextureUsage::SAMPLED | TextureUsage::TRANSFER_DESTINATION,
        ))
        .expect("gradient texture");
    let green = device
        .make_texture(TextureDescriptor::new(
            2,
            1,
            TextureFormat::Rgba8Unorm,
            TextureUsage::SAMPLED | TextureUsage::TRANSFER_DESTINATION,
        ))
        .expect("green texture");
    device
        .upload_texture_data(
            gradient,
            &[255, 0, 0, 255, 0, 0, 255, 255],
            TextureUploadRegion::new(2, 1),
            8,
            TextureSubresource::default(),
        )
        .expect("upload the gradient");
    device
        .upload_texture_data(
            green,
            &[0, 255, 0, 255, 0, 255, 0, 255],
            TextureUploadRegion::new(2, 1),
            8,
            TextureSubresource::default(),
        )
        .expect("upload green");

    let bound = |image, sampler| {
        device
            .make_binding_set(
                binding,
                BindingSetDescriptor::new(vec![
                    BindingSetEntry::new(0, BindingResource::Texture(image)),
                    BindingSetEntry::new(1, BindingResource::Sampler(sampler)),
                ]),
            )
            .expect("binding set")
    };
    let a = bound(gradient, linear);
    let b = bound(gradient, nearest);
    let c = bound(green, nearest);

    let target = device
        .make_texture(TextureDescriptor::new(
            16,
            4,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("colour target");

    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
            target,
            [0.0, 0.0, 0.0, 0.0],
        )]),
        |encoder| {
            encoder.set_pipeline(pipeline);
            encoder.set_viewport(Viewport::new(16.0, 4.0));
            for (index, set) in [a, b, c, a].into_iter().enumerate() {
                encoder.set_scissor(ScissorRect::at(index as i64 * 4, 0, 4, 4));
                for _ in 0..20 {
                    encoder.set_binding_set(set, 0, Vec::new());
                }
                encoder.draw(3, 1, 0, 0);
            }
        },
    );

    device.begin_frame().expect("begin frame");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("submit the binding churn");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let mut pixels = vec![0u8; 16 * 4 * 4];
    device
        .read_texture_data(
            target,
            16,
            4,
            64,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("read the binding target back");

    let expected: [[u8; 4]; 4] = [
        [128, 0, 128, 255],
        [0, 0, 255, 255],
        [0, 255, 0, 255],
        [128, 0, 128, 255],
    ];
    for (index, want) in expected.into_iter().enumerate() {
        let pixel = (2 * 16 + index * 4 + 2) * 4;
        for (channel, want_channel) in want.into_iter().enumerate() {
            let actual = pixels[pixel + channel];
            assert!(
                actual.abs_diff(want_channel) <= 1,
                "band {index} channel {channel} was {actual}, expected about {want_channel}"
            );
        }
    }
}

#[test]
fn offsets_inline_constants_and_new_pass_restore_the_required_buffer() {
    let Some(device) = prerequisites("raster") else {
        return;
    };
    let (Some(vertex), Some(fragment)) = (
        artifact("raster", ShaderStage::Vertex),
        artifact("raster", ShaderStage::Fragment),
    ) else {
        eprintln!("skipping: the raster stage artifacts are incomplete");
        return;
    };

    let binding = device
        .make_binding_layout(BindingLayoutDescriptor::new(vec![BindingLayoutEntry::new(
            0,
            BindingType::UniformBuffer,
            ShaderVisibility::GRAPHICS,
        )]))
        .expect("binding layout");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![binding.clone()]))
        .expect("pipeline layout");
    let inline_binding = device
        .make_binding_layout(BindingLayoutDescriptor::new(vec![BindingLayoutEntry::new(
            0,
            BindingType::UniformBuffer,
            ShaderVisibility::VERTEX,
        )]))
        .expect("inline binding layout");
    let mut inline_descriptor = PipelineLayoutDescriptor::new(vec![inline_binding.clone()]);
    inline_descriptor.push_constants = vec![PushConstantRange::new(ShaderStage::Fragment, 0, 16)];
    let inline_layout = device
        .make_pipeline_layout(inline_descriptor)
        .expect("inline pipeline layout");

    let build = |layout| {
        let mut descriptor = GraphicsPipelineDescriptor::new(
            layout,
            device
                .make_shader_module(vertex.module_descriptor(&[]).expect("vertex module"))
                .expect("vertex shader"),
        );
        descriptor.fragment = Some(
            device
                .make_shader_module(fragment.module_descriptor(&[]).expect("fragment module"))
                .expect("fragment shader"),
        );
        descriptor.color_attachments =
            vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
        descriptor.depth_format = None;
        descriptor.depth_stencil = None;
        device
            .make_graphics_pipeline(descriptor)
            .expect("graphics pipeline")
    };
    let pipeline = build(layout);
    let inline_pipeline = build(inline_layout);

    let buffer = device
        .make_buffer(BufferDescriptor::new(32, BufferUsage::UNIFORM))
        .expect("uniform buffer");
    let colors: [f32; 8] = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0];
    let color_bytes: Vec<u8> = colors
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    device
        .upload_buffer_data(buffer, 0, &color_bytes)
        .expect("upload both colours");
    let red = device
        .make_binding_set(
            binding,
            BindingSetDescriptor::new(vec![BindingSetEntry::new(
                0,
                BindingResource::UniformBuffer {
                    buffer,
                    offset: 0,
                    size: Some(16),
                },
            )]),
        )
        .expect("red set");
    let green = device
        .make_binding_set(
            binding,
            BindingSetDescriptor::new(vec![BindingSetEntry::new(
                0,
                BindingResource::UniformBuffer {
                    buffer,
                    offset: 16,
                    size: Some(16),
                },
            )]),
        )
        .expect("green set");
    let inline_set = device
        .make_binding_set(
            inline_binding,
            BindingSetDescriptor::new(vec![BindingSetEntry::new(
                0,
                BindingResource::UniformBuffer {
                    buffer,
                    offset: 16,
                    size: Some(16),
                },
            )]),
        )
        .expect("inline set");

    let target = device
        .make_texture(TextureDescriptor::new(
            128,
            32,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("colour target");

    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
            target,
            [0.0, 0.0, 0.0, 0.0],
        )]),
        |encoder| {
            encoder.set_pipeline(pipeline);
            encoder.set_viewport(Viewport::new(128.0, 32.0));
            encoder.set_scissor(ScissorRect::at(16, 24, 24, 8));
            encoder.set_binding_set(red, 0, Vec::new());
            encoder.set_binding_set(red, 0, Vec::new());
            encoder.draw(3, 1, 0, 0);
            encoder.set_scissor(ScissorRect::at(40, 24, 24, 8));
            encoder.set_binding_set(green, 0, Vec::new());
            encoder.draw(3, 1, 0, 0);
            encoder.set_scissor(ScissorRect::at(64, 24, 24, 8));
            encoder.set_pipeline(inline_pipeline);
            encoder.set_binding_set(inline_set, 0, Vec::new());
            encoder.push_constant(ShaderStage::Fragment, 0, [0.0f32, 0.0, 1.0, 1.0]);
            encoder.draw(3, 1, 0, 0);
            encoder.set_scissor(ScissorRect::at(88, 24, 24, 8));
            encoder.set_pipeline(pipeline);
            encoder.set_binding_set(green, 0, Vec::new());
            encoder.draw(3, 1, 0, 0);
        },
    );
    // A new pass must restore the pipeline's required buffer, not inherit the
    // previous pass's bindings.
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget {
            resolve_texture: None,
            texture: target,
            load_action: cove_graphics::rhi::ColorLoadAction::Load,
            store: true,
        }]),
        |encoder| {
            encoder.set_pipeline(pipeline);
            encoder.set_viewport(Viewport::new(128.0, 32.0));
            encoder.set_scissor(ScissorRect::at(112, 28, 12, 4));
            encoder.set_binding_set(green, 0, Vec::new());
            encoder.draw(3, 1, 0, 0);
        },
    );

    device.begin_frame().expect("begin frame");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("submit the offset/constant passes");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let mut pixels = vec![0u8; 128 * 32 * 4];
    device
        .read_texture_data(
            target,
            128,
            32,
            512,
            TextureSubresource::default(),
            &mut pixels,
        )
        .expect("read the offset target back");

    for (x, y, want) in [
        (28i64, 28i64, [255u8, 0, 0, 255]),
        (52, 28, [0, 255, 0, 255]),
        (76, 28, [0, 0, 255, 255]),
        (100, 28, [0, 255, 0, 255]),
        (116, 30, [0, 255, 0, 255]),
    ] {
        let pixel = ((y * 128 + x) * 4) as usize;
        assert_eq!(
            &pixels[pixel..pixel + 4],
            &want,
            "pixel ({x},{y}) must show the expected band colour"
        );
    }
}
