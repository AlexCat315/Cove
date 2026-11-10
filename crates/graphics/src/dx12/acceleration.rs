//! DXR triangle and instance acceleration structures for the shared RHI.
use crate::rhi::{
    AccelerationStructureDescriptor, RhiError, RhiResult, rhi_byte_range, rhi_require,
};
use windows::Win32::Graphics::{Direct3D12::*, Dxgi::Common::*};
use windows::core::Interface;

use super::device::Dx12DeviceHandle;
use super::extensions::native_error;
use super::resources::{CommandListVtbl, Dx12Registries};
use super::sys;

enum Inputs {
    Triangles {
        geometry: Vec<D3D12_RAYTRACING_GEOMETRY_DESC>,
        _vertices: Vec<ID3D12Resource>,
    },
    Instances {
        buffer: ID3D12Resource,
        count: u32,
        _bottom_levels: Vec<ID3D12Resource>,
    },
}

impl Inputs {
    fn native(&self) -> D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
        let mut native = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
            Flags: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_TRACE,
            DescsLayout: D3D12_ELEMENTS_LAYOUT_ARRAY,
            ..Default::default()
        };
        match self {
            Self::Triangles { geometry, .. } => {
                native.Type = D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL;
                native.NumDescs = geometry.len() as u32;
                native.Anonymous.pGeometryDescs = geometry.as_ptr();
            }
            Self::Instances { buffer, count, .. } => {
                native.Type = D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL;
                native.NumDescs = *count;
                // SAFETY: the instance buffer is a live committed buffer.
                native.Anonymous.InstanceDescs = unsafe { buffer.GetGPUVirtualAddress() };
            }
        }
        native
    }
}

pub struct Acceleration {
    inputs: Inputs,
    result: ID3D12Resource,
    scratch_bytes: u64,
}

impl Acceleration {
    pub fn new(
        device: &Dx12DeviceHandle,
        registries: &Dx12Registries,
        descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<Self> {
        if !device.capabilities.ray_tracing.acceleration_structures {
            return Err(RhiError::unsupported_feature(
                "the DX12 adapter has no DXR acceleration structure support",
            ));
        }
        let inputs = match descriptor {
            AccelerationStructureDescriptor::BottomLevel(triangles) => {
                rhi_require(
                    u32::try_from(triangles.len()).is_ok(),
                    "too many triangle geometries",
                )?;
                let mut geometry = Vec::with_capacity(triangles.len());
                let mut vertices = Vec::with_capacity(triangles.len());
                for triangle in triangles {
                    let buffer = registries
                        .buffers
                        .get(&triangle.vertices.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown DXR vertex buffer"))?;
                    rhi_require(
                        buffer.heap_type != sys::D3D12_HEAP_TYPE_READBACK,
                        "DXR vertex data must be GPU readable",
                    )?;
                    let count = u32::try_from(triangle.triangle_count)
                        .ok()
                        .and_then(|count| count.checked_mul(3))
                        .filter(|count| *count > 0)
                        .ok_or_else(|| RhiError::invalid_argument("invalid DXR triangle count"))?;
                    let offset = usize::try_from(triangle.vertex_offset)
                        .map_err(|_| RhiError::invalid_argument("invalid DXR vertex offset"))?;
                    let stride = usize::try_from(triangle.vertex_stride)
                        .map_err(|_| RhiError::invalid_argument("invalid DXR vertex stride"))?;
                    rhi_require(
                        offset.is_multiple_of(4) && stride >= 12 && stride.is_multiple_of(4),
                        "invalid DXR vertex alignment",
                    )?;
                    let bytes = (count as usize - 1)
                        .checked_mul(stride)
                        .and_then(|bytes| bytes.checked_add(12))
                        .ok_or_else(|| RhiError::invalid_argument("DXR vertex range overflow"))?;
                    rhi_byte_range(offset, bytes, buffer.size as usize)?;
                    // SAFETY: the registry owns the buffer, and the clone keeps
                    // it alive for future builds of this BLAS.
                    let retained = unsafe {
                        ID3D12Resource::from_raw_borrowed(&buffer.resource)
                            .unwrap()
                            .clone()
                    };
                    vertices.push(retained);
                    geometry.push(D3D12_RAYTRACING_GEOMETRY_DESC {
                        Type: D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES,
                        Flags: D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE,
                        Anonymous: D3D12_RAYTRACING_GEOMETRY_DESC_0 {
                            Triangles: D3D12_RAYTRACING_GEOMETRY_TRIANGLES_DESC {
                                VertexFormat: DXGI_FORMAT_R32G32B32_FLOAT,
                                VertexCount: count,
                                VertexBuffer: D3D12_GPU_VIRTUAL_ADDRESS_AND_STRIDE {
                                    StartAddress: buffer.gpu_address + offset as u64,
                                    StrideInBytes: stride as u64,
                                },
                                ..Default::default()
                            },
                        },
                    });
                }
                Inputs::Triangles {
                    geometry,
                    _vertices: vertices,
                }
            }
            AccelerationStructureDescriptor::TopLevel(instances) => {
                let mut packed = Vec::with_capacity(instances.len());
                let mut bottom_levels = Vec::with_capacity(instances.len());
                for (id, instance) in instances.into_iter().enumerate() {
                    let blas = registries
                        .accelerations
                        .get(&instance.structure.id())
                        .filter(|record| !record.is_top_level())
                        .ok_or_else(|| {
                            RhiError::invalid_argument(
                                "DXR instance requires a bottom-level acceleration structure",
                            )
                        })?;
                    let transform = instance.transform;
                    packed.push(D3D12_RAYTRACING_INSTANCE_DESC {
                        Transform: core::array::from_fn(|i| {
                            [transform.x, transform.y, transform.z][i / 4][i % 4]
                        }),
                        _bitfield1: id as u32 | instance.mask << 24,
                        _bitfield2: 0,
                        AccelerationStructure: blas.address(),
                    });
                    bottom_levels.push(blas.result.clone());
                }
                let bytes = core::mem::size_of_val(packed.as_slice());
                let upload = super::buffer::create_upload_heap(device.device, bytes)?;
                // SAFETY: the instance descriptor is a 64-byte C layout with
                // no padding. The mapped buffer covers all descriptors.
                const {
                    assert!(core::mem::size_of::<D3D12_RAYTRACING_INSTANCE_DESC>() == 64);
                }
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        packed.as_ptr().cast::<u8>(),
                        upload.mapped,
                        bytes,
                    )
                };
                Inputs::Instances {
                    // SAFETY: this transfers the single owned COM reference.
                    buffer: unsafe { ID3D12Resource::from_raw(upload.resource) },
                    count: packed.len() as u32,
                    _bottom_levels: bottom_levels,
                }
            }
        };
        // SAFETY: the handle owns this live device and the inputs stay in scope.
        let native = unsafe { ID3D12Device::from_raw_borrowed(&device.device).unwrap() };
        let dxr = native.cast::<ID3D12Device5>().map_err(native_error)?;
        let mut sizes = D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO::default();
        unsafe { dxr.GetRaytracingAccelerationStructurePrebuildInfo(&inputs.native(), &mut sizes) };
        rhi_require(
            sizes.ResultDataMaxSizeInBytes != 0 && sizes.ScratchDataSizeInBytes != 0,
            "DXR driver rejected the acceleration structure inputs",
        )?;
        let result = allocate(
            native,
            sizes.ResultDataMaxSizeInBytes,
            D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE,
        )?;
        Ok(Self {
            inputs,
            result,
            scratch_bytes: sizes.ScratchDataSizeInBytes,
        })
    }

    pub fn is_top_level(&self) -> bool {
        matches!(self.inputs, Inputs::Instances { .. })
    }

    pub fn address(&self) -> u64 {
        // SAFETY: the record owns a live acceleration structure buffer.
        unsafe { self.result.GetGPUVirtualAddress() }
    }

    pub fn encode(
        &self,
        device: &Dx12DeviceHandle,
        list: sys::ComPtr,
        staged: &mut Vec<sys::ComPtr>,
    ) -> RhiResult<()> {
        // SAFETY: the device and command list are live for recording.
        let native = unsafe { ID3D12Device::from_raw_borrowed(&device.device).unwrap() };
        let graphics = unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&list).unwrap() };
        let dxr = graphics
            .cast::<ID3D12GraphicsCommandList4>()
            .map_err(native_error)?;
        let scratch = allocate(
            native,
            self.scratch_bytes,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )?;
        let desc = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
            DestAccelerationStructureData: self.address(),
            Inputs: self.inputs.native(),
            ScratchAccelerationStructureData: unsafe { scratch.GetGPUVirtualAddress() },
            SourceAccelerationStructureData: 0,
        };
        // SAFETY: all buffer addresses remain alive through GPU completion.
        // The UAV barriers order builds against earlier and later queries.
        unsafe {
            uav_barrier(list, self.result.as_raw());
            dxr.BuildRaytracingAccelerationStructure(&desc, None);
            uav_barrier(list, self.result.as_raw());
        }
        staged.push(scratch.into_raw());
        Ok(())
    }
}

fn allocate(
    device: &ID3D12Device,
    bytes: u64,
    state: D3D12_RESOURCE_STATES,
) -> RhiResult<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        CreationNodeMask: 1,
        VisibleNodeMask: 1,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: bytes,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        ..Default::default()
    };
    let mut resource = None;
    // SAFETY: both descriptions describe a default-heap UAV buffer.
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            state,
            None,
            &mut resource,
        )
    }
    .map_err(native_error)?;
    resource.ok_or_else(|| RhiError::invalid_argument("DXR buffer allocation returned no resource"))
}

pub fn write_srv(device: sys::ComPtr, cpu: sys::D3D12_CPU_DESCRIPTOR_HANDLE, address: u64) {
    let descriptor = D3D12_SHADER_RESOURCE_VIEW_DESC {
        ViewDimension: D3D12_SRV_DIMENSION_RAYTRACING_ACCELERATION_STRUCTURE,
        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
        Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
            RaytracingAccelerationStructure: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_SRV {
                Location: address,
            },
        },
        ..Default::default()
    };
    // SAFETY: DXR SRVs take a GPU address in the descriptor and a null resource.
    unsafe {
        let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
        ((*vtbl).CreateShaderResourceView)(
            device,
            core::ptr::null_mut(),
            (&descriptor as *const D3D12_SHADER_RESOURCE_VIEW_DESC).cast(),
            cpu,
        );
    }
}

unsafe fn uav_barrier(list: sys::ComPtr, resource: sys::ComPtr) {
    let barrier = sys::D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV.0 as u32,
        Transition: sys::D3D12_RESOURCE_TRANSITION_BARRIER {
            // The UAV union member consists of just this pointer.
            pResource: resource,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: the list is open and owns a reference to the recorded resource.
    unsafe {
        let vtbl = *(list as *mut *const CommandListVtbl);
        ((*vtbl).ResourceBarrier)(list, 1, &barrier);
    }
}
