//! Translation of NativeRHI/Vulkan/Resources/VulkanAcceleration.swift:
//! BLAS/TLAS creation, device addresses, and build-command encoding.
//!
//! Vulkan needs a buffer device address for every acceleration-structure input,
//! so this module owns the address queries too. The scratch buffer is allocated
//! with the driver's required alignment, and the build reuses the geometry the
//! structure was created with.
use std::sync::{Arc, Mutex};

use crate::rhi::{
    AccelerationInstance, AccelerationStructure, AccelerationStructureDescriptor, Buffer,
    BufferDescriptor, BufferUsage, RhiError, RhiResult, rhi_byte_range, rhi_count, rhi_require,
};

use super::memory::VulkanMemoryAllocator;
use super::registries::{VulkanAccelerationRecord, VulkanRegistries};
use super::resources::{create_buffer, destroy_buffer, lock};
use super::setup::VulkanContext;
use super::sys;

/// The device address of `buffer`, which every acceleration input needs.
pub fn buffer_address(
    context: &VulkanContext,
    registries: &VulkanRegistries,
    handle: Buffer,
) -> RhiResult<sys::VkDeviceAddress> {
    let record = registries
        .buffers
        .get(&handle.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown device-address buffer"))?;
    let query = context.advanced.buffer_device_address.ok_or_else(|| {
        RhiError::unsupported_feature("Vulkan buffer device addresses are unavailable")
    })?;
    let info = sys::VkBufferDeviceAddressInfo {
        sType: sys::VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO,
        pNext: core::ptr::null(),
        buffer: record.buffer,
    };
    // SAFETY: the buffer is live on this device and `info` outlives the call.
    let address = unsafe { query(context.device, &info) };
    rhi_require(address != 0, "buffer device address unavailable")?;
    Ok(address)
}

/// The device address of an acceleration structure.
pub fn structure_address(
    context: &VulkanContext,
    registries: &VulkanRegistries,
    handle: AccelerationStructure,
) -> RhiResult<sys::VkDeviceAddress> {
    let record = registries
        .acceleration_structures
        .get(&handle.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown acceleration structure"))?;
    let query = context
        .advanced
        .acceleration_structure_address
        .ok_or_else(|| {
            RhiError::unsupported_feature("Vulkan acceleration structure addresses are unavailable")
        })?;
    let info = sys::VkAccelerationStructureDeviceAddressInfoKHR {
        sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_DEVICE_ADDRESS_INFO_KHR,
        pNext: core::ptr::null(),
        accelerationStructure: record.native,
    };
    // SAFETY: the structure is live and `info` outlives the call.
    let address = unsafe { query(context.device, &info) };
    rhi_require(address != 0, "acceleration structure address unavailable")?;
    Ok(address)
}

/// Packs instance records into the 64-byte GPU layout the driver reads.
///
/// Swift called `grhi_vulkan_pack_instance` for this because the C bitfields are
/// not writable from Swift. The Rust struct declares that word as one `u32`, so
/// the pack happens here.
pub fn pack_instances(
    instances: &[AccelerationInstance],
    addresses: &[sys::VkDeviceAddress],
) -> RhiResult<Vec<u8>> {
    rhi_require(
        instances.len() == addresses.len(),
        "every instance needs a bottom-level structure address",
    )?;
    let mut records = Vec::with_capacity(instances.len());
    for (index, (instance, address)) in instances.iter().zip(addresses).enumerate() {
        rhi_require(instance.mask <= 0xff, "instance mask exceeds eight bits")?;
        let transform = &instance.transform;
        // The GPU layout is a row-major 3x4 matrix read as twelve floats.
        let matrix = [
            transform.x[0],
            transform.x[1],
            transform.x[2],
            transform.x[3],
            transform.y[0],
            transform.y[1],
            transform.y[2],
            transform.y[3],
            transform.z[0],
            transform.z[1],
            transform.z[2],
            transform.z[3],
        ];
        rhi_require(
            matrix.iter().all(|value| value.is_finite()),
            "instance transform must contain finite values",
        )?;
        records.push(sys::VkAccelerationStructureInstanceKHR::new(
            matrix,
            index as u32,
            instance.mask,
            *address,
        ));
    }
    // SAFETY: the record is `#[repr(C)]` and holds only floats and integers, so its
    // bytes are initialised and readable as `size_of_val` bytes.
    Ok(unsafe {
        std::slice::from_raw_parts(
            records.as_ptr().cast::<u8>(),
            std::mem::size_of_val(&records[..]),
        )
        .to_vec()
    })
}

/// Creates a BLAS or TLAS and its backing buffers.
pub fn create_acceleration_structure(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    handle: AccelerationStructure,
    descriptor: AccelerationStructureDescriptor,
) -> RhiResult<()> {
    rhi_require(
        context.features.acceleration_structures,
        "Vulkan acceleration structures unavailable",
    )?;
    let sizes_query = context
        .advanced
        .acceleration_structure_build_sizes
        .ok_or_else(|| {
            RhiError::unsupported_feature("Vulkan acceleration structures unavailable")
        })?;
    let create = context
        .advanced
        .create_acceleration_structure
        .ok_or_else(|| {
            RhiError::unsupported_feature("Vulkan acceleration structures unavailable")
        })?;

    // Every buffer this call creates is released if a later step fails.
    let mut owned: Vec<Buffer> = Vec::new();
    let build = (|| -> RhiResult<VulkanAccelerationRecord> {
        let mut geometry: Vec<sys::VkAccelerationStructureGeometryKHR> = Vec::new();
        let mut ranges: Vec<sys::VkAccelerationStructureBuildRangeInfoKHR> = Vec::new();
        let mut instance_buffer: Option<Buffer> = None;
        let top_level;

        match &descriptor {
            AccelerationStructureDescriptor::BottomLevel(triangles) => {
                top_level = false;
                for triangle in triangles {
                    let (vertices_size, vertex_address) = {
                        let registries_guard = lock(registries);
                        let vertices = registries_guard
                            .buffers
                            .get(&triangle.vertices.id())
                            .map(|record| record.size)
                            .ok_or_else(|| RhiError::invalid_argument("unknown BLAS vertices"))?;
                        let address =
                            buffer_address(context, &registries_guard, triangle.vertices)?;
                        (vertices, address)
                    };
                    let count = rhi_count(triangle.triangle_count)?;
                    rhi_require(
                        count > 0 && count <= u32::MAX / 3 && triangle.vertex_stride >= 12,
                        "invalid BLAS geometry",
                    )?;
                    let vertex_bytes = (IntLike::from(count) * 3 - 1)
                        .checked_mul(triangle.vertex_stride)
                        .ok_or_else(|| RhiError::invalid_argument("BLAS range overflow"))?;
                    rhi_byte_range(
                        triangle.vertex_offset.max(0) as usize,
                        (vertex_bytes + 12) as usize,
                        vertices_size,
                    )?;
                    let triangles_data = sys::VkAccelerationStructureGeometryTrianglesDataKHR {
                        sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_TRIANGLES_DATA_KHR,
                        pNext: core::ptr::null(),
                        vertexFormat: sys::VK_FORMAT_R32G32B32_SFLOAT,
                        vertexData: vertex_address
                            + (triangle.vertex_offset.max(0) as sys::VkDeviceAddress),
                        vertexStride: triangle.vertex_stride as sys::VkDeviceSize,
                        maxVertex: count * 3 - 1,
                        indexType: sys::VK_INDEX_TYPE_NONE_KHR,
                        indexData: 0,
                        transformData: 0,
                    };
                    geometry.push(sys::VkAccelerationStructureGeometryKHR {
                        sType:
                            sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
                        pNext: core::ptr::null(),
                        geometryType: sys::VK_GEOMETRY_TYPE_TRIANGLES_KHR,
                        geometry: sys::VkAccelerationStructureGeometryDataKHR {
                            triangles: triangles_data,
                        },
                        flags: sys::VK_GEOMETRY_OPAQUE_BIT_KHR,
                    });
                    ranges.push(sys::VkAccelerationStructureBuildRangeInfoKHR {
                        primitiveCount: count,
                        primitiveOffset: 0,
                        firstVertex: 0,
                        transformOffset: 0,
                    });
                }
            }
            AccelerationStructureDescriptor::TopLevel(instances) => {
                top_level = true;
                rhi_require(
                    !instances.is_empty() && instances.len() <= 0x100_0000,
                    "invalid TLAS instance count",
                )?;
                let mut addresses = Vec::with_capacity(instances.len());
                {
                    let registries_guard = lock(registries);
                    for instance in instances {
                        let record = registries_guard
                            .acceleration_structures
                            .get(&instance.structure.id())
                            .ok_or_else(|| {
                                RhiError::invalid_argument("TLAS must reference a BLAS")
                            })?;
                        rhi_require(!record.top_level, "TLAS must reference a BLAS")?;
                        addresses.push(structure_address(
                            context,
                            &registries_guard,
                            instance.structure,
                        )?);
                    }
                }
                let bytes = pack_instances(instances, &addresses)?;
                // Coherent host storage makes the instance data visible to
                // whichever queue submits the build, so no hidden copy is needed.
                let buffer = Buffer::new(lock(registries).next_internal_id());
                create_buffer(
                    context,
                    registries,
                    allocator,
                    buffer,
                    BufferDescriptor::new(
                        bytes.len(),
                        BufferUsage::TRANSFER_SOURCE | BufferUsage::TRANSFER_DESTINATION,
                    ),
                )?;
                owned.push(buffer);
                super::submission::upload_buffer_data(context, registries, buffer, 0, &bytes)?;
                instance_buffer = Some(buffer);
                let instances_data = sys::VkAccelerationStructureGeometryInstancesDataKHR {
                    sType:
                        sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_INSTANCES_DATA_KHR,
                    pNext: core::ptr::null(),
                    arrayOfPointers: sys::VK_FALSE,
                    data: buffer_address(context, &lock(registries), buffer)?,
                };
                geometry.push(sys::VkAccelerationStructureGeometryKHR {
                    sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
                    pNext: core::ptr::null(),
                    geometryType: sys::VK_GEOMETRY_TYPE_INSTANCES_KHR,
                    geometry: sys::VkAccelerationStructureGeometryDataKHR {
                        instances: instances_data,
                    },
                    flags: 0,
                });
                ranges.push(sys::VkAccelerationStructureBuildRangeInfoKHR {
                    primitiveCount: instances.len() as u32,
                    primitiveOffset: 0,
                    firstVertex: 0,
                    transformOffset: 0,
                });
            }
        }
        rhi_require(
            !geometry.is_empty(),
            "acceleration structure requires input",
        )?;

        let build_info = sys::VkAccelerationStructureBuildGeometryInfoKHR {
            sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
            pNext: core::ptr::null(),
            type_: if top_level {
                sys::VK_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL_KHR
            } else {
                sys::VK_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL_KHR
            },
            flags: sys::VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_KHR,
            mode: sys::VK_BUILD_ACCELERATION_STRUCTURE_MODE_BUILD_KHR,
            srcAccelerationStructure: sys::VK_NULL_HANDLE,
            dstAccelerationStructure: sys::VK_NULL_HANDLE,
            geometryCount: geometry.len() as u32,
            pGeometries: geometry.as_ptr(),
            ppGeometries: core::ptr::null(),
            scratchData: 0,
        };
        let primitive_counts: Vec<u32> = ranges.iter().map(|range| range.primitiveCount).collect();
        let mut allocation = sys::VkAccelerationStructureBuildSizesInfoKHR {
            sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_SIZES_INFO_KHR,
            ..Default::default()
        };
        // SAFETY: `build_info` and `primitive_counts` outlive the call, and the
        // out-parameter is a live local.
        unsafe {
            (sizes_query)(
                context.device,
                sys::VK_ACCELERATION_STRUCTURE_BUILD_TYPE_DEVICE_KHR,
                &build_info,
                primitive_counts.as_ptr(),
                &mut allocation,
            )
        };

        let result_buffer = acceleration_buffer(
            context,
            registries,
            allocator,
            allocation.accelerationStructureSize,
        )?;
        owned.push(result_buffer);
        let alignment = context.advanced.scratch_alignment;
        let scratch_size = allocation
            .buildScratchSize
            .checked_add(alignment)
            .ok_or(RhiError::OutOfMemory)?;
        let scratch = acceleration_buffer(context, registries, allocator, scratch_size)?;
        owned.push(scratch);
        // The scratch address must be aligned, so allocate with slack and round up.
        let scratch_base = buffer_address(context, &lock(registries), scratch)?;
        let scratch_address = scratch_base.div_ceil(alignment) * alignment;

        let (result_native, result_size) = {
            let registries_guard = lock(registries);
            let record = registries_guard
                .buffers
                .get(&result_buffer.id())
                .ok_or_else(|| RhiError::invalid_argument("unknown acceleration buffer"))?;
            (record.buffer, record.size as sys::VkDeviceSize)
        };
        let create_info = sys::VkAccelerationStructureCreateInfoKHR {
            sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_CREATE_INFO_KHR,
            pNext: core::ptr::null(),
            createFlags: 0,
            buffer: result_native,
            offset: 0,
            size: result_size,
            type_: build_info.type_,
            deviceAddress: 0,
        };
        let mut created: sys::VkAccelerationStructureKHR = sys::VK_NULL_HANDLE;
        // SAFETY: the buffer is live and `create_info` outlives the call.
        let status = unsafe {
            (create)(
                context.device,
                &create_info,
                core::ptr::null(),
                &mut created,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && created != sys::VK_NULL_HANDLE,
            "vkCreateAccelerationStructureKHR failed",
        )?;

        Ok(VulkanAccelerationRecord {
            native: created,
            result: result_buffer,
            scratch,
            instance_buffer,
            scratch_address,
            geometry,
            ranges,
            top_level,
        })
    })();

    match build {
        Ok(record) => {
            lock(registries)
                .acceleration_structures
                .insert(handle.id(), record);
            Ok(())
        }
        Err(error) => {
            for buffer in owned {
                destroy_buffer(context, registries, allocator, buffer);
            }
            Err(error)
        }
    }
}

/// Integer helper so the triangle-range arithmetic stays in one integer type.
struct IntLike(i64);

impl IntLike {
    fn from(value: u32) -> i64 {
        Self(i64::from(value)).0
    }
}

/// A device-local buffer sized for acceleration-structure storage.
fn acceleration_buffer(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    size: sys::VkDeviceSize,
) -> RhiResult<Buffer> {
    let bytes = usize::try_from(size).map_err(|_| RhiError::OutOfMemory)?;
    rhi_require(
        bytes > 0,
        "acceleration structure buffer size must be positive",
    )?;
    let handle = Buffer::new(lock(registries).next_internal_id());
    create_buffer(
        context,
        registries,
        allocator,
        handle,
        BufferDescriptor::new(
            bytes,
            BufferUsage::STORAGE_READ
                | BufferUsage::STORAGE_WRITE
                | BufferUsage::TRANSFER_DESTINATION,
        ),
    )?;
    Ok(handle)
}

pub fn destroy_acceleration_structure(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    handle: AccelerationStructure,
) {
    let Some(record) = lock(registries)
        .acceleration_structures
        .remove(&handle.id())
    else {
        return;
    };
    if let Some(destroy) = context.advanced.destroy_acceleration_structure {
        // SAFETY: the frame ring waited for every submission that could use it.
        unsafe { (destroy)(context.device, record.native, core::ptr::null()) };
    }
    destroy_buffer(context, registries, allocator, record.result);
    destroy_buffer(context, registries, allocator, record.scratch);
    if let Some(buffer) = record.instance_buffer {
        destroy_buffer(context, registries, allocator, buffer);
    }
}

/// Encodes one build into `cmd`.
///
/// # Safety
/// `cmd` must be recording.
pub unsafe fn encode_build(
    context: &VulkanContext,
    registries: &VulkanRegistries,
    cmd: sys::VkCommandBuffer,
    handle: AccelerationStructure,
) -> RhiResult<()> {
    let record = registries
        .acceleration_structures
        .get(&handle.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown acceleration build"))?;
    let build = context
        .advanced
        .build_acceleration_structures
        .ok_or_else(|| RhiError::unsupported_feature("Vulkan acceleration builds unavailable"))?;
    // SAFETY: the caller guarantees `cmd` is recording.
    unsafe {
        crate::vulkan::submission::memory_dependency(
            context,
            cmd,
            sys::VK_ACCESS_MEMORY_WRITE_BIT,
            sys::VK_ACCESS_ACCELERATION_STRUCTURE_READ_BIT_KHR
                | sys::VK_ACCESS_ACCELERATION_STRUCTURE_WRITE_BIT_KHR,
        )
    };
    let build_info = sys::VkAccelerationStructureBuildGeometryInfoKHR {
        sType: sys::VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
        pNext: core::ptr::null(),
        type_: if record.top_level {
            sys::VK_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL_KHR
        } else {
            sys::VK_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL_KHR
        },
        flags: sys::VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_KHR,
        mode: sys::VK_BUILD_ACCELERATION_STRUCTURE_MODE_BUILD_KHR,
        srcAccelerationStructure: sys::VK_NULL_HANDLE,
        dstAccelerationStructure: record.native,
        geometryCount: record.geometry.len() as u32,
        pGeometries: record.geometry.as_ptr(),
        ppGeometries: core::ptr::null(),
        scratchData: record.scratch_address,
    };
    let range_pointer: *const sys::VkAccelerationStructureBuildRangeInfoKHR =
        record.ranges.as_ptr();
    let ranges: [*const sys::VkAccelerationStructureBuildRangeInfoKHR; 1] = [range_pointer];
    // SAFETY: the build info, geometry and ranges all live in the registry record,
    // which outlives this call, and `cmd` is recording.
    unsafe { (build)(cmd, 1, &build_info, ranges.as_ptr()) };
    // SAFETY: `cmd` is still recording.
    unsafe {
        crate::vulkan::submission::memory_dependency(
            context,
            cmd,
            sys::VK_ACCESS_ACCELERATION_STRUCTURE_WRITE_BIT_KHR,
            sys::VK_ACCESS_ACCELERATION_STRUCTURE_READ_BIT_KHR
                | sys::VK_ACCESS_ACCELERATION_STRUCTURE_WRITE_BIT_KHR,
        )
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::InstanceTransform;

    #[test]
    fn instance_records_use_the_64_byte_gpu_layout() {
        assert_eq!(
            std::mem::size_of::<sys::VkAccelerationStructureInstanceKHR>(),
            64
        );
    }

    #[test]
    fn packing_writes_row_major_transforms_and_custom_indices() {
        let mut instance = AccelerationInstance::new(AccelerationStructure::new(3));
        instance.transform = InstanceTransform {
            x: [1.0, 2.0, 3.0, 4.0],
            y: [5.0, 6.0, 7.0, 8.0],
            z: [9.0, 10.0, 11.0, 12.0],
        };
        instance.mask = 0x42;
        let bytes = pack_instances(&[instance], &[0xDEAD_BEEF]).expect("packs");
        assert_eq!(bytes.len(), 64);
        // The transform occupies the first 48 bytes as twelve consecutive floats.
        let matrix: Vec<f32> = bytes[..48]
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        assert_eq!(
            matrix,
            vec![
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0
            ]
        );
        // Spec layout: transform 0..48, `customIndex:24|mask:8` at 48..52,
        // `offset|flags` at 52..56, then the 8-byte address at 56..64.
        let word = u32::from_le_bytes(bytes[48..52].try_into().unwrap());
        assert_eq!(
            word >> 24,
            0x42,
            "the mask is the high byte of the instance word"
        );
        assert_eq!(word & 0x00FF_FFFF, 0, "instance 0 has custom index 0");
        assert_eq!(
            u32::from_le_bytes(bytes[52..56].try_into().unwrap()) & 0x00FF_FFFF,
            1,
            "the cull-disable flag is bit 0 of the offset word"
        );
        assert_eq!(
            u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
            0xDEAD_BEEF,
            "the device address ends the record"
        );
    }

    #[test]
    fn every_instance_needs_an_address() {
        let instance = AccelerationInstance::new(AccelerationStructure::new(1));
        assert!(pack_instances(&[instance], &[]).is_err());
    }

    #[test]
    fn non_finite_transforms_are_rejected() {
        let mut instance = AccelerationInstance::new(AccelerationStructure::new(1));
        instance.transform.x[0] = f32::NAN;
        assert!(pack_instances(&[instance], &[1]).is_err());
    }

    #[test]
    fn masks_are_limited_to_eight_bits() {
        let mut instance = AccelerationInstance::new(AccelerationStructure::new(1));
        instance.mask = 0x1ff;
        assert!(pack_instances(&[instance], &[1]).is_err());
    }
}
