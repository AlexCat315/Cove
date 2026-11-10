//! Translation of NativeRHI/NativeRHI.swift: module constants and the platform
//! backend selection used by `DeviceConfig` defaults.
use super::{GraphicsApi, TextureFormat};

/// Semantic version of the RHI port.
pub const VERSION: &str = "0.1.0";

/// Backend preference order for the current platform. The first backend that
/// constructs successfully wins; later entries are fallbacks.
pub fn platform_default_backends() -> Vec<GraphicsApi> {
    #[cfg(target_os = "macos")]
    {
        vec![GraphicsApi::Metal]
    }
    #[cfg(target_os = "ios")]
    {
        vec![GraphicsApi::Metal]
    }
    #[cfg(target_os = "windows")]
    {
        // The DX12 encoder still lacks descriptor and draw bindings.
        vec![GraphicsApi::Vulkan, GraphicsApi::Dx12]
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        vec![GraphicsApi::Vulkan]
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        Vec::new()
    }
}

/// Android presentation commonly exposes RGBA surfaces; desktop renderers use
/// the BGRA sRGB convention shared with Metal.
pub fn platform_default_surface_format() -> TextureFormat {
    #[cfg(target_os = "android")]
    {
        TextureFormat::Rgba8Unorm
    }
    #[cfg(not(target_os = "android"))]
    {
        TextureFormat::Bgra8UnormSrgb
    }
}

/// Whether a backend is part of the current build. Backends that are
/// source-only on the host (for example DX12 on macOS) report unavailable.
pub fn is_compiled_in(api: GraphicsApi) -> bool {
    match api {
        GraphicsApi::Metal => cfg!(any(target_os = "macos", target_os = "ios")),
        GraphicsApi::Vulkan => cfg!(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "android"
        )),
        GraphicsApi::Dx12 => cfg!(target_os = "windows"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_backend_is_always_compiled_in() {
        for api in platform_default_backends() {
            assert!(is_compiled_in(api), "{api:?} should be built for this host");
        }
    }

    #[test]
    fn surface_format_follows_the_platform() {
        #[cfg(target_os = "android")]
        assert_eq!(platform_default_surface_format(), TextureFormat::Rgba8Unorm);
        #[cfg(not(target_os = "android"))]
        assert_eq!(
            platform_default_surface_format(),
            TextureFormat::Bgra8UnormSrgb
        );
    }
}
