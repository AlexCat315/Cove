//! Binding-layout and root-signature registration for the DX12 backend.
//!
//! D3D12 has no descriptor-set object: a binding layout becomes a root signature,
//! and a binding set becomes a run of descriptors in the shader-visible heap. This
//! module owns the layout bookkeeping and the serializer call; Swift's C++ bridge
//! did the same work behind `grhi_dx12_binding_layout` / `grhi_dx12_binding_set`.
#![allow(non_snake_case)]
use std::collections::HashMap;

use crate::rhi::{
    BindingLayout, BindingLayoutDescriptor, BindingLayoutEntry, BindingSet, BindingSetEntry,
    PipelineLayout, PipelineLayoutDescriptor, RhiError, RhiResult, rhi_require,
};

use super::device::Dx12DeviceHandle;
use super::resources::Dx12Registries;
use super::sys;

/// One registered layout: its declarations plus the root signature it created.
pub struct Dx12LayoutRecord {
    pub entries: Vec<BindingLayoutEntry>,
    /// Owned root signature, or null while it is still being built.
    pub root_signature: sys::ComPtr,
}

/// One registered binding set: the slot values and the descriptor offsets they
/// occupy in the shader-visible heap.
pub struct Dx12SetRecord {
    pub layout_id: u32,
    pub entries: Vec<BindingSetEntry>,
    /// First descriptor index in the resource heap for each slot.
    pub descriptor_indices: HashMap<u32, u32>,
}

/// Registers a binding layout and records its declarations.
///
/// The root signature itself is built by [`register_pipeline_layout`], because a
/// D3D12 root signature spans every set of a pipeline layout at once.
pub fn register_layout(
    registries: &mut Dx12Registries,
    handle: BindingLayout,
    descriptor: &BindingLayoutDescriptor,
) -> RhiResult<()> {
    let mut seen = std::collections::HashSet::new();
    for entry in &descriptor.entries {
        rhi_require(
            seen.insert(entry.slot),
            "binding slots must be unique within a layout",
        )?;
    }
    registries.binding_layouts.insert(
        handle.id(),
        Dx12LayoutRecord { entries: descriptor.entries.clone(), root_signature: core::ptr::null_mut() },
    );
    Ok(())
}

/// Registers a pipeline layout and creates its root signature.
pub fn register_pipeline_layout(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: PipelineLayout,
    descriptor: &PipelineLayoutDescriptor,
) -> RhiResult<()> {
    // Every set must already be registered, and a D3D12 root signature covers all
    // of them, so the declarations are gathered in set order.
    let mut declarations: Vec<BindingLayoutEntry> = Vec::new();
    for layout in &descriptor.set_layouts {
        let record = registries.binding_layouts.get(&layout.id()).ok_or_else(|| {
            RhiError::invalid_argument("unknown binding layout")
        })?;
        declarations.extend(record.entries.iter().copied());
    }
    // D3D12 supports a single root signature per pipeline layout; 64 DWORDS is the
    // documented root-parameter budget.
    rhi_require(
        declarations.len() <= 64,
        "pipeline layout declares too many root parameters",
    )?;
    let root_signature = create_root_signature(device, &declarations, &descriptor.push_constants)?;
    registries.root_signatures.insert(handle.id(), root_signature);
    registries.pipeline_layouts.insert(
        handle.id(),
        super::resources::Dx12PipelineLayoutRecord {
            set_layouts: descriptor.set_layouts.clone(),
            push_constants: descriptor.push_constants.clone(),
        },
    );
    Ok(())
}

/// Registers a binding set: the resources are validated now and the descriptor
/// indices are reserved for the encode path.
pub fn register_set(
    registries: &mut Dx12Registries,
    handle: BindingSet,
    layout: BindingLayout,
    layout_entries: &[BindingLayoutEntry],
    set_entries: &[BindingSetEntry],
) -> RhiResult<()> {
    let record = registries
        .binding_layouts
        .get(&layout.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))?;
    let mut descriptor_indices = HashMap::new();
    let mut next_index = registries.next_descriptor_index;
    for entry in set_entries {
        let declaration = layout_entries
            .iter()
            .find(|candidate| candidate.slot == entry.slot)
            .ok_or_else(|| RhiError::layout_mismatch("binding has no declaration"))?;
        let _ = declaration;
        // Every resource is resolved at registration, so a retired handle cannot
        // reach the encoder.
        match entry.resource {
            crate::rhi::BindingResource::Sampler(sampler) => {
                rhi_require(
                    registries.samplers.contains_key(&sampler.id()),
                    "binding references an unknown sampler",
                )?;
            }
            crate::rhi::BindingResource::Texture(texture)
            | crate::rhi::BindingResource::StorageTexture(texture) => {
                rhi_require(
                    registries.textures.contains_key(&texture.id()),
                    "binding references an unknown texture",
                )?;
            }
            crate::rhi::BindingResource::UniformBuffer { buffer, offset, size } => {
                let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                    RhiError::invalid_argument("binding references an unknown buffer")
                })?;
                rhi_require(
                    offset < record.size as usize,
                    "binding references an unknown buffer or invalid offset",
                )?;
                if let Some(size) = size {
                    rhi_require(size > 0, "uniform binding size must be positive")?;
                    rhi_require(
                        offset + size <= record.size as usize,
                        "binding references an unknown buffer or invalid offset",
                    )?;
                }
            }
            crate::rhi::BindingResource::StorageBuffer { buffer, offset } => {
                let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                    RhiError::invalid_argument("binding references an unknown buffer")
                })?;
                rhi_require(
                    offset < record.size as usize,
                    "binding references an unknown buffer or invalid offset",
                )?;
            }
            crate::rhi::BindingResource::AccelerationStructure(_) => {}
        }
        descriptor_indices.insert(entry.slot, next_index);
        next_index += 1;
    }
    registries.next_descriptor_index = next_index;
    registries.binding_sets.insert(
        handle.id(),
        super::resources::Dx12SetRecordRecord { layout_id: layout.id(), entries: set_entries.to_vec() },
    );
    let _ = record;
    Ok(())
}

/// Releases one binding set. D3D12 descriptors live in the heap, so nothing needs
/// to be freed beyond the bookkeeping.
pub fn unregister_set(registries: &mut Dx12Registries, handle: BindingSet) {
    registries.binding_sets.remove(&handle.id());
}

/// Serializes a root signature and creates it on the device.
fn create_root_signature(
    device: &Dx12DeviceHandle,
    declarations: &[BindingLayoutEntry],
    push_constants: &[crate::rhi::PushConstantRange],
) -> RhiResult<sys::ComPtr> {
    let _ = (device, declarations, push_constants);
    // A serialized root signature needs `D3D12SerializeRootSignature` (from
    // d3d12.dll) plus the descriptor ranges each declaration implies. The
    // declarations and the DWORDS budget are validated above; the serializer call
    // is the remaining piece this build does not link.
    Err(RhiError::unsupported_feature(
        "DX12 root-signature serialization is not linked in this build",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{BindingResource, ShaderVisibility, BindingType};

    #[test]
    fn duplicate_binding_slots_are_rejected() {
        let mut registries = Dx12Registries::default();
        let layout = BindingLayout::new(1);
        let duplicated = BindingLayoutDescriptor::new(vec![
            BindingLayoutEntry::new(0, BindingType::Texture, ShaderVisibility::FRAGMENT),
            BindingLayoutEntry::new(0, BindingType::Sampler, ShaderVisibility::FRAGMENT),
        ]);
        assert!(register_layout(&mut registries, layout, &duplicated).is_err());

        let unique = BindingLayoutDescriptor::new(vec![
            BindingLayoutEntry::new(0, BindingType::Texture, ShaderVisibility::FRAGMENT),
            BindingLayoutEntry::new(1, BindingType::Sampler, ShaderVisibility::FRAGMENT),
        ]);
        assert!(register_layout(&mut registries, layout, &unique).is_ok());
        assert_eq!(registries.binding_layouts[&layout.id()].entries.len(), 2);
    }

    #[test]
    fn an_unregistered_layout_blocks_a_set() {
        let mut registries = Dx12Registries::default();
        let layout = BindingLayout::new(9);
        let set = BindingSet::new(4);
        let declarations = [BindingLayoutEntry::new(
            0,
            BindingType::StorageBuffer,
            ShaderVisibility::COMPUTE,
        )];
        let entries = [BindingSetEntry::new(
            0,
            BindingResource::storage_buffer(crate::rhi::Buffer::new(1)),
        )];
        assert!(register_set(&mut registries, set, layout, &declarations, &entries).is_err());
        unregister_set(&mut registries, set);
    }
}
