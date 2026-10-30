//! Translation of NativeRHI/Metal/Bindings/MetalRenderBindings.swift.
//!
//! State belongs to one render encoder. Metal keeps independent buffer,
//! texture and sampler index spaces for each shader stage, so the cache is
//! indexed by `(stage group, slot)` and only the common low slot range is
//! cached; higher slots keep the existing encoder path.
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLBuffer, MTLSamplerState, MTLTexture};

use crate::rhi::ShaderStage;

/// The number of slots cached per stage group.
const SLOTS: usize = 128;
/// Highest slot index cached per stage. Uncached slots always report a change.
const CACHED_SLOT_LIMIT: u32 = 32;

/// Identity comparison for retained Objective-C objects. Swift compares the
/// references (`===`); Objective-C pointer equality is the same relation.
fn same_object<T: ?Sized>(
    left: &Retained<ProtocolObject<T>>,
    right: &Retained<ProtocolObject<T>>,
) -> bool
where
    ProtocolObject<T>: objc2::Message,
{
    std::ptr::eq(Retained::as_ptr(left), Retained::as_ptr(right))
}

#[derive(Clone)]
struct BufferValue {
    resource: Retained<ProtocolObject<dyn MTLBuffer>>,
    offset: usize,
}

pub struct MetalRenderBindings {
    buffers: [Option<BufferValue>; SLOTS],
    textures: [Option<Retained<ProtocolObject<dyn MTLTexture>>>; SLOTS],
    samplers: [Option<Retained<ProtocolObject<dyn MTLSamplerState>>>; SLOTS],
}
impl Default for MetalRenderBindings {
    fn default() -> Self {
        Self::new()
    }
}
impl MetalRenderBindings {
    pub fn new() -> Self {
        Self {
            buffers: std::array::from_fn(|_| None),
            textures: std::array::from_fn(|_| None),
            samplers: std::array::from_fn(|_| None),
        }
    }

    /// Returns whether the encoder must rebind this buffer. The first binding
    /// of an uncached slot always reports a change so it is never skipped.
    pub fn buffer_changed(
        &mut self,
        resource: &Retained<ProtocolObject<dyn MTLBuffer>>,
        offset: usize,
        slot: u32,
        stage: ShaderStage,
    ) -> bool {
        let Some(index) = Self::index(slot, stage) else { return true };
        if let Some(previous) = &self.buffers[index]
            && same_object(&previous.resource, resource)
            && previous.offset == offset
        {
            return false;
        }
        self.buffers[index] = Some(BufferValue { resource: resource.clone(), offset });
        true
    }

    pub fn texture_changed(
        &mut self,
        resource: &Retained<ProtocolObject<dyn MTLTexture>>,
        slot: u32,
        stage: ShaderStage,
    ) -> bool {
        let Some(index) = Self::index(slot, stage) else { return true };
        if let Some(previous) = &self.textures[index]
            && same_object(previous, resource)
        {
            return false;
        }
        self.textures[index] = Some(resource.clone());
        true
    }

    pub fn sampler_changed(
        &mut self,
        resource: &Retained<ProtocolObject<dyn MTLSamplerState>>,
        slot: u32,
        stage: ShaderStage,
    ) -> bool {
        let Some(index) = Self::index(slot, stage) else { return true };
        if let Some(previous) = &self.samplers[index]
            && same_object(previous, resource)
        {
            return false;
        }
        self.samplers[index] = Some(resource.clone());
        true
    }

    pub fn invalidate_buffer(&mut self, slot: u32, stage: ShaderStage) {
        if let Some(index) = Self::index(slot, stage) {
            self.buffers[index] = None;
        }
    }

    fn index(slot: u32, stage: ShaderStage) -> Option<usize> {
        // Cache the common index range without restricting higher texture
        // slots. Uncached slots keep the existing encoder path.
        if slot >= CACHED_SLOT_LIMIT {
            return None;
        }
        let group = match stage {
            ShaderStage::Vertex => 0,
            ShaderStage::Fragment => 1,
            ShaderStage::Task => 2,
            ShaderStage::Mesh => 3,
            ShaderStage::Compute => return None,
        };
        Some(group * SLOTS + slot as usize)
    }
}
