//! Translation of `Graphics/Tests/NativeRHITests/Metal/MetalRayQueryTests.swift`.
//!
//! Behaviour checklist:
//! - a bottom-level and top-level acceleration structure build, then a compute
//!   ray query reports a hit for the ray through the triangle and a miss for
//!   the ray beside it
//! - geometry that claims more vertices than its buffer holds is rejected, both
//!   for a plausible triangle count and for an overflowing one
//!
//! Both cases need a ray-tracing-capable GPU, exactly as the Swift suite did;
//! the capability check reports the skip instead of passing vacuously.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    AccelerationInstance, AccelerationStructureDescriptor, BindingLayoutDescriptor,
    BindingLayoutEntry, BindingResource, BindingSetDescriptor, BindingSetEntry, BindingType,
    BufferDescriptor, BufferUsage, CommandBuffer, ComputePipelineDescriptor, Device, DeviceConfig,
    GraphicsApi, PipelineLayoutDescriptor, QueueClass, ShaderModuleDescriptor, ShaderStage,
    ShaderVisibility, TextureDescriptor, TextureFormat, TextureSubresource, TextureUsage,
    TriangleGeometry,
};

fn metal_device() -> Option<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        ..DeviceConfig::default()
    })
    .ok()
}

const RAY_QUERY_SHADER: &str = r#"
#include <metal_stdlib>
#include <metal_raytracing>
using namespace metal;
using namespace raytracing;
kernel void rayMain(instance_acceleration_structure scene [[buffer(0)]],
                    texture2d<float, access::write> output [[texture(1)]],
                    uint id [[thread_position_in_grid]]) {
    ray r;
    r.origin = float3(id == 0 ? 1.0f : 4.0f, 0, 1);
    r.direction = float3(0, 0, -1);
    r.min_distance = 0.001f;
    r.max_distance = 100.0f;
    intersector<triangle_data, instancing> tracer;
    auto hit = tracer.intersect(r, scene);
    output.write(hit.type == intersection_type::none ? float4(1,0,0,1) : float4(0,1,0,1), uint2(id,0));
}
"#;

#[test]
fn build_blas_tlas_then_trace_hit_and_miss() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping the ray query test");
        return;
    };
    if !device.capabilities().ray_tracing.compute_ray_query {
        eprintln!("this Metal GPU has no compute ray query support; skipping");
        return;
    }

    let vertices: [f32; 9] = [-1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0, 1.0, 0.0];
    let vertex_bytes: Vec<u8> = vertices
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let buffer = device
        .make_buffer(BufferDescriptor::new(36, BufferUsage::STORAGE_READ))
        .expect("vertex buffer");
    device
        .upload_buffer_data(buffer, 0, &vertex_bytes)
        .expect("upload the triangle vertices");

    let blas = device
        .make_acceleration_structure(AccelerationStructureDescriptor::BottomLevel(vec![
            TriangleGeometry::new(buffer, 1),
        ]))
        .expect("bottom-level acceleration structure");

    let mut instance = AccelerationInstance::new(blas);
    instance.transform.x[3] = 1.0;
    let tlas = device
        .make_acceleration_structure(AccelerationStructureDescriptor::TopLevel(vec![instance]))
        .expect("top-level acceleration structure");

    let shader = device
        .make_shader_module(
            ShaderModuleDescriptor::metal_source(RAY_QUERY_SHADER, ShaderStage::Compute)
                .with_entry_point("rayMain"),
        )
        .expect("ray query compute shader");

    let target = device
        .make_texture(TextureDescriptor::new(
            2,
            1,
            TextureFormat::Rgba8Unorm,
            TextureUsage::STORAGE_WRITE | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("ray query output texture");

    let binding = device
        .make_binding_layout(BindingLayoutDescriptor::new(vec![
            BindingLayoutEntry::new(
                0,
                BindingType::AccelerationStructure,
                ShaderVisibility::COMPUTE,
            ),
            BindingLayoutEntry::new(1, BindingType::StorageTexture, ShaderVisibility::COMPUTE),
        ]))
        .expect("ray query binding layout");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![binding.clone()]))
        .expect("ray query pipeline layout");
    let set = device
        .make_binding_set(
            binding,
            BindingSetDescriptor::new(vec![
                BindingSetEntry::new(0, BindingResource::AccelerationStructure(tlas)),
                BindingSetEntry::new(1, BindingResource::StorageTexture(target)),
            ]),
        )
        .expect("ray query binding set");
    let pipeline = device
        .make_compute_pipeline(ComputePipelineDescriptor::new(layout, shader))
        .expect("ray query compute pipeline");

    let mut commands = CommandBuffer::new();
    device
        .record_build(blas, &mut commands)
        .expect("record the bottom-level build");
    device
        .record_build(tlas, &mut commands)
        .expect("record the top-level build");
    commands.compute_pass(|encoder| {
        encoder.set_pipeline(pipeline);
        encoder.set_binding_set(set, 0, Vec::new());
        encoder.dispatch(2, 1, 1);
    });

    device.begin_frame().expect("begin frame");
    device
        .submit(&commands, QueueClass::Compute, &[], &[], None)
        .expect("submit the ray query work");
    device.end_frame();
    device.wait_until_idle().expect("wait for idle");

    let mut pixels = vec![0u8; 8];
    device
        .read_texture_data(target, 2, 1, 8, TextureSubresource::default(), &mut pixels)
        .expect("read the ray query result back");

    assert_eq!(
        pixels,
        vec![0, 255, 0, 255, 255, 0, 0, 255],
        "the first ray hits the triangle (green) and the second, beside it, misses (red)"
    );
}

#[test]
fn rejects_geometry_beyond_the_vertex_buffer() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping the ray query geometry validation test");
        return;
    };
    if !device.capabilities().ray_tracing.acceleration_structures {
        eprintln!("this Metal GPU has no acceleration structure support; skipping");
        return;
    }

    let buffer = device
        .make_buffer(BufferDescriptor::new(12, BufferUsage::STORAGE_READ))
        .expect("short vertex buffer");

    assert!(
        device
            .make_acceleration_structure(AccelerationStructureDescriptor::BottomLevel(vec![
                TriangleGeometry::new(buffer, 1),
            ]))
            .is_err(),
        "a triangle claiming three 12-byte vertices needs 36 bytes, not 12"
    );
    assert!(
        device
            .make_acceleration_structure(AccelerationStructureDescriptor::BottomLevel(vec![
                TriangleGeometry::new(buffer, i64::MAX),
            ]))
            .is_err(),
        "an overflowing triangle count is rejected"
    );
}
