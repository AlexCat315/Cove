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

/// Total length of each cache array: four stage groups of `SLOTS_PER_GROUP`.
const SLOTS: usize = 128;
/// Stride between stage groups. This is the slot bound, not the array length —
/// Swift indexes `group * 32 + slot` into 128-element arrays, so conflating the
/// two made every non-vertex lookup run past the end of the array.
const SLOTS_PER_GROUP: usize = 32;
/// Highest slot index cached per stage. Uncached slots always report a change.
const CACHED_SLOT_LIMIT: u32 = SLOTS_PER_GROUP as u32;

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
        let Some(index) = Self::index(slot, stage) else {
            return true;
        };
        if let Some(previous) = &self.buffers[index]
            && same_object(&previous.resource, resource)
            && previous.offset == offset
        {
            return false;
        }
        self.buffers[index] = Some(BufferValue {
            resource: resource.clone(),
            offset,
        });
        true
    }

    pub fn texture_changed(
        &mut self,
        resource: &Retained<ProtocolObject<dyn MTLTexture>>,
        slot: u32,
        stage: ShaderStage,
    ) -> bool {
        let Some(index) = Self::index(slot, stage) else {
            return true;
        };
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
        let Some(index) = Self::index(slot, stage) else {
            return true;
        };
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
        Some(group * SLOTS_PER_GROUP + slot as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: `index()` used to compute `group * SLOTS + slot` with
    /// `SLOTS = 128`, while each cache array is 128 long. Every non-vertex
    /// binding therefore indexed past the end and panicked with
    /// `index out of bounds: the len is 128 but the index is 129`. Swift uses a
    /// group stride of 32 into the same 128-element arrays.
    #[test]
    fn every_stage_group_indexes_inside_the_cache_arrays() {
        for stage in [
            ShaderStage::Vertex,
            ShaderStage::Fragment,
            ShaderStage::Task,
            ShaderStage::Mesh,
        ] {
            for slot in 0..CACHED_SLOT_LIMIT {
                let index = MetalRenderBindings::index(slot, stage)
                    .unwrap_or_else(|| panic!("slot {slot} in {stage:?} is cacheable"));
                assert!(
                    index < SLOTS,
                    "{stage:?} slot {slot} indexed {index} into a {SLOTS}-element array"
                );
            }
        }
    }

    #[test]
    fn stage_groups_have_disjoint_index_ranges_and_compute_is_uncached() {
        // Slot 0 of each group must land in its own band, and the bands must not
        // overlap — that is what makes the cache per-stage.
        let mut seen = std::collections::BTreeSet::new();
        for stage in [
            ShaderStage::Vertex,
            ShaderStage::Fragment,
            ShaderStage::Task,
            ShaderStage::Mesh,
        ] {
            let index = MetalRenderBindings::index(0, stage).expect("cacheable");
            assert!(seen.insert(index), "{stage:?} reuses another group's index {index}");
        }
        assert_eq!(seen.len(), 4, "four disjoint groups");
        assert!(
            MetalRenderBindings::index(0, ShaderStage::Compute).is_none(),
            "compute has no cached binding slots"
        );
    }

    #[test]
    fn slots_at_or_beyond_the_limit_are_uncached() {
        for slot in [CACHED_SLOT_LIMIT, CACHED_SLOT_LIMIT + 1, 127, 4096] {
            for stage in [ShaderStage::Vertex, ShaderStage::Fragment, ShaderStage::Mesh] {
                assert!(
                    MetalRenderBindings::index(slot, stage).is_none(),
                    "slot {slot} in {stage:?} must stay uncached"
                );
            }
        }
    }

    #[test]
    fn invalidating_every_cacheable_slot_does_not_panic() {
        let mut bindings = MetalRenderBindings::new();
        for stage in [
            ShaderStage::Vertex,
            ShaderStage::Fragment,
            ShaderStage::Task,
            ShaderStage::Mesh,
        ] {
            for slot in 0..CACHED_SLOT_LIMIT {
                bindings.invalidate_buffer(slot, stage);
            }
        }
        for slot in [CACHED_SLOT_LIMIT, 4096] {
            bindings.invalidate_buffer(slot, ShaderStage::Fragment);
        }
    }
}
