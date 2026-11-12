use cove_graphics::rhi::*;

use super::scene::{
    FLOOR_MIN, GRID, GRID_STEP, LIGHT_SAMPLES, ShadowMask, Triangle, light_samples,
};

const WORKGROUP: usize = 8;
const CONSTANT_BYTES: usize = 256;
const MASK_BYTES: usize = GRID * GRID * size_of::<u32>();
const _: () = assert!(GRID.is_multiple_of(WORKGROUP));

// This offscreen device belongs to the preview worker. Its handles and readback
// never participate in the host's swapchain frame or share its resource caches.
pub(super) struct RayQuery {
    device: Device,
    pipeline: ComputePipeline,
    bindings: BindingSet,
    constants: Buffer,
    mask: Buffer,
}

impl RayQuery {
    pub fn available(geometry: &[Triangle]) -> Option<Self> {
        for api in DeviceConfig::default().preferred_backends {
            let Ok(device) = make_device(api) else {
                continue;
            };
            if device.capabilities().ray_tracing.compute_ray_query {
                match Self::new(device, geometry) {
                    Ok(scene) => return Some(scene),
                    Err(error) => eprintln!("{} shadow preview: {error}", api.display_name()),
                }
            }
        }
        None
    }

    fn new(device: Device, geometry: &[Triangle]) -> RhiResult<Self> {
        let bytes: Vec<_> = geometry
            .iter()
            .flat_map(|triangle| triangle.points.into_iter().flatten())
            .flat_map(f32::to_le_bytes)
            .collect();
        let vertices = device.make_buffer(BufferDescriptor::new(
            bytes.len(),
            BufferUsage::VERTEX | BufferUsage::TRANSFER_DESTINATION,
        ))?;
        device.upload_buffer_data(vertices, 0, &bytes)?;
        let blas =
            device.make_acceleration_structure(AccelerationStructureDescriptor::BottomLevel(
                vec![TriangleGeometry::new(vertices, geometry.len() as i64)],
            ))?;
        let tlas =
            device.make_acceleration_structure(AccelerationStructureDescriptor::TopLevel(vec![
                AccelerationInstance::new(blas),
            ]))?;

        let shader = device.make_shader_module(shader(device.backend_api())?)?;
        let constants = device.make_buffer(BufferDescriptor::new(
            CONSTANT_BYTES,
            BufferUsage::UNIFORM | BufferUsage::TRANSFER_DESTINATION,
        ))?;
        let mask = device.make_buffer(BufferDescriptor::new(
            MASK_BYTES,
            BufferUsage::STORAGE_WRITE | BufferUsage::TRANSFER_SOURCE,
        ))?;
        let binding_layout = device.make_binding_layout(BindingLayoutDescriptor::new(vec![
            BindingLayoutEntry::new(0, BindingType::StorageBuffer, ShaderVisibility::COMPUTE),
            BindingLayoutEntry::new(
                1,
                BindingType::AccelerationStructure,
                ShaderVisibility::COMPUTE,
            ),
            BindingLayoutEntry::new(2, BindingType::UniformBuffer, ShaderVisibility::COMPUTE),
        ]))?;
        let bindings = device.make_binding_set(
            binding_layout,
            BindingSetDescriptor::new(vec![
                BindingSetEntry::new(0, BindingResource::storage_buffer(mask)),
                BindingSetEntry::new(1, BindingResource::AccelerationStructure(tlas)),
                BindingSetEntry::new(2, BindingResource::uniform_buffer(constants)),
            ]),
        )?;
        let layout =
            device.make_pipeline_layout(PipelineLayoutDescriptor::new(vec![binding_layout]))?;
        let pipeline =
            device.make_compute_pipeline(ComputePipelineDescriptor::new(layout, shader))?;

        let mut commands = CommandBuffer::new();
        device.record_build(blas, &mut commands)?;
        device.record_build(tlas, &mut commands)?;
        submit(&device, &commands)?;
        Ok(Self {
            device,
            pipeline,
            bindings,
            constants,
            mask,
        })
    }

    pub fn api(&self) -> GraphicsApi {
        self.device.backend_api()
    }

    pub fn trace(&self, light: u8) -> RhiResult<ShadowMask> {
        let grid = [FLOOR_MIN, GRID_STEP, GRID as f32, 0.0];
        let values = light_samples(light)
            .into_iter()
            .flat_map(|[x, y, z]| [x, y, z, 0.0])
            .chain(grid)
            .flat_map(f32::to_le_bytes);
        let mut bytes = [0; CONSTANT_BYTES];
        for (byte, value) in bytes.iter_mut().zip(values) {
            *byte = value;
        }
        self.device.upload_buffer_data(self.constants, 0, &bytes)?;

        let groups = (GRID / WORKGROUP) as i64;
        let mut commands = CommandBuffer::new();
        commands.compute_pass(|pass| {
            pass.set_pipeline(self.pipeline);
            pass.set_binding_set(self.bindings, 0, Vec::new());
            pass.dispatch(groups, groups, 1);
        });
        submit(&self.device, &commands)?;

        let mut bytes = vec![0; MASK_BYTES];
        self.device.read_buffer_data(self.mask, 0, &mut bytes)?;
        bytes
            .chunks_exact(4)
            .map(|word| {
                let count = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
                if count > LIGHT_SAMPLES as u32 {
                    return Err(RhiError::invalid_argument("invalid shadow sample count"));
                }
                Ok(count as u8)
            })
            .collect()
    }
}

fn make_device(api: GraphicsApi) -> RhiResult<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![api],
        frames_in_flight: 1,
        ..DeviceConfig::default()
    })
}

fn submit(device: &Device, commands: &CommandBuffer) -> RhiResult<()> {
    device.begin_frame()?;
    let result = device.submit(commands, QueueClass::Compute, &[], &[], None);
    device.end_frame();
    result?;
    device.wait_until_idle()
}

fn shader(api: GraphicsApi) -> RhiResult<ShaderModuleDescriptor> {
    let (format, code): (_, &[u8]) = match api {
        GraphicsApi::Dx12 => (ShaderFormat::Dxil, include_bytes!("shadows.dxil")),
        GraphicsApi::Vulkan => (ShaderFormat::Spirv, include_bytes!("shadows.spv")),
        GraphicsApi::Metal => (ShaderFormat::MslSource, include_bytes!("shadows.metal")),
        GraphicsApi::WebGpu => return Err(RhiError::unsupported_feature("WebGPU ray queries")),
    };
    let mut descriptor = ShaderModuleDescriptor::new(ShaderStage::Compute, format, code)
        .with_entry_point("shadowMain");
    descriptor.threadgroup_size = ThreadgroupSize::new(WORKGROUP as i64, WORKGROUP as i64, 1);
    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gallery_content::stories::ray_traced_shadows::scene::{
        occluders, raster_shadow_mask,
    };

    fn compare_masks(api: GraphicsApi) {
        let device = make_device(api).and_then(|device| {
            if device.capabilities().ray_tracing.compute_ray_query {
                Ok(device)
            } else {
                Err(RhiError::unsupported_feature(format!(
                    "{} has no compute ray query",
                    device.device_name()
                )))
            }
        });
        let device = match device {
            Ok(device) => device,
            Err(error) if std::env::var_os("COVE_REQUIRE_RAY_QUERY").is_none() => {
                eprintln!("Skipping {api:?} ray queries: {error}");
                return;
            }
            Err(error) => panic!("{api:?} ray queries required: {error}"),
        };
        eprintln!("{api:?} shadow masks on {}", device.device_name());
        let geometry = occluders();
        let ray_query = RayQuery::new(device, &geometry).unwrap();
        let mut previous = None;
        for light in [0, 8, 16] {
            let traced = ray_query.trace(light).unwrap();
            let raster = raster_shadow_mask(&geometry, light);
            assert!(traced.contains(&0) && traced.contains(&(LIGHT_SAMPLES as u8)));
            assert!(
                traced
                    .iter()
                    .any(|&value| value > 0 && value < LIGHT_SAMPLES as u8)
            );
            assert_ne!(previous.as_ref(), Some(&traced));
            let differing = traced.iter().zip(&raster).filter(|(a, b)| a != b).count();
            assert!(
                differing < GRID * GRID / 50,
                "{differing} texels differ at light position {light}"
            );
            previous = Some(traced);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn dx12_shadow_masks() {
        compare_masks(GraphicsApi::Dx12);
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "android"))]
    #[test]
    fn vulkan_shadow_masks() {
        compare_masks(GraphicsApi::Vulkan);
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn metal_shadow_masks() {
        compare_masks(GraphicsApi::Metal);
    }
}
