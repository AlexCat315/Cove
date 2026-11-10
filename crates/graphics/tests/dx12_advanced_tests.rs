#![cfg(target_os = "windows")]

use cove_graphics::rhi::*;
use std::path::Path;
use std::process::Command;

fn prerequisites() -> Option<(Device, std::ffi::OsString)> {
    let compiler = std::env::var_os("DXC").unwrap_or_else(|| "dxc.exe".into());
    if Command::new(&compiler).arg("-help").output().is_err() {
        eprintln!("DXC is unavailable; set DXC to the Windows SDK compiler");
        return None;
    }
    let device = match Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Dx12],
        ..DeviceConfig::default()
    }) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("DX12 device unavailable: {error}");
            return None;
        }
    };
    eprintln!(
        "DX12 adapter: {} {:?}",
        device.device_name(),
        device.adapter_capabilities()
    );
    Some((device, compiler))
}

fn shader(
    device: &Device,
    dxc: &std::ffi::OsStr,
    entry: &str,
    target: &str,
    stage: ShaderStage,
) -> ShaderModule {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dx12_advanced.hlsl");
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let output =
        std::env::temp_dir().join(format!("cove-{}-{id}-{entry}.dxil", std::process::id()));
    let compile = Command::new(dxc)
        .args(["-E", entry, "-T", target, "-Fo"])
        .arg(&output)
        .arg(fixture)
        .output()
        .expect("run DXC");
    assert!(
        compile.status.success(),
        "DXC {entry}: {}{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    let code = std::fs::read(&output).expect("read DXIL");
    std::fs::remove_file(output).expect("remove compiled test fixture");
    device
        .make_shader_module(
            ShaderModuleDescriptor::new(stage, ShaderFormat::Dxil, code).with_entry_point(entry),
        )
        .expect("shader module")
}

fn render(device: &Device, draw: impl FnOnce(&mut RenderPassEncoder)) {
    let texture = device
        .make_texture(TextureDescriptor::new(
            32,
            32,
            TextureFormat::Rgba8Unorm,
            TextureUsage::COLOR_TARGET | TextureUsage::TRANSFER_SOURCE,
        ))
        .expect("render target");
    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(
            texture,
            [0.0, 0.0, 0.0, 1.0],
        )]),
        draw,
    );
    submit(device, &commands, QueueClass::Graphics);
    let mut pixels = vec![0; 32 * 32 * 4];
    device
        .read_texture_data(texture, 32, 32, 128, Default::default(), &mut pixels)
        .expect("rendered pixels");
    let center = (16 * 32 + 16) * 4;
    assert_eq!(&pixels[center..center + 4], &[0, 255, 0, 255]);
    device.destroy_texture(texture);
}

fn submit(device: &Device, commands: &CommandBuffer, queue: QueueClass) {
    device.begin_frame().expect("frame");
    device
        .submit(commands, queue, &[], &[], None)
        .expect("submit");
    device.end_frame();
    device.wait_until_idle().expect("GPU idle");
}

fn arguments(device: &Device, bytes: &[u32]) -> Buffer {
    let bytes: Vec<_> = bytes.iter().flat_map(|n| n.to_le_bytes()).collect();
    let buffer = device
        .make_buffer(BufferDescriptor::new(
            bytes.len(),
            BufferUsage::INDIRECT | BufferUsage::TRANSFER_DESTINATION,
        ))
        .expect("indirect argument buffer");
    device
        .upload_buffer_data(buffer, 0, &bytes)
        .expect("upload arguments");
    buffer
}

#[test]
fn dx12_executes_indirect_draw_and_dispatch() {
    let Some((device, dxc)) = prerequisites() else {
        return;
    };
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![]))
        .expect("layout");
    let vertex = shader(&device, &dxc, "vertexMain", "vs_6_5", ShaderStage::Vertex);
    let fragment = shader(
        &device,
        &dxc,
        "fragmentMain",
        "ps_6_5",
        ShaderStage::Fragment,
    );
    let mut pipeline = GraphicsPipelineDescriptor::new(layout, vertex);
    pipeline.fragment = Some(fragment);
    pipeline.color_attachments = vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
    let pipeline = device
        .make_graphics_pipeline(pipeline)
        .expect("graphics pipeline");
    // The first argument block draws nothing. The second makes the green triangle.
    let draw = arguments(&device, &[0, 1, 0, 0, 3, 1, 0, 0]);
    render(&device, |pass| {
        pass.set_pipeline(pipeline);
        pass.draw_indirect(draw, 0, 2);
    });

    let compute = shader(&device, &dxc, "computeMain", "cs_6_5", ShaderStage::Compute);
    let (pipeline, set, output) = compute_bindings(&device, compute, None);
    let dispatch = arguments(&device, &[0, 3, 1, 1]);
    let mut commands = CommandBuffer::new();
    commands.compute_pass(|pass| {
        pass.set_pipeline(pipeline);
        pass.set_binding_set(set, 0, Vec::new());
        pass.dispatch_indirect(dispatch, 4);
    });
    submit(&device, &commands, QueueClass::Compute);
    assert_eq!(&read_words(&device, output)[..3], &[7, 8, 9]);
    device.destroy_buffer(draw);
    device.destroy_buffer(dispatch);
    device.destroy_buffer(output);
}

#[test]
fn dx12_rasterizes_a_mesh_shader() {
    let Some((device, dxc)) = prerequisites() else {
        return;
    };
    if !device.capabilities().mesh_shading.mesh {
        eprintln!("adapter does not expose mesh shaders");
        return;
    }
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![]))
        .expect("layout");
    let mesh = shader(&device, &dxc, "meshMain", "ms_6_5", ShaderStage::Mesh);
    let fragment = shader(
        &device,
        &dxc,
        "fragmentMain",
        "ps_6_5",
        ShaderStage::Fragment,
    );
    let mut pipeline = MeshPipelineDescriptor::new(layout, mesh);
    pipeline.fragment = Some(fragment);
    pipeline.color_attachments = vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
    let pipeline = device.make_mesh_pipeline(pipeline).expect("mesh pipeline");
    render(&device, |pass| {
        pass.set_mesh_pipeline(pipeline);
        pass.draw_mesh_tasks(1, 1, 1);
    });
    if device.capabilities().mesh_shading.task {
        let task = shader(&device, &dxc, "taskMain", "as_6_5", ShaderStage::Task);
        let mesh = shader(&device, &dxc, "taskMeshMain", "ms_6_5", ShaderStage::Mesh);
        let mut pipeline = MeshPipelineDescriptor::new(layout, mesh);
        pipeline.task = Some(task);
        pipeline.fragment = Some(fragment);
        pipeline.color_attachments =
            vec![ColorAttachmentDescriptor::new(TextureFormat::Rgba8Unorm)];
        let pipeline = device
            .make_mesh_pipeline(pipeline)
            .expect("mesh pipeline with task shader");
        render(&device, |pass| {
            pass.set_mesh_pipeline(pipeline);
            pass.draw_mesh_tasks(1, 1, 1);
        });
    }
}

#[test]
fn dx12_builds_blas_and_tlas_and_queries_hit_and_miss() {
    let Some((device, dxc)) = prerequisites() else {
        return;
    };
    if !device.capabilities().ray_tracing.acceleration_structures {
        eprintln!("adapter does not expose DXR acceleration structures");
        return;
    }
    let vertices = [-1.0_f32, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0, 1.0, 0.0];
    let bytes: Vec<_> = vertices.into_iter().flat_map(f32::to_le_bytes).collect();
    let vertex_buffer = device
        .make_buffer(BufferDescriptor::new(
            bytes.len(),
            BufferUsage::VERTEX | BufferUsage::TRANSFER_DESTINATION,
        ))
        .expect("triangle buffer");
    device
        .upload_buffer_data(vertex_buffer, 0, &bytes)
        .expect("upload triangle");
    let blas = device
        .make_acceleration_structure(AccelerationStructureDescriptor::BottomLevel(vec![
            TriangleGeometry::new(vertex_buffer, 1),
        ]))
        .expect("BLAS");
    let tlas = device
        .make_acceleration_structure(AccelerationStructureDescriptor::TopLevel(vec![
            AccelerationInstance::new(blas),
        ]))
        .expect("TLAS");
    let mut commands = CommandBuffer::new();
    device
        .record_build(blas, &mut commands)
        .expect("record BLAS");
    device
        .record_build(tlas, &mut commands)
        .expect("record TLAS");
    if device.capabilities().ray_tracing.compute_ray_query {
        let ray = shader(&device, &dxc, "rayMain", "cs_6_5", ShaderStage::Compute);
        let (pipeline, set, output) = compute_bindings(&device, ray, Some(tlas));
        commands.compute_pass(|pass| {
            pass.set_pipeline(pipeline);
            pass.set_binding_set(set, 0, Vec::new());
            pass.dispatch(2, 1, 1);
        });
        submit(&device, &commands, QueueClass::Compute);
        assert_eq!(&read_words(&device, output)[..2], &[1, 0]);
        device.destroy_buffer(output);
    } else {
        submit(&device, &commands, QueueClass::Compute);
        eprintln!("DXR build verified; adapter does not expose inline ray query");
    }
    device.destroy_acceleration_structure(tlas);
    device.destroy_acceleration_structure(blas);
    device.destroy_buffer(vertex_buffer);
}

fn compute_bindings(
    device: &Device,
    shader: ShaderModule,
    scene: Option<AccelerationStructure>,
) -> (ComputePipeline, BindingSet, Buffer) {
    let output = device
        .make_buffer(BufferDescriptor::new(
            16,
            BufferUsage::STORAGE_WRITE | BufferUsage::TRANSFER_SOURCE,
        ))
        .expect("result buffer");
    let mut declarations = vec![BindingLayoutEntry::new(
        0,
        BindingType::StorageBuffer,
        ShaderVisibility::COMPUTE,
    )];
    let mut resources = vec![BindingSetEntry::new(
        0,
        BindingResource::StorageBuffer {
            buffer: output,
            offset: 0,
        },
    )];
    if let Some(scene) = scene {
        declarations.push(BindingLayoutEntry::new(
            1,
            BindingType::AccelerationStructure,
            ShaderVisibility::COMPUTE,
        ));
        resources.push(BindingSetEntry::new(
            1,
            BindingResource::AccelerationStructure(scene),
        ));
    }
    let set_layout = device
        .make_binding_layout(BindingLayoutDescriptor::new(declarations))
        .expect("set layout");
    let layout = device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(vec![set_layout]))
        .expect("compute layout");
    let set = device
        .make_binding_set(set_layout, BindingSetDescriptor::new(resources))
        .expect("compute bindings");
    let pipeline = device
        .make_compute_pipeline(ComputePipelineDescriptor::new(layout, shader))
        .expect("compute pipeline");
    (pipeline, set, output)
}

fn read_words(device: &Device, buffer: Buffer) -> [u32; 4] {
    let mut bytes = [0; 16];
    device
        .read_buffer_data(buffer, 0, &mut bytes)
        .expect("GPU result");
    core::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
}
