//! Translation of
//! `Graphics/Tests/NativeRHITests/Vulkan/VulkanCapabilityProbeTests.swift`.
//!
//! Vulkan availability must be queryable without an installed ICD, and when a
//! driver *is* present the reported capabilities must be self-consistent: a
//! claimed feature implies its prerequisite, and the two features Vulkan never
//! implements here (ray-tracing pipelines, indirect mesh dispatch) stay false.
//! Invalid compute commands must be rejected while an empty compute submit
//! succeeds.
//!
//! Reachability: the Swift file is `#if (os(Windows) || os(Linux)) && canImport(CVulkanHeaders)`
//! and each case begins `guard VulkanBackend.isAvailable else { throw XCTSkip(...) }`.
//! The whole target is therefore gated to Windows/Linux, matching
//! `vulkan_slang_tests.rs`. On this macOS host it is built and linked but never
//! run; on a Vulkan host the driver-dependent cases skip when no loader/ICD is
//! present, exactly as Swift did.
//!
//! Behaviour checklist:
//! - querying availability never crashes, whether or not a driver is installed
//! - a device is only produced when the backend is compiled in and a driver is
//!   present
//! - on a Vulkan host: mesh task implies mesh, ray query implies structures
//! - on a Vulkan host: ray-tracing pipelines and indirect mesh stay unsupported
//! - on a Vulkan host: graphics and compute are always advertised
//! - on a Vulkan host: a dispatch without a pipeline is rejected and an empty
//!   compute submit succeeds

#![cfg(any(target_os = "linux", target_os = "windows"))]

use cove_graphics::{CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass, is_compiled_in};

fn vulkan() -> Option<Device> {
    if !is_compiled_in(GraphicsApi::Vulkan) {
        return None;
    }
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    })
    .ok()
}

#[test]
fn availability_query_never_crashes() {
    // The Swift case is a bare `_ = VulkanBackend.isAvailable`. The Rust
    // equivalent must be a total function on every host.
    let compiled = is_compiled_in(GraphicsApi::Vulkan);
    eprintln!("Vulkan compiled in: {compiled}");
    assert_eq!(
        compiled,
        is_compiled_in(GraphicsApi::Vulkan),
        "the availability probe is stable across calls",
    );
}

#[test]
fn missing_driver_is_refused_without_panicking() {
    // The backend is compiled in on both gated platforms, so the interesting
    // refusal is a host with no loader/ICD rather than an uncompiled backend.
    if vulkan().is_some() {
        eprintln!("a Vulkan driver is present; the refusal case does not apply");
        return;
    }
    let result = Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    });
    assert!(
        result.is_err(),
        "a host without a loader/ICD cannot produce a Vulkan device",
    );
}

#[test]
fn capabilities_are_self_consistent() {
    let Some(device) = vulkan() else {
        eprintln!("skipping: no Vulkan loader/ICD on this host");
        return;
    };
    let name = device.device_name();
    assert!(!name.is_empty(), "the backend reports a device name");

    let capabilities = device.capabilities();
    let adapter = device.adapter_capabilities();

    assert!(
        !capabilities.mesh_shading.task || capabilities.mesh_shading.mesh,
        "a task shader implies mesh shading",
    );
    assert!(
        !capabilities.ray_tracing.compute_ray_query
            || (capabilities.ray_tracing.acceleration_structures && adapter.ray_tracing),
        "compute ray query implies acceleration structures and adapter support",
    );
    assert!(
        !capabilities.ray_tracing.pipelines,
        "ray-tracing pipelines are not implemented by this backend",
    );
    assert!(
        !capabilities.mesh_shading.indirect,
        "indirect mesh dispatch is not implemented by this backend",
    );
    assert!(capabilities.graphics, "graphics is always available");
    assert!(capabilities.compute, "compute is always available");
}

#[test]
fn invalid_compute_commands_are_rejected_and_empty_submit_succeeds() {
    let Some(device) = vulkan() else {
        eprintln!("skipping: no Vulkan loader/ICD on this host");
        return;
    };

    // A dispatch with no pipeline bound cannot be recorded into a valid submit.
    let mut invalid = CommandBuffer::new();
    invalid.compute_pass(|pass| pass.dispatch(1, 1, 1));
    device.begin_frame().expect("the frame opens");
    assert!(
        device
            .submit(&invalid, QueueClass::Graphics, &[], &[], None)
            .is_err(),
        "a dispatch without a pipeline is rejected",
    );

    // An empty compute submission is legal.
    let empty = CommandBuffer::new();
    device
        .submit(&empty, QueueClass::Compute, &[], &[], None)
        .expect("an empty compute submit succeeds");
    device.end_frame();
    device.wait_until_idle().expect("the device drains");
}
