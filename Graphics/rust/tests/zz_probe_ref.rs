#[test]
fn probe_shader_refcount() {
    use cove_graphics::rhi::*;
    let device = Device::make(DeviceConfig { preferred_backends: vec![GraphicsApi::Metal], enable_validation: false, ..Default::default() }).unwrap();
    let shader = device.make_shader_module(ShaderModuleDescriptor::new(ShaderStage::Vertex, ShaderFormat::MslSource, b"#include <metal_stdlib>\nvertex float4 v(uint i [[vertex_id]]) { return float4(0); }".to_vec()).with_entry_point("v")).unwrap();
    println!("after make: live_shaders={}", device.pipeline_statistics().live_shader_modules);
    let layout = device.make_pipeline_layout(PipelineLayoutDescriptor::new(vec![])).unwrap();
    let mut d = GraphicsPipelineDescriptor::new(layout, shader);
    d.color_attachments = vec![ColorAttachmentDescriptor::new(TextureFormat::Bgra8Unorm)];
    d.depth_format = None; d.depth_stencil = None;
    let p = device.make_graphics_pipeline(d).unwrap();
    println!("after pipeline: live_shaders={} live_pipelines={}", device.pipeline_statistics().live_shader_modules, device.pipeline_statistics().live_pipelines());
    device.destroy_shader_module(shader);
    println!("after destroy_shader: live_shaders={} live_pipelines={}", device.pipeline_statistics().live_shader_modules, device.pipeline_statistics().live_pipelines());
    device.destroy_graphics_pipeline(p);
    device.wait_until_idle().unwrap();
    println!("after destroy_pipeline: live_shaders={} live_pipelines={}", device.pipeline_statistics().live_shader_modules, device.pipeline_statistics().live_pipelines());
}
