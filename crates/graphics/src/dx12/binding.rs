//! Binding-layout and root-signature registration for the DX12 backend.
//!
//! D3D12 has no descriptor-set object: a pipeline layout owns one root signature,
//! and a binding set owns a run of descriptors in the shader-visible heaps. This
//! module builds the root signature, plans the descriptor writes and records the
//! resulting heap ranges for the encoder.
#![allow(non_snake_case)]
// The root-parameter builder is shared with the platform-independent unit
// tests; off Windows the device-gated serializer is the only caller that is
// compiled out.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use std::collections::HashMap;

use crate::rhi::{
    BindingLayout, BindingLayoutDescriptor, BindingLayoutEntry, BindingSet, BindingSetEntry,
    BindingType, PipelineLayout, PipelineLayoutDescriptor, PushConstantRange, RhiError, RhiResult,
    ShaderStage, ShaderVisibility, rhi_require,
};

use super::device::{Dx12DeviceHandle, RESOURCE_HEAP_CAPACITY, SAMPLER_HEAP_CAPACITY};
use super::resources::{Dx12PipelineLayoutRecord, Dx12Registries, Dx12SetRecord};
use super::sys;

/// One registered binding layout: its declarations.
pub struct Dx12LayoutRecord {
    pub entries: Vec<BindingLayoutEntry>,
}

/// Registers a binding layout and records its declarations.
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
        rhi_require(
            entry.array_size == 1,
            "DX12 does not support binding arrays yet",
        )?;
    }
    registries.binding_layouts.insert(
        handle.id(),
        Dx12LayoutRecord {
            entries: descriptor.entries.clone(),
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
    // Gather every set's declarations so the root signature can cover them. The
    // registered declarations are authoritative: the encoder relies on the same
    // ordering when it writes descriptors into the heaps.
    let mut set_entries: Vec<Vec<BindingLayoutEntry>> = Vec::new();
    for layout in &descriptor.set_layouts {
        let record = registries
            .binding_layouts
            .get(&layout.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))?;
        set_entries.push(record.entries.clone());
    }

    let root_signature = create_root_signature(device, &set_entries, &descriptor.push_constants)?;
    // A re-registration replaces the old signature rather than leaking it.
    if let Some(previous) = registries
        .root_signatures
        .insert(handle.id(), root_signature.0)
    {
        // SAFETY: the pointer was created by this backend and is owned by the map.
        unsafe { sys::IUnknownVtbl::release(previous) };
    }
    registries.pipeline_layouts.insert(
        handle.id(),
        Dx12PipelineLayoutRecord {
            set_layouts: descriptor.set_layouts.clone(),
            push_constants: descriptor.push_constants.clone(),
            resource_table_root_index: root_signature.1.resource_table_root_index,
            sampler_table_root_index: root_signature.1.sampler_table_root_index,
            buffer_root_index: root_signature.1.buffer_root_index,
            push_constant_root_index: root_signature.1.push_constant_root_index,
        },
    );
    Ok(())
}

/// Root-parameter mapping produced while building a root signature.
pub struct RootMapping {
    /// Per-set descriptor-table root parameter for CBV/SRV/UAV, if the set has any.
    pub resource_table_root_index: Vec<Option<u32>>,
    /// Per-set descriptor-table root parameter for samplers, if the set has any.
    pub sampler_table_root_index: Vec<Option<u32>>,
    /// Maps `(set_index, binding_slot)` for buffer bindings to their root parameter.
    pub buffer_root_index: HashMap<(u32, u32), u32>,
    /// Maps a push-constant slot to its root-constants parameter.
    pub push_constant_root_index: HashMap<u32, u32>,
}

/// One descriptor the backend must write into a shader-visible heap.
enum DescriptorWrite {
    #[cfg(target_os = "windows")]
    Acceleration {
        index: u32,
        address: u64,
    },
    Sampler {
        index: u32,
        description: sys::D3D12_SAMPLER_DESC,
    },
    Srv {
        index: u32,
        resource: sys::ComPtr,
    },
    Uav {
        index: u32,
        resource: sys::ComPtr,
    },
}

/// The plan for one binding set: heap ranges plus the descriptors to write.
struct SetPlan {
    resource_base: u32,
    resource_count: u32,
    sampler_base: u32,
    sampler_count: u32,
    writes: Vec<DescriptorWrite>,
}

/// Validates a set and reserves contiguous heap ranges for its descriptors.
///
/// This is deliberately device-independent so the bookkeeping can be tested on
/// every host. Nothing is committed to the registries until the descriptors have
/// actually been written by [`register_set`].
fn plan_set(
    registries: &Dx12Registries,
    layout: BindingLayout,
    layout_entries: &[BindingLayoutEntry],
    set_entries: &[BindingSetEntry],
) -> RhiResult<SetPlan> {
    let declarations = registries
        .binding_layouts
        .get(&layout.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))?
        .entries
        .clone();
    rhi_require(
        layout_entries.len() == declarations.len(),
        "binding set layout does not match the registered layout",
    )?;
    for entry in set_entries {
        rhi_require(
            declarations
                .iter()
                .any(|candidate| candidate.slot == entry.slot),
            "binding has no declaration",
        )?;
    }

    let resource_count = declarations
        .iter()
        .filter(|entry| {
            matches!(
                entry.binding_type,
                BindingType::Texture
                    | BindingType::StorageTexture
                    | BindingType::AccelerationStructure
            )
        })
        .count() as u32;
    let sampler_count = declarations
        .iter()
        .filter(|entry| entry.binding_type == BindingType::Sampler)
        .count() as u32;
    let sets = &registries.binding_sets;
    let mut plan = SetPlan {
        resource_base: free_descriptor_range(
            RESOURCE_HEAP_CAPACITY,
            resource_count,
            sets.values()
                .map(|set| (set.resource_heap_base, set.resource_heap_count)),
        )?,
        resource_count: 0,
        sampler_base: free_descriptor_range(
            SAMPLER_HEAP_CAPACITY,
            sampler_count,
            sets.values()
                .map(|set| (set.sampler_heap_base, set.sampler_heap_count)),
        )?,
        sampler_count: 0,
        writes: Vec::new(),
    };
    // Walk the *declarations* in order: the root signature's ranges were built in
    // this same order with append offsets, so the heap slots must line up.
    for declaration in &declarations {
        let entry = set_entries
            .iter()
            .find(|candidate| candidate.slot == declaration.slot)
            .ok_or_else(|| RhiError::layout_mismatch("binding has no declaration"))?;
        match declaration.binding_type {
            BindingType::Sampler => {
                let description = match entry.resource {
                    crate::rhi::BindingResource::Sampler(sampler) => registries
                        .samplers
                        .get(&sampler.id())
                        .copied()
                        .ok_or_else(|| {
                            RhiError::invalid_argument("binding references an unknown sampler")
                        })?,
                    _ => return Err(RhiError::layout_mismatch("sampler slot has no sampler")),
                };
                plan.writes.push(DescriptorWrite::Sampler {
                    index: plan.sampler_base + plan.sampler_count,
                    description,
                });
                plan.sampler_count += 1;
            }
            BindingType::Texture | BindingType::StorageTexture => {
                let texture = match entry.resource {
                    crate::rhi::BindingResource::Texture(texture)
                    | crate::rhi::BindingResource::StorageTexture(texture) => texture,
                    _ => return Err(RhiError::layout_mismatch("texture slot has no texture")),
                };
                let record = registries.textures.get(&texture.id()).ok_or_else(|| {
                    RhiError::invalid_argument("binding references an unknown texture")
                })?;
                let index = plan.resource_base + plan.resource_count;
                if declaration.binding_type == BindingType::StorageTexture {
                    plan.writes.push(DescriptorWrite::Uav {
                        index,
                        resource: record.resource,
                    });
                } else {
                    plan.writes.push(DescriptorWrite::Srv {
                        index,
                        resource: record.resource,
                    });
                }
                plan.resource_count += 1;
            }
            BindingType::UniformBuffer | BindingType::StorageBuffer => {
                // Buffers are root descriptors, bound at encode time from the
                // resource's GPU address; they need no descriptor in a heap.
                validate_buffer_binding(registries, entry, declaration)?;
            }
            BindingType::AccelerationStructure => {
                #[cfg(target_os = "windows")]
                {
                    let crate::rhi::BindingResource::AccelerationStructure(handle) = entry.resource
                    else {
                        return Err(RhiError::layout_mismatch(
                            "DXR slot has no acceleration structure",
                        ));
                    };
                    let record = registries
                        .accelerations
                        .get(&handle.id())
                        .filter(|r| r.is_top_level())
                        .ok_or_else(|| {
                            RhiError::invalid_argument(
                                "DXR shader binding requires a live top-level structure",
                            )
                        })?;
                    plan.writes.push(DescriptorWrite::Acceleration {
                        index: plan.resource_base + plan.resource_count,
                        address: record.address(),
                    });
                    plan.resource_count += 1;
                }
                #[cfg(not(target_os = "windows"))]
                return Err(RhiError::unsupported_feature(
                    "DXR is only available on Windows",
                ));
            }
        }
    }
    Ok(plan)
}

fn free_descriptor_range(
    capacity: u32,
    count: u32,
    occupied: impl Iterator<Item = (u32, u32)>,
) -> RhiResult<u32> {
    if count == 0 {
        return Ok(0);
    }
    let mut ranges: Vec<_> = occupied.filter(|&(_, length)| length != 0).collect();
    ranges.sort_unstable();
    let mut base = 0;
    for (start, length) in ranges {
        if start - base >= count {
            return Ok(base);
        }
        base = start + length;
    }
    if capacity - base >= count {
        Ok(base)
    } else {
        Err(RhiError::OutOfMemory)
    }
}

/// Checks a buffer binding against the registered buffer record.
fn validate_buffer_binding(
    registries: &Dx12Registries,
    entry: &BindingSetEntry,
    declaration: &BindingLayoutEntry,
) -> RhiResult<()> {
    let (buffer, offset, size) = match entry.resource {
        crate::rhi::BindingResource::UniformBuffer {
            buffer,
            offset,
            size,
        } => {
            rhi_require(
                declaration.binding_type == BindingType::UniformBuffer,
                "buffer slot has no uniform buffer",
            )?;
            (buffer, offset, size)
        }
        crate::rhi::BindingResource::StorageBuffer { buffer, offset } => {
            rhi_require(
                declaration.binding_type == BindingType::StorageBuffer,
                "buffer slot has no storage buffer",
            )?;
            (buffer, offset, None)
        }
        _ => return Err(RhiError::layout_mismatch("buffer slot has no buffer")),
    };
    let record = registries
        .buffers
        .get(&buffer.id())
        .ok_or_else(|| RhiError::invalid_argument("binding references an unknown buffer"))?;
    rhi_require(
        offset < record.size as usize,
        "binding references an unknown buffer or invalid offset",
    )?;
    if let Some(size) = size {
        rhi_require(size > 0, "uniform binding size must be positive")?;
        rhi_require(
            offset.saturating_add(size) <= record.size as usize,
            "binding references an unknown buffer or invalid offset",
        )?;
    }
    Ok(())
}

/// Registers a binding set: resources are validated and texture/sampler
/// descriptors are written into the shader-visible heaps.
pub fn register_set(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: BindingSet,
    layout: BindingLayout,
    layout_entries: &[BindingLayoutEntry],
    set_entries: &[BindingSetEntry],
) -> RhiResult<()> {
    let plan = plan_set(registries, layout, layout_entries, set_entries)?;
    for write in &plan.writes {
        match *write {
            #[cfg(target_os = "windows")]
            DescriptorWrite::Acceleration { index, address } => {
                let cpu = super::texture::resource_heap_cpu(device, index)?;
                super::acceleration::write_srv(device.device, cpu, address);
            }
            DescriptorWrite::Sampler { index, description } => {
                let cpu = super::texture::sampler_heap_cpu(device, index)?;
                super::texture::write_sampler_at(device, &description, cpu);
            }
            DescriptorWrite::Srv { index, resource } => {
                let cpu = super::texture::resource_heap_cpu(device, index)?;
                super::texture::create_texture_srv(device, resource, cpu);
            }
            DescriptorWrite::Uav { index, resource } => {
                let cpu = super::texture::resource_heap_cpu(device, index)?;
                super::texture::create_texture_uav(device, resource, cpu);
            }
        }
    }
    registries.binding_sets.insert(
        handle.id(),
        Dx12SetRecord {
            layout_id: layout.id(),
            entries: set_entries.to_vec(),
            resource_heap_base: plan.resource_base,
            resource_heap_count: plan.resource_count,
            sampler_heap_base: plan.sampler_base,
            sampler_heap_count: plan.sampler_count,
        },
    );
    Ok(())
}

/// Releases the descriptor ranges after the frontend retires the binding set.
pub fn unregister_set(registries: &mut Dx12Registries, handle: BindingSet) {
    registries.binding_sets.remove(&handle.id());
}

/// Builds the root signature a pipeline layout needs.
///
/// The layout is mapped as follows:
/// - each buffer binding becomes a root descriptor (CBV, SRV or UAV);
/// - each set's texture/storage resources become one CBV/SRV/UAV descriptor table;
/// - each set's samplers become one sampler descriptor table;
/// - each push-constant range becomes one 32-bit root-constants parameter.
fn create_root_signature(
    device: &Dx12DeviceHandle,
    sets: &[Vec<BindingLayoutEntry>],
    push_constants: &[PushConstantRange],
) -> RhiResult<(sys::ComPtr, RootMapping)> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, sets, push_constants);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        // `_range_storage` owns the buffers `parameters` points at; it must stay
        // alive until the serializer returns.
        let (parameters, _range_storage, mapping) = build_root_parameters(sets, push_constants)?;
        let description = sys::D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: parameters.len() as u32,
            pParameters: parameters.as_ptr(),
            NumStaticSamplers: 0,
            pStaticSamplers: core::ptr::null(),
            Flags: sys::D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };
        let mut blob: sys::ComPtr = core::ptr::null_mut();
        let mut error_blob: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the description and both out-parameters are live; the ranges
        // pointed at by `parameters` stay in `range_storage` for the whole call.
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
            // SAFETY: an error blob, when present, is owned by this call.
            unsafe { sys::IUnknownVtbl::release(error_blob) };
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        let mut signature: sys::ComPtr = core::ptr::null_mut();
        let (pointer, length) = {
            // SAFETY: `blob` is a live ID3DBlob; these are its first two methods
            // after IUnknown and return the serialized bytes.
            unsafe {
                let vtbl = *(blob as *mut *const BlobVtbl);
                (
                    ((*vtbl).GetBufferPointer)(blob),
                    ((*vtbl).GetBufferSize)(blob),
                )
            }
        };
        // SAFETY: the device is live and the blob's bytes outlive the call.
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
        // SAFETY: both blobs were created above and are no longer needed.
        unsafe {
            sys::IUnknownVtbl::release(blob);
            sys::IUnknownVtbl::release(error_blob);
        }
        if status != sys::S_OK || signature.is_null() {
            return Err(RhiError::invalid_argument(sys::hresult_message(status)));
        }
        Ok((signature, mapping))
    }
}

/// Builds the root parameters and keeps the descriptor-range storage they point
/// at alive. A pure function, so its layout and budget can be verified without a
/// GPU device.
fn build_root_parameters(
    sets: &[Vec<BindingLayoutEntry>],
    push_constants: &[PushConstantRange],
) -> RhiResult<(
    Vec<sys::D3D12_ROOT_PARAMETER>,
    Vec<Vec<sys::D3D12_DESCRIPTOR_RANGE>>,
    RootMapping,
)> {
    let mut parameters: Vec<sys::D3D12_ROOT_PARAMETER> = Vec::new();
    let mut range_storage: Vec<Vec<sys::D3D12_DESCRIPTOR_RANGE>> = Vec::new();
    let mut dwords: u32 = 0;
    let mut mapping = RootMapping {
        resource_table_root_index: vec![None; sets.len()],
        sampler_table_root_index: vec![None; sets.len()],
        buffer_root_index: HashMap::new(),
        push_constant_root_index: HashMap::new(),
    };

    for (set_index, entries) in sets.iter().enumerate() {
        let set_index = set_index as u32;
        let mut resource_ranges: Vec<sys::D3D12_DESCRIPTOR_RANGE> = Vec::new();
        let mut sampler_ranges: Vec<sys::D3D12_DESCRIPTOR_RANGE> = Vec::new();

        for entry in entries {
            rhi_require(
                entry.array_size == 1,
                "DX12 does not support binding arrays yet",
            )?;
            match entry.binding_type {
                BindingType::UniformBuffer | BindingType::StorageBuffer => {
                    let parameter_type = if entry.binding_type == BindingType::UniformBuffer {
                        sys::D3D12_ROOT_PARAMETER_TYPE_CBV
                    } else if entry.buffer.read_only {
                        sys::D3D12_ROOT_PARAMETER_TYPE_SRV
                    } else {
                        sys::D3D12_ROOT_PARAMETER_TYPE_UAV
                    };
                    mapping
                        .buffer_root_index
                        .insert((set_index, entry.slot), parameters.len() as u32);
                    parameters.push(root_descriptor(
                        parameter_type,
                        entry.slot,
                        visibility_to_d3d12(entry.visibility),
                    ));
                    // A root descriptor occupies two DWORDs (the GPU VA).
                    dwords += 2;
                }
                BindingType::Texture
                | BindingType::StorageTexture
                | BindingType::AccelerationStructure => {
                    resource_ranges.push(range_for(
                        entry,
                        if entry.binding_type == BindingType::StorageTexture {
                            sys::D3D12_DESCRIPTOR_RANGE_TYPE_UAV
                        } else {
                            sys::D3D12_DESCRIPTOR_RANGE_TYPE_SRV
                        },
                    ));
                }
                BindingType::Sampler => {
                    sampler_ranges.push(range_for(entry, sys::D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER));
                }
            }
        }

        if !resource_ranges.is_empty() {
            mapping.resource_table_root_index[set_index as usize] = Some(parameters.len() as u32);
            dwords += 1;
            range_storage.push(resource_ranges);
            // The outer Vec may reallocate, but the inner Vec's heap buffer — the
            // address the driver actually reads — stays put.
            let ranges = range_storage.last().expect("just pushed");
            parameters.push(descriptor_table(
                ranges.len() as u32,
                ranges.as_ptr(),
                sys::D3D12_SHADER_VISIBILITY_ALL,
            ));
        }
        if !sampler_ranges.is_empty() {
            mapping.sampler_table_root_index[set_index as usize] = Some(parameters.len() as u32);
            dwords += 1;
            range_storage.push(sampler_ranges);
            let ranges = range_storage.last().expect("just pushed");
            parameters.push(descriptor_table(
                ranges.len() as u32,
                ranges.as_ptr(),
                sys::D3D12_SHADER_VISIBILITY_ALL,
            ));
        }
    }

    for range in push_constants {
        rhi_require(
            range.byte_count > 0 && range.byte_count % 4 == 0,
            "push-constant byte count must be a positive multiple of four",
        )?;
        let values = (range.byte_count / 4) as u32;
        mapping
            .push_constant_root_index
            .insert(range.slot, parameters.len() as u32);
        parameters.push(sys::D3D12_ROOT_PARAMETER {
            ParameterType: sys::D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: sys::D3D12_ROOT_PARAMETER_UNION {
                Constants: sys::D3D12_ROOT_CONSTANTS {
                    ShaderRegister: range.slot,
                    RegisterSpace: 0,
                    Num32BitValues: values,
                },
            },
            ShaderVisibility: stage_to_d3d12_visibility(range.stage),
        });
        dwords += values;
    }
    rhi_require(
        dwords <= sys::D3D12_MAX_ROOT_SIGNATURE_DWORDS,
        "pipeline layout exceeds the 64-DWORD root-signature budget",
    )?;
    Ok((parameters, range_storage, mapping))
}

/// One root descriptor (a buffer's GPU virtual address).
///
/// The project's Slang/DXIL shaders declare every binding in `space0` and the
/// frontend rejects reflected layouts from any other space, so the set index is
/// *not* a register space: it only selects the root parameter at encode time.
fn root_descriptor(parameter_type: u32, slot: u32, visibility: u32) -> sys::D3D12_ROOT_PARAMETER {
    sys::D3D12_ROOT_PARAMETER {
        ParameterType: parameter_type,
        Anonymous: sys::D3D12_ROOT_PARAMETER_UNION {
            Descriptor: sys::D3D12_ROOT_DESCRIPTOR {
                ShaderRegister: slot,
                RegisterSpace: 0,
            },
        },
        ShaderVisibility: visibility,
    }
}

/// One descriptor table root parameter.
fn descriptor_table(
    count: u32,
    ranges: *const sys::D3D12_DESCRIPTOR_RANGE,
    visibility: u32,
) -> sys::D3D12_ROOT_PARAMETER {
    sys::D3D12_ROOT_PARAMETER {
        ParameterType: sys::D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        Anonymous: sys::D3D12_ROOT_PARAMETER_UNION {
            DescriptorTable: sys::D3D12_ROOT_DESCRIPTOR_TABLE {
                NumDescriptorRanges: count,
                pDescriptorRanges: ranges,
            },
        },
        ShaderVisibility: visibility,
    }
}

/// One contiguous descriptor range. `APPEND` places ranges back-to-back in the
/// table, which is the order [`plan_set`] writes descriptors into the heap.
fn range_for(entry: &BindingLayoutEntry, range_type: u32) -> sys::D3D12_DESCRIPTOR_RANGE {
    // `space0` is the shader ABI (see `root_descriptor`).
    sys::D3D12_DESCRIPTOR_RANGE {
        RangeType: range_type,
        NumDescriptors: entry.array_size.max(1),
        BaseShaderRegister: entry.slot,
        RegisterSpace: 0,
        OffsetInDescriptorsFromTableStart: sys::D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
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
    // SAFETY: the name is NUL-terminated and the symbol has the documented signature.
    let module = unsafe { LoadLibraryA(c"d3d12.dll".as_ptr()) };
    if module.is_null() {
        return Err(RhiError::unsupported_backend("d3d12.dll is unavailable"));
    }
    // SAFETY: the module is live and the export name is NUL-terminated.
    let symbol = unsafe { GetProcAddress(module, c"D3D12SerializeRootSignature".as_ptr()) };
    if symbol.is_null() {
        return Err(RhiError::unsupported_backend(
            "D3D12SerializeRootSignature is unavailable",
        ));
    }
    // SAFETY: the export really is a function of this signature.
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

/// D3D12 has no compute-only visibility, so a compute binding (or any
/// multi-stage visibility the enum cannot express) is promoted to `ALL`.
fn visibility_to_d3d12(visibility: ShaderVisibility) -> u32 {
    match visibility.stages().as_slice() {
        [ShaderStage::Vertex] => sys::D3D12_SHADER_VISIBILITY_VERTEX,
        [ShaderStage::Fragment] => sys::D3D12_SHADER_VISIBILITY_PIXEL,
        [ShaderStage::Mesh] => sys::D3D12_SHADER_VISIBILITY_MESH,
        [ShaderStage::Task] => sys::D3D12_SHADER_VISIBILITY_AMPLIFICATION,
        _ => sys::D3D12_SHADER_VISIBILITY_ALL,
    }
}

fn stage_to_d3d12_visibility(stage: ShaderStage) -> u32 {
    match stage {
        ShaderStage::Vertex => sys::D3D12_SHADER_VISIBILITY_VERTEX,
        ShaderStage::Fragment => sys::D3D12_SHADER_VISIBILITY_PIXEL,
        ShaderStage::Task => sys::D3D12_SHADER_VISIBILITY_AMPLIFICATION,
        ShaderStage::Mesh => sys::D3D12_SHADER_VISIBILITY_MESH,
        ShaderStage::Compute => sys::D3D12_SHADER_VISIBILITY_ALL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::BufferUsage;
    use crate::rhi::{
        BindingResource, Buffer, Sampler, Texture, TextureDimension, TextureFormat, TextureUsage,
    };

    fn sampler_record() -> sys::D3D12_SAMPLER_DESC {
        sys::D3D12_SAMPLER_DESC::default()
    }

    fn texture_record() -> super::super::resources::Dx12TextureRecord {
        super::super::resources::Dx12TextureRecord {
            state: sys::D3D12_RESOURCE_STATE_COMMON,
            resource: core::ptr::null_mut(),
            format: TextureFormat::Rgba8Unorm,
            width: 4,
            height: 4,
            depth: 1,
            layers: 1,
            mip_levels: 1,
            sample_count: 1,
            dimension: TextureDimension::Texture2D,
            usage: TextureUsage::SAMPLED,
            attachment_index: None,
            rtv: None,
            dsv: None,
        }
    }

    fn buffer_record() -> super::super::resources::Dx12ResourceRecord {
        super::super::resources::Dx12ResourceRecord {
            heap_type: sys::D3D12_HEAP_TYPE_DEFAULT,
            resource: core::ptr::null_mut(),
            size: 256,
            gpu_address: 0,
            mapped: core::ptr::null_mut(),
            usage: BufferUsage::UNIFORM,
        }
    }

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
        let registries = Dx12Registries::default();
        let layout = BindingLayout::new(9);
        let declarations = [BindingLayoutEntry::new(
            0,
            BindingType::StorageBuffer,
            ShaderVisibility::COMPUTE,
        )];
        let entries = [BindingSetEntry::new(
            0,
            BindingResource::storage_buffer(Buffer::new(1)),
        )];
        assert!(plan_set(&registries, layout, &declarations, &entries).is_err());
    }

    #[test]
    fn retired_descriptor_ranges_are_reused_without_overlapping_live_sets() {
        let mut registries = Dx12Registries::default();
        let layout = BindingLayout::new(3);
        let declarations = vec![
            BindingLayoutEntry::new(0, BindingType::Texture, ShaderVisibility::FRAGMENT),
            BindingLayoutEntry::new(1, BindingType::Sampler, ShaderVisibility::FRAGMENT),
            BindingLayoutEntry::new(2, BindingType::UniformBuffer, ShaderVisibility::VERTEX),
        ];
        register_layout(
            &mut registries,
            layout,
            &BindingLayoutDescriptor::new(declarations.clone()),
        )
        .unwrap();
        registries.textures.insert(7, texture_record());
        registries.samplers.insert(8, sampler_record());
        registries.buffers.insert(9, buffer_record());

        let entries = vec![
            BindingSetEntry::new(0, BindingResource::Texture(Texture::new(7))),
            BindingSetEntry::new(1, BindingResource::Sampler(Sampler::new(8))),
            BindingSetEntry::new(
                2,
                BindingResource::UniformBuffer {
                    buffer: Buffer::new(9),
                    offset: 0,
                    size: Some(64),
                },
            ),
        ];
        let first = plan_set(&registries, layout, &declarations, &entries).unwrap();
        assert_eq!((first.resource_base, first.resource_count), (0, 1));
        assert_eq!((first.sampler_base, first.sampler_count), (0, 1));
        assert_eq!(first.writes.len(), 2);

        let live = BindingSet::new(1);
        registries.binding_sets.insert(
            live.id(),
            Dx12SetRecord {
                layout_id: layout.id(),
                entries: entries.clone(),
                resource_heap_base: first.resource_base,
                resource_heap_count: first.resource_count,
                sampler_heap_base: first.sampler_base,
                sampler_heap_count: first.sampler_count,
            },
        );
        let second = plan_set(&registries, layout, &declarations, &entries).unwrap();
        // Samplers no longer reuse the RTV counter, and sets never overlap.
        assert_eq!((second.resource_base, second.resource_count), (1, 1));
        assert_eq!((second.sampler_base, second.sampler_count), (1, 1));
        unregister_set(&mut registries, live);
        let recycled = plan_set(&registries, layout, &declarations, &entries).unwrap();
        assert_eq!((recycled.resource_base, recycled.sampler_base), (0, 0));
    }

    #[test]
    fn root_parameters_keep_full_width_table_pointers() {
        let sets = vec![vec![
            BindingLayoutEntry::new(0, BindingType::Texture, ShaderVisibility::FRAGMENT),
            BindingLayoutEntry::new(1, BindingType::Sampler, ShaderVisibility::FRAGMENT),
            BindingLayoutEntry::new(2, BindingType::UniformBuffer, ShaderVisibility::VERTEX),
        ]];
        let (parameters, ranges, mapping) = build_root_parameters(&sets, &[]).unwrap();
        assert_eq!(parameters.len(), 3);
        assert_eq!(ranges.len(), 2);
        assert_eq!(mapping.buffer_root_index[&(0, 2)], 0);
        assert_eq!(mapping.resource_table_root_index[0], Some(1));
        assert_eq!(mapping.sampler_table_root_index[0], Some(2));

        // SAFETY: index 1 was built as a descriptor table above.
        let table = unsafe { parameters[1].Anonymous.DescriptorTable };
        assert_eq!(table.NumDescriptorRanges, 1);
        assert_eq!(table.pDescriptorRanges, ranges[0].as_ptr());
        assert_eq!(
            parameters[1].ParameterType,
            sys::D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE
        );
        // A root CBV is parameter type 2 and keeps its visibility at the end.
        assert_eq!(
            parameters[0].ParameterType,
            sys::D3D12_ROOT_PARAMETER_TYPE_CBV
        );
        assert_eq!(
            parameters[0].ShaderVisibility,
            sys::D3D12_SHADER_VISIBILITY_VERTEX
        );
        // Slang/DXIL declares every binding in space0, regardless of set index.
        // SAFETY: index 0 was built as a root descriptor above.
        let root = unsafe { parameters[0].Anonymous.Descriptor };
        assert_eq!((root.ShaderRegister, root.RegisterSpace), (2, 0));
        assert_eq!(ranges[0][0].RegisterSpace, 0);
    }

    #[test]
    fn the_root_signature_dword_budget_is_enforced() {
        let oversized = vec![PushConstantRange::new(ShaderStage::Vertex, 0, 4 * 65)];
        assert!(build_root_parameters(&[], &oversized).is_err());
        let exact = vec![PushConstantRange::new(ShaderStage::Vertex, 0, 4 * 64)];
        assert!(build_root_parameters(&[], &exact).is_ok());
    }
}
