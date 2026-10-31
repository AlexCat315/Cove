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
        Dx12LayoutRecord {
            entries: descriptor.entries.clone(),
            root_signature: core::ptr::null_mut(),
        },
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
        let record = registries
            .binding_layouts
            .get(&layout.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))?;
        declarations.extend(record.entries.iter().copied());
    }
    // D3D12 supports a single root signature per pipeline layout; 64 DWORDS is the
    // documented root-parameter budget.
    rhi_require(
        declarations.len() <= 64,
        "pipeline layout declares too many root parameters",
    )?;
    let root_signature = create_root_signature(device, &declarations, &descriptor.push_constants)?;
    registries
        .root_signatures
        .insert(handle.id(), root_signature);
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
            crate::rhi::BindingResource::UniformBuffer {
                buffer,
                offset,
                size,
            } => {
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
        super::resources::Dx12SetRecordRecord {
            layout_id: layout.id(),
            entries: set_entries.to_vec(),
        },
    );
    let _ = record;
    Ok(())
}

/// Releases one binding set. D3D12 descriptors live in the heap, so nothing needs
/// to be freed beyond the bookkeeping.
pub fn unregister_set(registries: &mut Dx12Registries, handle: BindingSet) {
    registries.binding_sets.remove(&handle.id());
}

/// Builds and creates the root signature a pipeline layout needs.
///
/// D3D12 root signatures are serialized from a `D3D12_ROOT_SIGNATURE_DESC` and then
/// created from the blob. The descriptor ranges come from the layout's
/// declarations: one root descriptor per declared slot, which is the simplest
/// mapping that keeps every binding addressable without a descriptor table.
fn create_root_signature(
    device: &Dx12DeviceHandle,
    declarations: &[BindingLayoutEntry],
    push_constants: &[crate::rhi::PushConstantRange],
) -> RhiResult<sys::ComPtr> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, declarations, push_constants);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        // A root descriptor is the widest parameter form and needs no heap, so each
        // declared buffer/texture slot becomes one. Constants occupy b[0] when the
        // layout declares any push range.
        let mut parameters: Vec<sys::D3D12_ROOT_PARAMETER> = Vec::new();
        for declaration in declarations {
            let parameter_type = match declaration.binding_type {
                crate::rhi::BindingType::UniformBuffer => 0u32, // CBV
                crate::rhi::BindingType::StorageBuffer => 1u32, // SRV
                crate::rhi::BindingType::StorageTexture => 2u32, // UAV
                crate::rhi::BindingType::Texture => 1u32,       // SRV
                // A sampler is a descriptor-table entry in D3D12, not a root
                // descriptor, so it is left to the descriptor-table path.
                crate::rhi::BindingType::Sampler => continue,
                crate::rhi::BindingType::AccelerationStructure => 1u32,
            };
            parameters.push(sys::D3D12_ROOT_PARAMETER {
                ParameterType: parameter_type,
                ShaderVisibility: if declaration
                    .visibility
                    .contains(crate::rhi::ShaderVisibility::FRAGMENT)
                {
                    2
                } else {
                    1
                },
                shader_register: declaration.slot,
                register_space: 0,
                // Root-descriptor flags: none.
                payload: [0; 4],
            });
        }
        if !push_constants.is_empty() {
            // 32-bit constants occupy one root parameter of `DWORD` count.
            parameters.push(sys::D3D12_ROOT_PARAMETER {
                ParameterType: 3,
                ShaderVisibility: 1,
                shader_register: 0,
                register_space: 0,
                payload: [1, 0, 0, 0],
            });
        }

        let description = sys::D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: parameters.len() as u32,
            pParameters: parameters.as_ptr(),
            NumStaticSamplers: 0,
            pStaticSamplers: core::ptr::null(),
            Flags: sys::D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };
        let mut blob: sys::ComPtr = core::ptr::null_mut();
        let mut error_blob: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the description and the parameter array it points at outlive the
        // call, and both out-parameters are live locals.
        let status = unsafe {
            let serialize = resolve_serialize_root_signature()?;
            serialize(
                &description,
                sys::D3D12_ROOT_SIGNATURE_VERSION_1,
                &mut blob,
                &mut error_blob,
            )
        };
        if status != sys::S_OK || blob.is_null() {
            // SAFETY: the error blob, when present, is owned by this call.
            unsafe { sys::IUnknownVtbl::release(error_blob) };
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        let mut signature: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the blob is a live `ID3DBlob`; its vtable prefix exposes
        // `GetBufferPointer`/`GetBufferSize` after `IUnknown`.
        let (pointer, length) = unsafe {
            let vtbl = *(blob as *mut *const BlobVtbl);
            (
                ((*vtbl).GetBufferPointer)(blob),
                ((*vtbl).GetBufferSize)(blob),
            )
        };
        // SAFETY: the device is live and the blob buffer stays valid until the blob
        // is released below.
        let status = unsafe {
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateRootSignature)(
                device.device,
                0,
                pointer.cast_const(),
                length,
                &sys::IID_ID3D12_ROOT_SIGNATURE,
                &mut signature,
            )
        };
        // SAFETY: the blob was created above and the signature now owns a copy of
        // its bytes.
        unsafe {
            sys::IUnknownVtbl::release(blob);
            sys::IUnknownVtbl::release(error_blob);
        }
        if status != sys::S_OK || signature.is_null() {
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        Ok(signature)
    }
}

/// `D3D12SerializeRootSignature` from `d3d12.dll`.
#[cfg(target_os = "windows")]
type SerializeRootSignature = unsafe extern "system" fn(
    *const sys::D3D12_ROOT_SIGNATURE_DESC,
    u32,
    *mut sys::ComPtr,
    *mut sys::ComPtr,
) -> sys::HRESULT;

#[cfg(target_os = "windows")]
fn resolve_serialize_root_signature() -> RhiResult<SerializeRootSignature> {
    if let Some(serialize) = SERIALIZE_ROOT_SIGNATURE.get() {
        return Ok(*serialize);
    }
    // SAFETY: the DLL name is NUL-terminated and the symbol is looked up with the
    // signature its type declares.
    let module = unsafe { LoadLibraryA(c"d3d12.dll".as_ptr()) };
    if module.is_null() {
        return Err(RhiError::unsupported_backend("d3d12.dll is unavailable"));
    }
    // SAFETY: the module handle is live.
    let symbol = unsafe { GetProcAddress(module, c"D3D12SerializeRootSignature".as_ptr()) };
    if symbol.is_null() {
        return Err(RhiError::unsupported_backend(
            "D3D12SerializeRootSignature is unavailable",
        ));
    }
    // SAFETY: `D3D12SerializeRootSignature` has the documented signature.
    let serialize: SerializeRootSignature = unsafe { core::mem::transmute_copy(&symbol) };
    let _ = SERIALIZE_ROOT_SIGNATURE.set(serialize);
    Ok(serialize)
}

#[cfg(target_os = "windows")]
static SERIALIZE_ROOT_SIGNATURE: std::sync::OnceLock<SerializeRootSignature> =
    std::sync::OnceLock::new();

/// `ID3DBlob`'s vtable prefix.
#[cfg(target_os = "windows")]
#[repr(C)]
struct BlobVtbl {
    base: sys::IUnknownVtbl,
    GetBufferPointer: unsafe extern "system" fn(sys::ComPtr) -> *mut core::ffi::c_void,
    GetBufferSize: unsafe extern "system" fn(sys::ComPtr) -> usize,
}

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn LoadLibraryA(name: *const i8) -> sys::HINSTANCE;
    fn GetProcAddress(module: sys::HINSTANCE, name: *const i8) -> *mut core::ffi::c_void;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{BindingResource, BindingType, ShaderVisibility};

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
