//! Translation of `Graphics/Tests/NativeRHITests/NativeRHISmokeTests.swift`.
//!
//! Behaviour checklist:
//! - the module reports a version and a non-empty default backend list
//! - an invalid device configuration fails with `InvalidArgument` before any
//!   backend is constructed
//! - on macOS the default backend is Metal, Vulkan and DX12 are not compiled
//!   in, and asking for Vulkan yields `UnsupportedBackend`

use cove_graphics::rhi::{Device, DeviceConfig, GraphicsApi, RhiError, platform_default_backends};
use cove_graphics::rhi::is_compiled_in;

#[test]
fn module_reports_a_version_and_default_backends() {
    assert!(
        !cove_graphics::rhi::VERSION.is_empty(),
        "the graphics module reports a version"
    );
    assert!(
        !platform_default_backends().is_empty(),
        "a host always has at least one default backend"
    );
}

#[test]
fn invalid_configuration_fails_before_backend_creation() {
    let cases = [
        DeviceConfig {
            preferred_backends: Vec::new(),
            ..DeviceConfig::default()
        },
        DeviceConfig {
            frames_in_flight: 0,
            ..DeviceConfig::default()
        },
    ];
    for config in cases {
        let error = match Device::make(config) {
            Ok(_) => panic!("an invalid configuration is rejected"),
            Err(error) => error,
        };
        assert!(
            matches!(error, RhiError::InvalidArgument(_)),
            "expected InvalidArgument, got {error:?}"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn mac_uses_metal_without_vulkan_fallback() {
    assert_eq!(
        platform_default_backends(),
        vec![GraphicsApi::Metal],
        "macOS defaults to Metal alone"
    );
    assert!(
        !is_compiled_in(GraphicsApi::Vulkan),
        "Vulkan is not built into the macOS binary"
    );
    assert!(
        !is_compiled_in(GraphicsApi::Dx12),
        "DX12 is not built into the macOS binary"
    );

    let error = match Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Vulkan],
        ..DeviceConfig::default()
    }) {
        Ok(_) => panic!("Vulkan cannot be created on macOS"),
        Err(error) => error,
    };
    assert!(
        matches!(error, RhiError::UnsupportedBackend(_)),
        "expected UnsupportedBackend, got {error:?}"
    );
}
