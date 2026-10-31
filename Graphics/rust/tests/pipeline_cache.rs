//! Translation of Graphics/Tests/NativeRHITests/PipelineCacheTests.swift.
//!
//! Backend-call interning and refcounting without a GPU: the recording backend
//! observes exactly which native pipelines the frontend asked for, so the suite
//! can assert that identical state reuses one pipeline and that only the last
//! release retires it.

mod support;

use cove_graphics::{
    BindingLayoutDescriptor, ColorAttachmentDescriptor, ComputePipelineDescriptor, Device,
    GraphicsApi, GraphicsPipelineDescriptor, MeshPipelineDescriptor, ShaderStage, TextureFormat,
};

use support::{RecordingBackend, make_device};

fn graphics_descriptor(device: &Device) -> GraphicsPipelineDescriptor {
    let layout = support::empty_layout(device);
    let vertex = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Vertex,
            &[1, 2, 3, 4],
        ))
        .expect("vertex shader");
    let fragment = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Fragment,
            &[1, 2, 3, 4],
        ))
        .expect("fragment shader");
    let mut descriptor = GraphicsPipelineDescriptor::new(layout, vertex);
    descriptor.fragment = Some(fragment);
    descriptor.color_attachments = vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
    descriptor.depth_format = None;
    descriptor.depth_stencil = None;
    descriptor
}

#[test]
fn graphics_pipelines_intern_and_ref_count() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let descriptor = graphics_descriptor(&device);

    let first = device
        .make_graphics_pipeline(descriptor.clone())
        .expect("first pipeline");
    // The label is not part of the pipeline key.
    let mut relabeled = descriptor.clone();
    relabeled.label = Some("label does not affect state");
    let second = device
        .make_graphics_pipeline(relabeled)
        .expect("second pipeline");

    assert_eq!(first, second, "identical state interns to one handle");
    assert_eq!(
        backend.lock().created_graphics,
        vec![first],
        "one backend compile"
    );

    let statistics = device.pipeline_statistics();
    assert_eq!(statistics.graphics.cold.count, 1);
    assert_eq!(statistics.graphics.cold.during_frame, 0);
    assert_eq!(statistics.graphics.warm_count, 1);
    assert_eq!(statistics.graphics.live, 1);

    device.destroy_graphics_pipeline(first);
    assert!(
        backend.lock().destroyed_graphics.is_empty(),
        "the second lease keeps it alive"
    );
    assert_eq!(device.pipeline_statistics().graphics.live, 1);

    device.destroy_graphics_pipeline(second);
    assert_eq!(
        backend.lock().destroyed_graphics,
        vec![first],
        "the last release retires it"
    );
    assert_eq!(device.pipeline_statistics().graphics.live, 0);

    let recreated = device
        .make_graphics_pipeline(descriptor)
        .expect("recreated pipeline");
    assert_ne!(recreated, first, "a retired pipeline is not resurrected");
    assert_eq!(backend.lock().created_graphics.len(), 2);
    assert_eq!(device.pipeline_statistics().graphics.cold.count, 2);
    device.destroy_graphics_pipeline(recreated);
}

#[test]
fn different_graphics_state_gets_its_own_pipeline() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let descriptor = graphics_descriptor(&device);

    let single_sample = device
        .make_graphics_pipeline(descriptor.clone())
        .expect("single sample");
    let mut multisampled = descriptor;
    multisampled.sample_count = 2;
    let two_samples = device
        .make_graphics_pipeline(multisampled)
        .expect("two samples");

    assert_ne!(
        single_sample, two_samples,
        "sample count changes the native pipeline"
    );
    assert_eq!(backend.lock().created_graphics.len(), 2);
    assert_eq!(device.pipeline_statistics().graphics.live, 2);
    device.destroy_graphics_pipeline(single_sample);
    device.destroy_graphics_pipeline(two_samples);
}

#[test]
fn compute_pipelines_intern_and_ref_count() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let layout = support::empty_layout(&device);
    let shader = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Compute,
            &[1, 2, 3, 4],
        ))
        .expect("compute shader");
    let descriptor = ComputePipelineDescriptor::new(layout, shader);

    let first = device
        .make_compute_pipeline(descriptor.clone())
        .expect("first compute");
    let second = device
        .make_compute_pipeline(descriptor)
        .expect("second compute");
    assert_eq!(first, second);
    assert_eq!(backend.lock().created_compute, vec![first]);

    device.destroy_compute_pipeline(first);
    assert!(
        backend.lock().destroyed_compute.is_empty(),
        "still leased once"
    );
    device.destroy_compute_pipeline(second);
    assert_eq!(backend.lock().destroyed_compute, vec![first]);
    assert_eq!(device.pipeline_statistics().compute.live, 0);
}

#[test]
fn mesh_pipelines_intern_and_ref_count() {
    let backend = RecordingBackend::with_mesh_shading();
    let device = make_device(&backend, 2);
    let layout = support::empty_layout(&device);
    let mesh = device
        .make_shader_module(support::shader_descriptor(ShaderStage::Mesh, &[1, 2, 3, 4]))
        .expect("mesh shader");
    let fragment = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Fragment,
            &[1, 2, 3, 4],
        ))
        .expect("fragment shader");
    let mut descriptor = MeshPipelineDescriptor::new(layout, mesh);
    descriptor.fragment = Some(fragment);

    let first = device
        .make_mesh_pipeline(descriptor.clone())
        .expect("first mesh");
    let second = device.make_mesh_pipeline(descriptor).expect("second mesh");
    assert_eq!(first, second);
    assert_eq!(backend.lock().created_mesh, vec![first]);
    assert_eq!(device.pipeline_statistics().mesh.live, 1);

    device.destroy_mesh_pipeline(first);
    assert!(
        backend.lock().destroyed_mesh.is_empty(),
        "still leased once"
    );
    device.destroy_mesh_pipeline(second);
    assert_eq!(backend.lock().destroyed_mesh, vec![first]);
    assert_eq!(device.pipeline_statistics().mesh.live, 0);
}

#[test]
fn shader_module_creations_are_measured() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let vertex = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Vertex,
            &[1, 2, 3, 4],
        ))
        .expect("vertex shader");
    let fragment = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Fragment,
            &[1, 2, 3, 4],
        ))
        .expect("fragment shader");

    assert_eq!(
        backend.lock().created_shader_modules,
        vec![vertex, fragment]
    );
    assert_eq!(device.pipeline_statistics().shader_modules.count, 2);
    device.reset_pipeline_statistics();
    assert_eq!(device.pipeline_statistics().shader_modules.count, 0);
    device.destroy_shader_module(vertex);
    device.destroy_shader_module(fragment);
}

#[test]
fn creation_during_frame_is_counted() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let descriptor = graphics_descriptor(&device);

    device.begin_frame().expect("begin frame");
    let pipeline = device.make_graphics_pipeline(descriptor).expect("pipeline");
    let module = device
        .make_shader_module(support::shader_descriptor(ShaderStage::Vertex, &[5, 6]))
        .expect("shader module");
    device.end_frame();

    let statistics = device.pipeline_statistics();
    assert_eq!(statistics.graphics.cold.during_frame, 1);
    assert_eq!(statistics.shader_modules.during_frame, 1);
    assert_eq!(statistics.creations_during_frame(), 2);
    device.destroy_graphics_pipeline(pipeline);
    device.destroy_shader_module(module);
}

#[test]
fn shader_modules_share_content_and_release_their_own_leases() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let descriptor = support::shader_descriptor(ShaderStage::Vertex, &[1, 2, 3, 4]);
    let first = device
        .make_shader_module(descriptor.clone())
        .expect("first shader");
    let second = device
        .make_shader_module(descriptor)
        .expect("second shader");

    assert_eq!(first, second, "identical source intern to one module");
    assert_eq!(backend.lock().created_shader_modules.len(), 1);
    assert_eq!(device.pipeline_statistics().shader_module_cache_hits, 1);

    device.destroy_shader_module(first);
    assert_eq!(
        device.pipeline_statistics().live_shader_modules,
        1,
        "one lease remains"
    );
    device.destroy_shader_module(second);
    assert_eq!(device.pipeline_statistics().live_shader_modules, 0);

    let recreated = device
        .make_shader_module(support::shader_descriptor(
            ShaderStage::Vertex,
            &[1, 2, 3, 4],
        ))
        .expect("recreated shader");
    assert_ne!(
        first, recreated,
        "production allocates a fresh handle after retirement"
    );
    device.destroy_shader_module(recreated);
}

#[test]
fn reset_clears_counters_but_keeps_interned_pipelines() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let descriptor = graphics_descriptor(&device);
    let pipeline = device
        .make_graphics_pipeline(descriptor.clone())
        .expect("pipeline");

    device.reset_pipeline_statistics();
    let statistics = device.pipeline_statistics();
    assert_eq!(statistics.graphics.cold.count, 0);
    assert_eq!(statistics.graphics.warm_count, 0);
    assert_eq!(
        statistics.graphics.live, 1,
        "interning outlives the counters"
    );

    let reused = device
        .make_graphics_pipeline(descriptor)
        .expect("reused pipeline");
    assert_eq!(
        backend.lock().created_graphics.len(),
        1,
        "still one backend compile"
    );
    assert_eq!(device.pipeline_statistics().graphics.warm_count, 1);
    device.destroy_graphics_pipeline(pipeline);
    device.destroy_graphics_pipeline(reused);
}

/// The key itself ignores the label, which is what makes relabelling a cache hit.
#[test]
fn pipeline_key_excludes_the_label() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let mut first = graphics_descriptor(&device);
    first.label = Some("first");
    let mut second = first.clone();
    second.label = Some("second");
    assert_eq!(
        support::graphics_key(&first),
        support::graphics_key(&second)
    );
}

/// A backend without mesh shading reports the feature as unavailable rather than
/// silently accepting the descriptor.
#[test]
fn mesh_shading_requires_the_capability() {
    let backend = RecordingBackend::standard();
    let device = make_device(&backend, 2);
    let descriptor = support::shader_descriptor(ShaderStage::Mesh, &[1, 2, 3, 4]);
    assert!(
        device.make_shader_module(descriptor).is_err(),
        "a backend without mesh shading must refuse a mesh shader module"
    );

    // The same source is accepted once the backend advertises the capability.
    let capable = RecordingBackend::with_mesh_shading();
    let device = make_device(&capable, 2);
    let mesh = device
        .make_shader_module(support::shader_descriptor(ShaderStage::Mesh, &[1, 2, 3, 4]))
        .expect("mesh shader with the capability");
    let layout = support::empty_layout(&device);
    let pipeline = device
        .make_mesh_pipeline(MeshPipelineDescriptor::new(layout, mesh))
        .expect("mesh pipeline");
    assert_eq!(capable.lock().created_mesh, vec![pipeline]);
    device.destroy_mesh_pipeline(pipeline);
    device.destroy_shader_module(mesh);
    let _ = BindingLayoutDescriptor::new(Vec::new());
    let _ = GraphicsApi::Metal;
}
