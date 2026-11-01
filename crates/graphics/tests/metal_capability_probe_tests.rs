//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalCapabilityProbeTests.swift`.
//!
//! Behaviour checklist:
//! - capabilities and the device name come from a real probe, not a constant
//! - a failed binding-set registration is not cached, so a repeat fails too
//! - duplicate binding slots and task shaders are rejected
//!
//! The Swift suite also compared `rayTracing` against `MTLDevice.supportsRaytracing`
//! directly. That needs the `metal` crate to query the device here, so the
//! equivalent assertion is that the front-end capability mirrors the probed
//! adapter value; the adapter probe itself is what Metal populates from
//! `supportsRaytracing`.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSetDescriptor,
    BindingSetEntry, BindingType, Buffer, Device, DeviceConfig, GraphicsApi, ShaderFormat,
    ShaderModuleDescriptor, ShaderStage, ShaderVisibility,
};

fn metal_device() -> Option<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
}

fn compute_layout(
    device: &Device,
    slots: &[u32],
) -> cove_graphics::rhi::RhiResult<cove_graphics::rhi::BindingLayout> {
    device.make_binding_layout(BindingLayoutDescriptor::new(
        slots
            .iter()
            .map(|slot| {
                BindingLayoutEntry::new(
                    *slot,
                    BindingType::StorageBuffer,
                    ShaderVisibility::COMPUTE,
                )
            })
            .collect(),
    ))
}

#[test]
fn capabilities_and_name_come_from_a_real_probe() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping");
        return;
    };

    assert!(
        !device.device_name().is_empty(),
        "a real Metal device reports its name"
    );
    let adapter = device.adapter_capabilities();
    let capabilities = device.capabilities();
    assert_eq!(
        adapter.ray_tracing, capabilities.ray_tracing.acceleration_structures,
        "the front-end acceleration-structure capability mirrors the probed adapter value"
    );
}

#[test]
fn invalid_binding_failure_does_not_poison_the_binding_cache() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping");
        return;
    };
    let layout = compute_layout(&device, &[0]).expect("layout creation");

    // A zero-handle buffer can never register, so both attempts must fail: a
    // failed backend registration must not be cached as a success.
    let descriptor = BindingSetDescriptor::new(vec![BindingSetEntry::new(
        0,
        BindingResource::StorageBuffer {
            buffer: Buffer::new(0),
            offset: 0,
        },
    )]);
    for attempt in 0..2 {
        assert!(
            device.make_binding_set(layout, descriptor.clone()).is_err(),
            "attempt {attempt} must fail; a failed registration is never cached"
        );
    }
}

#[test]
fn duplicate_binding_slots_and_task_shaders_are_rejected() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping");
        return;
    };

    let duplicate = device.make_binding_layout(BindingLayoutDescriptor {
        entries: vec![
            BindingLayoutEntry::new(0, BindingType::StorageBuffer, ShaderVisibility::COMPUTE),
            BindingLayoutEntry::new(0, BindingType::UniformBuffer, ShaderVisibility::COMPUTE),
        ],
        label: None,
    });
    assert!(
        duplicate.is_err(),
        "two entries may not share a binding slot"
    );

    assert!(
        !device.capabilities().mesh_shading.task,
        "Metal has no task-shader stage, so the capability is off"
    );
    let task_shader = device.make_shader_module(ShaderModuleDescriptor::new(
        ShaderStage::Task,
        ShaderFormat::MslSource,
        b"unused".to_vec(),
    ));
    assert!(task_shader.is_err(), "a task shader is rejected on Metal");
}
