//! Translation of NativeRHI/Metal/MetalAccelerationStructure.swift.
//!
//! A BLAS/TLAS owns its native structure, the descriptor that built it, the
//! scratch size the driver reported, and strong references to every input
//! resource, so a descriptor can never outlive the buffers it points at.
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSArray;
use objc2_metal::{
    MTLAccelerationStructure, MTLAccelerationStructureDescriptor,
    MTLAccelerationStructureGeometryDescriptor, MTLAccelerationStructureInstanceDescriptor,
    MTLAccelerationStructureInstanceOptions, MTLAccelerationStructureTriangleGeometryDescriptor,
    MTLBuffer, MTLDevice, MTLInstanceAccelerationStructureDescriptor, MTLPackedFloat3,
    MTLPackedFloat4x3, MTLPrimitiveAccelerationStructureDescriptor, MTLResource,
};

use crate::rhi::{
    AccelerationInstance, AccelerationStructureDescriptor, InstanceTransform, RhiError, RhiResult,
    TriangleGeometry, rhi_require,
};

use super::registries::MetalRegistries;

/// A built acceleration structure plus everything it depends on.
pub struct MetalAccelerationStructure {
    pub structure: Retained<ProtocolObject<dyn MTLAccelerationStructure>>,
    /// Retained so the build command can replay the exact descriptor used for
    /// sizing; it owns the geometry and instance-buffer references as well.
    pub descriptor: Retained<MTLAccelerationStructureDescriptor>,
    pub scratch_size: usize,
    /// Input buffers and child structures, kept alive for the traversal.
    pub dependencies: Vec<Retained<ProtocolObject<dyn MTLResource>>>,
    /// Whether this structure is a top-level structure. A TLAS may only
    /// reference bottom-level children, so the encoder checks this rather than
    /// asking the native descriptor its class.
    pub top_level: bool,
}

impl core::fmt::Debug for MetalAccelerationStructure {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MetalAccelerationStructure")
            .field("scratch_size", &self.scratch_size)
            .field("dependencies", &self.dependencies.len())
            .field("top_level", &self.top_level)
            .finish()
    }
}

/// Upcasts any Metal resource protocol object to `MTLResource`, which is the
/// type `useResource:usage:` accepts for indirect traversal dependencies.
pub fn as_resource<P: ?Sized>(
    object: Retained<ProtocolObject<P>>,
) -> Retained<ProtocolObject<dyn MTLResource>> {
    // SAFETY: every Metal object this backend stores conforms to `MTLResource`,
    // so re-typing the same object pointer to that protocol is sound.
    unsafe { Retained::cast_unchecked(object) }
}

/// Whether `geometry` describes a triangle range fully inside its vertex buffer.
pub fn validate_triangle_geometry(geometry: &TriangleGeometry, capacity: usize) -> RhiResult<()> {
    rhi_require(
        geometry.triangle_count > 0
            && geometry.vertex_offset >= 0
            && geometry.vertex_offset % 4 == 0
            && geometry.vertex_stride >= 12
            && geometry.vertex_stride % 4 == 0,
        "invalid triangle geometry dimensions",
    )?;
    let vertices = geometry
        .triangle_count
        .checked_mul(3)
        .ok_or_else(|| RhiError::invalid_argument("triangle count overflows"))?;
    let last_offset = (vertices - 1)
        .checked_mul(geometry.vertex_stride)
        .ok_or_else(|| RhiError::invalid_argument("triangle count overflows"))?;
    let end = geometry
        .vertex_offset
        .checked_add(last_offset)
        .ok_or_else(|| RhiError::invalid_argument("triangle count overflows"))?;
    rhi_require(
        end >= 0 && end as usize <= capacity.saturating_sub(12),
        "triangle geometry exceeds vertex buffer",
    )
}

/// Packs one instance record in Metal's `MTLAccelerationStructureInstanceDescriptor`
/// layout. Metal stores the transform as three packed columns, so the RHI's
/// row-major rows become the transposed packed columns the driver expects.
pub fn packed_instance(
    transform: &InstanceTransform,
    mask: u32,
    index: u32,
) -> MTLAccelerationStructureInstanceDescriptor {
    MTLAccelerationStructureInstanceDescriptor {
        transformationMatrix: MTLPackedFloat4x3 {
            columns: [
                MTLPackedFloat3 {
                    x: transform.x[0],
                    y: transform.y[0],
                    z: transform.z[0],
                },
                MTLPackedFloat3 {
                    x: transform.x[1],
                    y: transform.y[1],
                    z: transform.z[1],
                },
                MTLPackedFloat3 {
                    x: transform.x[2],
                    y: transform.y[2],
                    z: transform.z[2],
                },
                MTLPackedFloat3 {
                    x: transform.x[3],
                    y: transform.y[3],
                    z: transform.z[3],
                },
            ],
        },
        options: MTLAccelerationStructureInstanceOptions::Opaque,
        mask,
        intersectionFunctionTableOffset: 0,
        accelerationStructureIndex: index,
    }
}

/// Serializes instance records into the raw bytes Metal reads from the buffer.
pub fn packed_bytes(instances: &[MTLAccelerationStructureInstanceDescriptor]) -> Vec<u8> {
    // SAFETY: `MTLAccelerationStructureInstanceDescriptor` is `#[repr(C)]` and
    // holds only integers and packed floats, so every byte of the slice is
    // initialised and readable as `size_of_val` bytes.
    unsafe {
        std::slice::from_raw_parts(
            instances.as_ptr().cast::<u8>(),
            std::mem::size_of_val(instances),
        )
        .to_vec()
    }
}

fn bottom_level(
    registries: &MetalRegistries,
    geometries: &[TriangleGeometry],
    dependencies: &mut Vec<Retained<ProtocolObject<dyn MTLResource>>>,
) -> RhiResult<Retained<MTLAccelerationStructureDescriptor>> {
    rhi_require(!geometries.is_empty(), "BLAS requires triangle geometry")?;
    let mut descriptors: Vec<Retained<MTLAccelerationStructureGeometryDescriptor>> =
        Vec::with_capacity(geometries.len());
    for geometry in geometries {
        let buffer = registries
            .buffers
            .get(&geometry.vertices.id())
            .ok_or_else(|| {
                RhiError::invalid_argument("BLAS references an unknown vertex buffer")
            })?;
        validate_triangle_geometry(geometry, buffer.length())?;
        let triangle = { MTLAccelerationStructureTriangleGeometryDescriptor::new() };
        triangle.setVertexBuffer(Some(buffer));
        // SAFETY: `validate_triangle_geometry` proved `vertex_offset` is a
        // non-negative multiple of four, and the descriptor accepts any such
        // offset inside the bound buffer.
        unsafe { triangle.setVertexBufferOffset(geometry.vertex_offset as usize) };
        triangle.setVertexStride(geometry.vertex_stride as usize);
        triangle.setTriangleCount(geometry.triangle_count as usize);
        triangle.setOpaque(true);
        dependencies.push(as_resource(buffer.clone()));
        descriptors.push(Retained::into_super(triangle));
    }
    let primitive = { MTLPrimitiveAccelerationStructureDescriptor::new() };
    primitive.setGeometryDescriptors(Some(&NSArray::from_retained_slice(&descriptors)));
    Ok(Retained::into_super(primitive))
}

fn top_level(
    device: &ProtocolObject<dyn MTLDevice>,
    registries: &MetalRegistries,
    instances: &[AccelerationInstance],
    dependencies: &mut Vec<Retained<ProtocolObject<dyn MTLResource>>>,
) -> RhiResult<Retained<MTLAccelerationStructureDescriptor>> {
    rhi_require(!instances.is_empty(), "TLAS requires instances")?;
    let mut structures: Vec<Retained<ProtocolObject<dyn MTLAccelerationStructure>>> =
        Vec::with_capacity(instances.len());
    let mut packed: Vec<MTLAccelerationStructureInstanceDescriptor> =
        Vec::with_capacity(instances.len());
    for (index, value) in instances.iter().enumerate() {
        let child = registries
            .acceleration_structures
            .get(&value.structure.id())
            .ok_or_else(|| {
                RhiError::invalid_argument("TLAS instances must reference live BLAS handles")
            })?;
        rhi_require(
            !child.top_level,
            "TLAS instances must reference a bottom-level acceleration structure",
        )?;
        let transform = &value.transform;
        rhi_require(
            [transform.x, transform.y, transform.z]
                .iter()
                .all(|row| row.iter().all(|component| component.is_finite())),
            "instance transform must contain finite values",
        )?;
        structures.push(child.structure.clone());
        dependencies.push(as_resource(child.structure.clone()));
        packed.push(packed_instance(transform, value.mask, index as u32));
    }

    // Metal reads instance descriptors from a shared-storage buffer.
    let bytes = packed_bytes(&packed);
    // SAFETY: `bytes` is live for the call and Metal copies it into the new
    // buffer before returning.
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            core::ptr::NonNull::new(bytes.as_ptr() as *mut core::ffi::c_void)
                .expect("packed instance bytes are non-empty"),
            bytes.len(),
            objc2_metal::MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or(RhiError::OutOfMemory)?;
    dependencies.push(as_resource(buffer.clone()));

    let instance = { MTLInstanceAccelerationStructureDescriptor::new() };
    instance.setInstanceDescriptorBuffer(Some(&buffer));
    instance.setInstanceCount(instances.len());
    instance.setInstancedAccelerationStructures(Some(&NSArray::from_retained_slice(&structures)));
    Ok(Retained::into_super(instance))
}

impl MetalAccelerationStructure {
    /// Builds the native structure for `descriptor`, resolving every referenced
    /// handle through `registries`.
    pub fn create(
        device: &ProtocolObject<dyn MTLDevice>,
        registries: &MetalRegistries,
        descriptor: &AccelerationStructureDescriptor,
    ) -> RhiResult<Self> {
        rhi_require(
            device.supportsRaytracing(),
            "adapter has no ray tracing support",
        )?;
        let mut dependencies = Vec::new();
        let (native, top_level) = match descriptor {
            AccelerationStructureDescriptor::BottomLevel(geometries) => (
                bottom_level(registries, geometries, &mut dependencies)?,
                false,
            ),
            AccelerationStructureDescriptor::TopLevel(instances) => (
                top_level(device, registries, instances, &mut dependencies)?,
                true,
            ),
        };
        let sizes = { device.accelerationStructureSizesWithDescriptor(&native) };
        let structure = device
            .newAccelerationStructureWithSize(sizes.accelerationStructureSize)
            .ok_or(RhiError::OutOfMemory)?;
        Ok(Self {
            structure,
            descriptor: native,
            scratch_size: sizes.buildScratchBufferSize,
            dependencies,
            top_level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_instances_pack_transposed_columns() {
        let record = packed_instance(&InstanceTransform::default(), 0xff, 3);
        assert_eq!(record.accelerationStructureIndex, 3);
        assert_eq!(record.mask, 0xff);
        assert_eq!(record.intersectionFunctionTableOffset, 0);
        assert!(
            record
                .options
                .contains(MTLAccelerationStructureInstanceOptions::Opaque)
        );
        // Row 0 of an identity transform is a basis vector, so column 0 keeps a
        // single one in its first component.
        assert_eq!(record.transformationMatrix.columns[0].x, 1.0);
        assert_eq!(record.transformationMatrix.columns[1].y, 1.0);
        assert_eq!(record.transformationMatrix.columns[2].z, 1.0);
        assert_eq!(record.transformationMatrix.columns[0].y, 0.0);
    }

    #[test]
    fn packed_records_serialize_to_their_repr_c_size() {
        let records = [packed_instance(&InstanceTransform::default(), 1, 0)];
        let bytes = packed_bytes(&records);
        assert_eq!(
            bytes.len(),
            std::mem::size_of::<MTLAccelerationStructureInstanceDescriptor>()
        );
        assert!(!bytes.is_empty());
    }

    #[test]
    fn triangle_geometry_rejects_overlowing_and_out_of_bounds_ranges() {
        let mut geometry = TriangleGeometry::new(crate::rhi::Buffer::new(1), 1);
        assert!(validate_triangle_geometry(&geometry, 36).is_ok());
        // One triangle needs three 12-byte vertices plus the final 12 bytes.
        assert!(validate_triangle_geometry(&geometry, 35).is_err());
        geometry.triangle_count = 0;
        assert!(validate_triangle_geometry(&geometry, 4096).is_err());
        geometry.triangle_count = 1;
        geometry.vertex_stride = 10;
        assert!(validate_triangle_geometry(&geometry, 4096).is_err());
        geometry.vertex_stride = 12;
        geometry.vertex_offset = 2;
        assert!(validate_triangle_geometry(&geometry, 4096).is_err());
        geometry.vertex_offset = i64::MAX;
        assert!(validate_triangle_geometry(&geometry, 4096).is_err());
    }
}
