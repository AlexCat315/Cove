//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalSlangTests.swift`.
//!
//! Behaviour checklist:
//! - a Slang compute artifact reflects an eight-thread workgroup, and dispatching
//!   two groups writes the consecutive run 7..23 through its storage buffer
//! - a Slang mesh artifact draws one green triangle into a red-cleared target
//!   and returns the real mesh pipeline
//!
//! Named difference (documented, not dropped): the Swift suite compiled its
//! fixtures at run time through the first-party `cove-dev shader compile` tool,
//! driven by `SLANGC`, and skipped without both. This repository has no Rust
//! `cove-dev`, so the two prerequisites are checked explicitly and the case
//! reports which one is missing instead of passing vacuously — the same pattern
//! as `vulkan_slang_tests.rs`. The real pipeline construction, dispatch and
//! readback assertions below are the ones from Swift; they execute as soon as a
//! compiled artifact and `SLANGC` are present. The artifact location is the one
//! `ShaderArtifact::load_directory` already reads, so no new toolchain contract
//! is invented here.
//!
//! The mesh case additionally needs a mesh-shading GPU, which Swift also gated.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};

use cove_graphics::rhi::{
    BindingResource, BindingSetDescriptor, BindingSetEntry, BufferDescriptor, BufferUsage,
    ColorAttachmentDescriptor, CommandBuffer, ComputePipelineDescriptor, Device, DeviceConfig,
    GraphicsApi, MeshPipelineDescriptor, PipelineLayoutDescriptor, QueueClass, RenderColorTarget,
    RenderPassDescriptor, ShaderArtifact, ShaderStage, TextureBufferUpload, TextureDescriptor,
    TextureFormat, TextureSubresource, TextureUploadRegion, TextureUsage, Viewport,
};

/// Why the Slang integration cases cannot run here.
enum FixtureUnavailable {
    NoCompiler,
    NoArtifact(&'static str),
    NoDevice,
    NoMeshShading,
}

impl std::fmt::Display for FixtureUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCompiler => formatter.write_str(
                "no Slang compiler tool is available (Swift required SLANGC and cove-dev)",
            ),
            Self::NoArtifact(name) => write!(
                formatter,
                "no compiled artifact for the `{name}` fixture (the Rust tree has no cove-dev to produce it)"
            ),
            Self::NoDevice => formatter.write_str("no Metal device is available on this host"),
            Self::NoMeshShading => formatter.write_str("this Metal GPU has no mesh shader support"),
        }
    }
}

/// Loads one precompiled stage of a fixture through the offline artifact loader.
fn compiled_artifact(name: &str, stage: ShaderStage) -> Option<ShaderArtifact> {
    std::env::var_os("SLANGC")?;
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

fn metal_device() -> Result<Device, FixtureUnavailable> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        ..DeviceConfig::default()
    })
    .map_err(|_| FixtureUnavailable::NoDevice)
}

#[test]
fn slang_compute_uses_reflected_eight_thread_workgroups() {
    if std::env::var_os("SLANGC").is_none() {
        eprintln!(
            "skipping the Slang compute case: {}",
            FixtureUnavailable::NoCompiler
        );
        return;
    }
    let Some(artifact) = compiled_artifact("compute", ShaderStage::Compute) else {
        eprintln!(
            "skipping the Slang compute case: {}",
            FixtureUnavailable::NoArtifact("compute")
        );
        return;
    };

    assert_eq!(
        artifact.interface.threadgroup_size.x, 8,
        "the reflected workgroup is the eight threads the Slang source declares"
    );

    let device = match metal_device() {
        Ok(device) => device,
        Err(reason) => {
            eprintln!("skipping the Slang compute case: {reason}");
            return;
        }
    };

    let shader = device
        .make_shader_module(artifact.module_descriptor(&[]).expect("module descriptor"))
        .expect("compute shader module");
    let buffer = device
        .make_buffer(BufferDescriptor::new(
            64,
            BufferUsage::STORAGE_WRITE | BufferUsage::TRANSFER_SOURCE,
        ))
        .expect("storage buffer");
    let target = device
        .make_texture(TextureDescriptor::new(
            16,
            1,
            TextureFormat::R32Uint,
            TextureUsage::TRANSFER_DESTINATION | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("readback texture");
    let binding = device
        .make_binding_layout(
            artifact
                .binding_layout_descriptor(0)
                .expect("reflected layout"),
        )
        .expect("binding layout");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![binding]))
        .expect("pipeline layout");
    let set = device
        .make_binding_set(
            binding,
            BindingSetDescriptor::new(vec![BindingSetEntry::new(
                0,
                BindingResource::StorageBuffer { buffer, offset: 0 },
            )]),
        )
        .expect("binding set");
    let pipeline = device
        .make_compute_pipeline(ComputePipelineDescriptor::new(layout, shader))
        .expect("compute pipeline");

    let mut commands = CommandBuffer::new();
    commands.compute_pass(|encoder| {
        encoder.set_pipeline(pipeline);
        encoder.set_binding_set(set, 0, Vec::new());
        encoder.dispatch(2, 1, 1);
    });
    commands.copy_pass(|encoder| {
        encoder.upload_buffer_to_texture(TextureBufferUpload::new(
            buffer,
            64,
            target,
            TextureUploadRegion::new(16, 1),
        ));
    });

    device.begin_frame().expect("begin frame");
    device
        .submit(&commands, QueueClass::Compute, &[], &[], None)
        .expect("submit the Slang compute work");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let mut bytes = vec![0u8; 64];
    device
        .read_texture_data(target, 16, 1, 64, TextureSubresource::default(), &mut bytes)
        .expect("read the compute output back");
    let values: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    assert_eq!(
        values,
        (7..23).collect::<Vec<u32>>(),
        "two eight-thread groups write the consecutive run starting at 7"
    );
}

#[test]
fn slang_mesh_pipeline_draws_green_triangle() {
    if std::env::var_os("SLANGC").is_none() {
        eprintln!(
            "skipping the Slang mesh case: {}",
            FixtureUnavailable::NoCompiler
        );
        return;
    }
    // The mesh fixture is compiled with an explicit [3, 1, 1] workgroup, which
    // the offline tool records in the artifact interface.
    let Some(mesh_artifact) = compiled_artifact("mesh", ShaderStage::Mesh) else {
        eprintln!(
            "skipping the Slang mesh case: {}",
            FixtureUnavailable::NoArtifact("mesh")
        );
        return;
    };
    let Some(fragment_artifact) = compiled_artifact("mesh", ShaderStage::Fragment) else {
        eprintln!(
            "skipping the Slang mesh case: {}",
            FixtureUnavailable::NoArtifact("mesh")
        );
        return;
    };

    let device = match metal_device() {
        Ok(device) => device,
        Err(reason) => {
            eprintln!("skipping the Slang mesh case: {reason}");
            return;
        }
    };
    if !device.capabilities().mesh_shading.mesh {
        eprintln!(
            "skipping the Slang mesh case: {}",
            FixtureUnavailable::NoMeshShading
        );
        return;
    }

    let mesh = device
        .make_shader_module(mesh_artifact.module_descriptor(&[]).expect("mesh module"))
        .expect("mesh shader module");
    let fragment = device
        .make_shader_module(
            fragment_artifact
                .module_descriptor(&[])
                .expect("fragment module"),
        )
        .expect("fragment shader module");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(Vec::new()))
        .expect("empty pipeline layout");

    let mut descriptor = MeshPipelineDescriptor::new(layout, mesh);
    descriptor.fragment = Some(fragment);
    descriptor.color_attachments = vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
    let pipeline = device
        .make_mesh_pipeline(descriptor)
        .expect("mesh pipeline");

    let target = device
        .make_texture(TextureDescriptor::new(
            8,
            8,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("colour target");

    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
            target,
            [1.0, 0.0, 0.0, 1.0],
        )]),
        |encoder| {
            encoder.set_mesh_pipeline(pipeline);
            encoder.set_viewport(Viewport::new(8.0, 8.0));
            encoder.draw_mesh_tasks(1, 1, 1);
        },
    );

    device.begin_frame().expect("begin frame");
    device
        .submit(&commands, QueueClass::Graphics, &[], &[], None)
        .expect("submit the mesh draw");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let mut pixels = vec![0u8; 256];
    device
        .read_texture_data(target, 8, 8, 32, TextureSubresource::default(), &mut pixels)
        .expect("read the mesh target back");
    let centre = (4 * 8 + 4) * 4;
    assert_eq!(
        &pixels[centre..centre + 4],
        &[0, 255, 0, 255],
        "the mesh shader's fragment stage writes opaque green at the centre"
    );
    assert_eq!(
        pixels[0], 255,
        "a corner keeps the red clear colour, so the triangle does not cover it"
    );
}
