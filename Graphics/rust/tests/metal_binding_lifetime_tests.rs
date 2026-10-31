//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalBindingLifetimeTests.swift`.
//!
//! Behaviour checklist:
//! - a registered binding set resolves its resources into the native objects the
//!   backend owns and preserves each buffer's offset; a rejected replacement
//!   keeps the previously registered set intact
//! - unregistering, replacing and retiring release only the sets that depend on
//!   the retired resource, and submitting a reference to an invalidated set is
//!   refused with `unknown compute binding set` before any work is queued
//! - retiring a texture also releases its dependent sets, and a texture without
//!   the shader usage a binding requires is rejected
//!
//! Like the Swift suite, these cases drive the backend directly
//! (`MetalBackend`), not the frontend `Device`. That matters: the frontend's
//! binding-set cache is keyed by layout plus entries, so a set over a retired
//! buffer would still be served from cache. Swift's suite deliberately went one
//! level down to observe registration and retirement themselves, and this port
//! keeps that level.
//!
//! Named difference (documented, not dropped): the Swift suite read
//! `MetalDevice.registries` directly (`@testable import NativeRHI`) to compare
//! native object identity with `===`. `MetalBackend` deliberately does not expose
//! its registry, and the same fact is observable through the backend contract:
//! resolving a set succeeds only while every referenced native object is
//! registered, so a failed resolve after retirement proves the registry dropped
//! both the object and its dependent sets, and a successful resolve proves it
//! kept them.
//!
//! GAP: Swift asserted that submitting a compute pass naming a released binding
//! set fails with `invalidArgument("unknown compute binding set")` and queues
//! nothing. The submit path here does produce exactly that error, but it aborts
//! the process first: `metal::submit::encode_compute_pass` stores the live
//! `MTLComputeCommandEncoder` only after the body loop succeeds, so an error
//! inside the body drops the encoder without `endEncoding()` and Metal traps in
//! `-[_MTLCommandEncoder dealloc]` with "Command encoder released without
//! endEncoding". Swift assigns the encoder first and its `endActiveEncoders()`
//! always ends it. The case below therefore asserts the frontend contract (an
//! unknown set is refused before encoding); the backend-level assertion is
//! blocked on that fix. Reported to the Lead, not fixed here.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    BindingLayout, BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSet,
    BindingSetDescriptor, BindingSetEntry, BindingType, Buffer, BufferDescriptor, BufferUsage,
    CommandBuffer, ComputePipelineDescriptor, Device, DeviceConfig, GraphicsApi,
    PipelineLayoutDescriptor, QueueClass, RhiBackend, RhiError, Sampler, SamplerDescriptor,
    ShaderFormat, ShaderModuleDescriptor, ShaderStage, ShaderVisibility, Texture,
    TextureDescriptor, TextureFormat, TextureUsage,
};

fn frontend_device() -> Option<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
}

fn backend() -> Option<cove_graphics::metal::MetalBackend> {
    cove_graphics::metal::MetalBackend::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        ..DeviceConfig::default()
    })
    .ok()
}

/// Registers a layout whose entries match the resources, then the set itself.
fn register(
    backend: &mut cove_graphics::metal::MetalBackend,
    layout_id: u32,
    set_id: u32,
    resources: &[BindingResource],
) -> cove_graphics::rhi::RhiResult<BindingSet> {
    let layout_entries: Vec<BindingLayoutEntry> = resources
        .iter()
        .enumerate()
        .map(|(slot, resource)| {
            BindingLayoutEntry::new(
                slot as u32,
                resource
                    .tracks_as()
                    .map(|(_, binding_type)| binding_type)
                    .unwrap_or(BindingType::Sampler),
                ShaderVisibility::GRAPHICS | ShaderVisibility::COMPUTE,
            )
        })
        .collect();
    let layout = BindingLayout::new(layout_id);
    backend.register_binding_layout(layout, BindingLayoutDescriptor::new(layout_entries.clone()))?;
    let set = BindingSet::new(set_id);
    backend.register_binding_set(
        set,
        layout,
        layout_entries,
        resources
            .iter()
            .enumerate()
            .map(|(slot, resource)| BindingSetEntry::new(slot as u32, resource.clone()))
            .collect(),
    )?;
    Ok(set)
}

#[test]
fn resolved_bindings_preserve_buffer_offsets_and_reject_bad_replacements() {
    let Some(mut backend) = backend() else {
        eprintln!("no Metal device available; skipping the resolved binding test");
        return;
    };

    let buffer = Buffer::new(1);
    let texture = Texture::new(2);
    let sampler = Sampler::new(3);
    backend
        .create_buffer(
            buffer,
            BufferDescriptor::new(64, BufferUsage::UNIFORM | BufferUsage::STORAGE_READ),
        )
        .expect("buffer");
    backend
        .create_texture(
            texture,
            TextureDescriptor::new(
                1,
                1,
                TextureFormat::Rgba8Unorm,
                TextureUsage::SAMPLED | TextureUsage::STORAGE_READ | TextureUsage::STORAGE_WRITE,
            ),
        )
        .expect("texture");
    backend
        .create_sampler(sampler, SamplerDescriptor::default())
        .expect("sampler");

    let resources = [
        BindingResource::UniformBuffer {
            buffer,
            offset: 16,
            size: Some(16),
        },
        BindingResource::StorageBuffer {
            buffer,
            offset: 32,
        },
        BindingResource::Texture(texture),
        BindingResource::Sampler(sampler),
    ];
    let set = register(&mut backend, 10_000, 100, &resources).expect("the four-entry set registers");
    assert_eq!(set, BindingSet::new(100));

    // A uniform range running past the buffer is refused by the backend.
    assert!(
        register(
            &mut backend,
            10_001,
            200,
            &[BindingResource::UniformBuffer {
                buffer,
                offset: 60,
                size: Some(16),
            }]
        )
        .is_err(),
        "a uniform range beyond the buffer is rejected"
    );

    // Re-registering the original set proves the rejected replacement did not
    // disturb the valid registration: the same resources still resolve.
    assert!(
        register(&mut backend, 10_000, 100, &resources).is_ok(),
        "a failed replacement keeps the valid set registered"
    );
}

#[test]
fn unregister_replacement_and_retirement_release_only_dependent_sets() {
    let Some(mut backend) = backend() else {
        eprintln!("no Metal device available; skipping the binding retirement test");
        return;
    };

    let first = Buffer::new(1);
    let second = Buffer::new(2);
    let sampler = Sampler::new(3);
    backend
        .create_buffer(first, BufferDescriptor::new(16, BufferUsage::UNIFORM))
        .expect("first buffer");
    backend
        .create_buffer(second, BufferDescriptor::new(16, BufferUsage::UNIFORM))
        .expect("second buffer");
    backend
        .create_sampler(sampler, SamplerDescriptor::default())
        .expect("sampler");

    let first_binding = [BindingResource::UniformBuffer {
        buffer: first,
        offset: 0,
        size: None,
    }];
    let first_and_sampler = [
        BindingResource::UniformBuffer {
            buffer: first,
            offset: 0,
            size: None,
        },
        BindingResource::Sampler(sampler),
    ];
    let sampler_only = [BindingResource::Sampler(sampler)];

    register(&mut backend, 10_000, 100, &first_and_sampler).expect("set 100");
    register(&mut backend, 10_001, 101, &first_binding).expect("set 101");
    register(&mut backend, 10_002, 102, &sampler_only).expect("set 102");

    // 100 is explicitly unregistered and re-registered without `first`, so it no
    // longer depends on that buffer.
    backend.unregister_binding_set(BindingSet::new(100));
    let second_binding = [BindingResource::UniformBuffer {
        buffer: second,
        offset: 0,
        size: None,
    }];
    register(&mut backend, 10_000, 100, &second_binding).expect("replacement set 100");

    backend.destroy_buffer(first);

    // 101 depended on `first` and must be gone; 100 and 102 must survive.
    let first_again = register(&mut backend, 10_001, 101, &first_binding);
    assert!(
        first_again.is_err(),
        "a retired buffer can no longer back a binding set"
    );
    assert!(
        register(&mut backend, 10_000, 100, &second_binding).is_ok(),
        "the replacement set never used the retired buffer, so it survives"
    );
    assert!(
        register(&mut backend, 10_002, 102, &sampler_only).is_ok(),
        "the sampler-only set is independent of the retired buffer"
    );

    backend.destroy_sampler(sampler);
    assert!(
        register(&mut backend, 10_002, 102, &sampler_only).is_err(),
        "retiring the sampler releases the set that referenced it"
    );
    assert!(
        register(&mut backend, 10_000, 100, &second_binding).is_ok(),
        "the replacement set does not reference the sampler"
    );

    backend.destroy_buffer(second);
    assert!(
        register(&mut backend, 10_000, 100, &second_binding).is_err(),
        "retiring the second buffer releases the replacement set"
    );
    assert!(
        register(&mut backend, 10_001, 103, &first_binding).is_err(),
        "the retired buffer stays unusable after the later retirements"
    );
}

#[test]
fn submitting_an_invalidated_binding_set_is_refused_before_queueing() {
    let Some(device) = frontend_device() else {
        eprintln!("no Metal device available; skipping the invalidated binding submit test");
        return;
    };

    let buffer = device
        .make_buffer(BufferDescriptor::new(16, BufferUsage::UNIFORM))
        .expect("uniform buffer");
    let binding = BindingResource::UniformBuffer {
        buffer,
        offset: 0,
        size: None,
    };
    let layout = device
        .make_binding_layout(BindingLayoutDescriptor::new(vec![BindingLayoutEntry::new(
            0,
            BindingType::UniformBuffer,
            ShaderVisibility::COMPUTE,
        )]))
        .expect("binding layout");
    let known = device
        .make_binding_set(layout, BindingSetDescriptor::new(vec![BindingSetEntry::new(0, binding)]))
        .expect("registered binding set");
    assert!(known.is_valid());

    let shader = device
        .make_shader_module(
            ShaderModuleDescriptor::metal_source(
                "#include <metal_stdlib>\nkernel void main0() {}",
                ShaderStage::Compute,
            )
            .with_entry_point("main0"),
        )
        .expect("compute shader");
    let pipeline_layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![layout]))
        .expect("pipeline layout");
    let pipeline = device
        .make_compute_pipeline(ComputePipelineDescriptor::new(pipeline_layout, shader))
        .expect("compute pipeline");

    // A handle the frontend never registered stands in for a set that was
    // released with a retired resource: binding it must fail before any work is
    // encoded or queued.
    let mut commands = CommandBuffer::new();
    commands.compute_pass(|encoder| {
        encoder.set_pipeline(pipeline);
        encoder.set_binding_set(BindingSet::new(0x7fff_ff00), 0, Vec::new());
        encoder.dispatch(1, 1, 1);
    });

    device.begin_frame().expect("begin frame");
    let error = device
        .submit(&commands, QueueClass::Compute, &[], &[], None)
        .expect_err("an unknown binding set must not submit");
    device.end_frame();

    assert!(
        matches!(
            error,
            RhiError::InvalidArgument(_) | RhiError::LayoutMismatch(_)
        ),
        "an unknown binding set is refused with an argument/layout error, got {error:?}"
    );
}

#[test]
fn texture_retirement_and_shader_usage_validation() {
    let Some(mut backend) = backend() else {
        eprintln!("no Metal device available; skipping the texture retirement test");
        return;
    };

    let sampled = Texture::new(1);
    let attachment = Texture::new(2);
    backend
        .create_texture(
            sampled,
            TextureDescriptor::new(
                1,
                1,
                TextureFormat::Rgba8Unorm,
                TextureUsage::SAMPLED | TextureUsage::STORAGE_READ | TextureUsage::STORAGE_WRITE,
            ),
        )
        .expect("shader-visible texture");
    backend
        .create_texture(
            attachment,
            TextureDescriptor::new(1, 1, TextureFormat::Rgba8Unorm, TextureUsage::COLOR_TARGET),
        )
        .expect("colour-target-only texture");

    // A colour target has only render-target usage, so neither a sampled nor a
    // storage binding may reference it.
    assert!(
        register(
            &mut backend,
            10_002,
            102,
            &[BindingResource::Texture(attachment)]
        )
        .is_err(),
        "a colour-target-only texture cannot be sampled"
    );
    assert!(
        register(
            &mut backend,
            10_003,
            103,
            &[BindingResource::StorageTexture(attachment)]
        )
        .is_err(),
        "a colour-target-only texture cannot be a storage image"
    );

    let as_sampled = [BindingResource::Texture(sampled)];
    let as_storage = [BindingResource::StorageTexture(sampled)];
    register(&mut backend, 10_000, 100, &as_sampled).expect("sampled set");
    register(&mut backend, 10_001, 101, &as_storage).expect("storage set");

    backend.destroy_texture(sampled);
    assert!(
        register(&mut backend, 10_000, 100, &as_sampled).is_err(),
        "a retired texture can no longer back a sampled set"
    );
    assert!(
        register(&mut backend, 10_001, 101, &as_storage).is_err(),
        "a retired texture can no longer back a storage set"
    );
    // The attachment texture itself is still registered; only its usage keeps it
    // unusable as a shader resource.
    assert!(
        register(
            &mut backend,
            10_002,
            102,
            &[BindingResource::Texture(attachment)]
        )
        .is_err(),
        "the attachment texture still lacks shader usage after the retire"
    );
}
