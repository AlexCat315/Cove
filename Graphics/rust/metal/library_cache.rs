//! Translation of NativeRHI/Metal/Bindings/MetalLibraryCache.swift.
//!
//! MSL source is compiled at most once per unique source; precompiled
//! `.metallib` data is loaded once per unique blob. Shader modules then look up
//! their `MTLFunction` from the cached library, so the hot path never calls
//! `newLibraryWithSource`.
use std::collections::HashMap;
use std::sync::Mutex;

use dispatch2::DispatchData;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{MTLDevice, MTLLibrary};

use crate::rhi::{RhiError, RhiResult, ShaderFormat, ShaderModuleDescriptor};

/// Both maps sit behind the same lock, mirroring Swift's single `NSLock`: the
/// cache is consulted outside the device render lock, so it must serialise its
/// own mutation.
#[derive(Default)]
struct Libraries {
    source: HashMap<Vec<u8>, Retained<ProtocolObject<dyn MTLLibrary>>>,
    metallib: HashMap<Vec<u8>, Retained<ProtocolObject<dyn MTLLibrary>>>,
}

pub struct MetalLibraryCache {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    libraries: Mutex<Libraries>,
}
impl MetalLibraryCache {
    pub fn new(device: &Retained<ProtocolObject<dyn MTLDevice>>) -> Self {
        Self { device: device.clone(), libraries: Mutex::new(Libraries::default()) }
    }

    /// Returns the compiled/loaded library for a shader module descriptor,
    /// compiling MSL source only on first sight of that source.
    pub fn library(&self, descriptor: &ShaderModuleDescriptor) -> RhiResult<Retained<ProtocolObject<dyn MTLLibrary>>> {
        let mut libraries = self.libraries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match descriptor.format {
            ShaderFormat::MslSource => {
                if let Some(cached) = libraries.source.get(&descriptor.code) {
                    return Ok(cached.clone());
                }
                let source = String::from_utf8_lossy(&descriptor.code);
                match self.device.newLibraryWithSource_options_error(&NSString::from_str(&source), None) {
                    Ok(library) => {
                        libraries.source.insert(descriptor.code.clone(), library.clone());
                        Ok(library)
                    }
                    Err(error) => Err(RhiError::invalid_argument(format!(
                        "MSL compile failed: {}",
                        error.localizedDescription()
                    ))),
                }
            }
            ShaderFormat::Metallib => {
                if let Some(cached) = libraries.metallib.get(&descriptor.code) {
                    return Ok(cached.clone());
                }
                // `newLibraryWithData:` consumes a dispatch data object, not a
                // plain byte slice; the conversion happens on first load only.
                let data = DispatchData::from_bytes(&descriptor.code);
                match self.device.newLibraryWithData_error(&data) {
                    Ok(library) => {
                        libraries.metallib.insert(descriptor.code.clone(), library.clone());
                        Ok(library)
                    }
                    Err(error) => Err(RhiError::invalid_argument(format!(
                        "metallib load failed: {}",
                        error.localizedDescription()
                    ))),
                }
            }
            ShaderFormat::Spirv | ShaderFormat::Dxil => Err(RhiError::unsupported_feature(
                "Metal backend does not import SPIR-V/DXIL shaders",
            )),
        }
    }
}
