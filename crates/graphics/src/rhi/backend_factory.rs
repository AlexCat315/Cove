//! Translation of NativeRHI/BackendFactory.swift.
//!
//! Maps a selected [`GraphicsApi`] to its concrete backend. A backend that is
//! not part of the current build returns an honest unsupported error rather
//! than being referenced as a missing symbol.
//!
//! Swift resolves the mapping at compile time with `#if canImport(...)`. The
//! Rust port keeps that property with a constructor registry: each backend
//! module installs its constructor through [`register_backend`] from
//! `install_backends`, and a platform that never installs one reports
//! "not built for this platform".
use std::sync::{Mutex, Once};

use super::{DeviceConfig, GraphicsApi, RhiBackend, RhiError, RhiResult};

/// Builds a backend for the given device configuration.
pub type BackendConstructor = fn(&DeviceConfig) -> RhiResult<Box<dyn RhiBackend>>;

static CONSTRUCTORS: Mutex<Vec<(GraphicsApi, BackendConstructor)>> = Mutex::new(Vec::new());

/// Installs `constructor` as the builder for `api`. Re-registering replaces the
/// previous constructor, which keeps repeated `install_backends` calls
/// idempotent.
pub fn register_backend(api: GraphicsApi, constructor: BackendConstructor) {
    let mut constructors = CONSTRUCTORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match constructors
        .iter_mut()
        .find(|(registered, _)| *registered == api)
    {
        Some(entry) => entry.1 = constructor,
        None => constructors.push((api, constructor)),
    }
}

/// Whether a constructor for `api` is part of this build.
pub fn is_backend_registered(api: GraphicsApi) -> bool {
    install_backends();
    let constructors = CONSTRUCTORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    constructors
        .iter()
        .any(|(registered, _)| *registered == api)
}

/// Installs every backend this build can construct. Each module is referenced
/// only under the `cfg` of the platform whose framework it needs, so a host
/// never links a backend it cannot run.
///
/// Calling this more than once is harmless: [`register_backend`] replaces any
/// existing constructor for the same API.
pub fn install_backends() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            register_backend(GraphicsApi::Metal, crate::metal::make_backend);
        }
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "android"))]
        {
            register_backend(GraphicsApi::Vulkan, crate::vulkan::make_backend);
        }
        #[cfg(target_os = "windows")]
        {
            register_backend(GraphicsApi::Dx12, crate::dx12::make_backend);
        }
        #[cfg(target_arch = "wasm32")]
        {
            register_backend(GraphicsApi::WebGpu, crate::webgpu::make_backend);
        }
    });
}

/// Constructs the backend for `api`, installing the build's backends first.
pub fn make_backend(api: GraphicsApi, config: &DeviceConfig) -> RhiResult<Box<dyn RhiBackend>> {
    install_backends();
    let constructor = {
        let constructors = CONSTRUCTORS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        constructors
            .iter()
            .find(|(registered, _)| *registered == api)
            .map(|(_, constructor)| *constructor)
    };
    match constructor {
        Some(constructor) => constructor(config),
        None => Err(RhiError::unsupported_backend(not_built_message(api))),
    }
}

fn not_built_message(api: GraphicsApi) -> &'static str {
    match api {
        GraphicsApi::Metal => "Metal is only available on Apple platforms",
        GraphicsApi::Vulkan => "Vulkan is built for Windows, Linux and Android; macOS uses Metal",
        GraphicsApi::Dx12 => "DirectX 12 is only available on Windows",
        GraphicsApi::WebGpu => "WebGPU requires the browser host",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unregistered_backend_reports_why() {
        let config = DeviceConfig::default();
        // Metal is never registered on a non-Apple host, and vice versa.
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        assert!(make_backend(GraphicsApi::Metal, &config).is_err());
        #[cfg(not(target_os = "windows"))]
        assert!(make_backend(GraphicsApi::Dx12, &config).is_err());
        assert!(not_built_message(GraphicsApi::Dx12).contains("Windows"));
    }
}
