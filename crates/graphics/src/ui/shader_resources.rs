use crate::rhi::*;
use std::sync::Arc;
pub(super) struct NativeUIShaderResources {
    pub device: Arc<Device>,
    pub vertex: ShaderModule,
    pub fragment: ShaderModule,
    pub bindings: BindingLayout,
    pub layout: PipelineLayout,
    pub sampler: Sampler,
    pub constants: PushConstantRange,
}
impl NativeUIShaderResources {
    pub fn new(device: Arc<Device>, backdrop: bool) -> RhiResult<Arc<Self>> {
        let name = if backdrop { "ui_backdrop" } else { "ui" };
        let va = super::resources::artifact(device.backend_api(), name, ShaderStage::Vertex)?;
        let fa = super::resources::artifact(device.backend_api(), name, ShaderStage::Fragment)?;
        let (stage, bytes) = if backdrop {
            (ShaderStage::Fragment, 64)
        } else {
            (ShaderStage::Vertex, 16)
        };
        let ranges: Vec<_> = va
            .interface
            .push_constants
            .iter()
            .chain(&fa.interface.push_constants)
            .copied()
            .collect();
        if ranges.len() != 1 || ranges[0].stage != stage || ranges[0].byte_count != bytes {
            return Err(RhiError::layout_mismatch(format!(
                "{name} requires one {bytes}-byte {stage:?} constant block"
            )));
        }
        let constants = ranges[0];
        let api = device.backend_api();
        let vertex = make_module(&device, &va, api, name, ShaderStage::Vertex)?;
        let fragment = match make_module(&device, &fa, api, name, ShaderStage::Fragment) {
            Ok(value) => value,
            Err(error) => {
                device.destroy_shader_module(vertex);
                return Err(error);
            }
        };
        let remainder = (|| {
            let bindings = device.make_binding_layout(reflected_binding_layout(&[va, fa])?)?;
            let mut ld = PipelineLayoutDescriptor::new(vec![bindings]);
            ld.push_constants = vec![constants];
            let layout = device.make_pipeline_layout(ld)?;
            let sampler = device.make_sampler(SamplerDescriptor {
                min_filter: SamplerFilter::Linear,
                mag_filter: SamplerFilter::Linear,
                mip_filter: SamplerMipFilter::Nearest,
                address_mode_u: SamplerAddressMode::ClampToEdge,
                address_mode_v: SamplerAddressMode::ClampToEdge,
                ..Default::default()
            })?;
            Ok((bindings, layout, sampler))
        })();
        let (bindings, layout, sampler) = match remainder {
            Ok(value) => value,
            Err(error) => {
                device.destroy_shader_module(vertex);
                device.destroy_shader_module(fragment);
                return Err(error);
            }
        };
        Ok(Arc::new(Self {
            device,
            vertex,
            fragment,
            bindings,
            layout,
            sampler,
            constants,
        }))
    }
    pub fn binding(&self, texture: Texture) -> RhiResult<BindingSet> {
        self.device.make_binding_set(
            self.bindings,
            BindingSetDescriptor::new(vec![
                BindingSetEntry::new(1, BindingResource::Texture(texture)),
                BindingSetEntry::new(2, BindingResource::Sampler(self.sampler)),
            ]),
        )
    }
}
impl Drop for NativeUIShaderResources {
    fn drop(&mut self) {
        self.device.destroy_shader_module(self.vertex);
        self.device.destroy_shader_module(self.fragment);
        self.device.destroy_sampler(self.sampler);
    }
}
pub(super) struct NativeUIPipeline {
    pub shaders: Arc<NativeUIShaderResources>,
    pub format: TextureFormat,
    pub samples: usize,
    pub handle: GraphicsPipeline,
}
impl NativeUIPipeline {
    pub fn new(
        shaders: Arc<NativeUIShaderResources>,
        format: TextureFormat,
        samples: usize,
    ) -> RhiResult<Arc<Self>> {
        if !matches!(
            format,
            TextureFormat::Rgba8Unorm
                | TextureFormat::Bgra8Unorm
                | TextureFormat::Rgba8UnormSrgb
                | TextureFormat::Bgra8UnormSrgb
                | TextureFormat::Rgba16Float
        ) {
            return Err(RhiError::unsupported_feature(
                "unsupported UI color target format",
            ));
        }
        let handle = make_pipeline(&shaders, format, samples, false, true, "native-ui")?;
        Ok(Arc::new(Self {
            shaders,
            format,
            samples,
            handle,
        }))
    }
    pub fn srgb(&self) -> f32 {
        if matches!(
            self.format,
            TextureFormat::Rgba8UnormSrgb | TextureFormat::Bgra8UnormSrgb
        ) {
            1.0
        } else {
            0.0
        }
    }
}
impl Drop for NativeUIPipeline {
    fn drop(&mut self) {
        self.shaders.device.destroy_graphics_pipeline(self.handle);
    }
}
pub(super) fn make_pipeline(
    shaders: &NativeUIShaderResources,
    format: TextureFormat,
    samples: usize,
    backdrop: bool,
    blend: bool,
    label: &'static str,
) -> RhiResult<GraphicsPipeline> {
    let mut descriptor = GraphicsPipelineDescriptor::new(shaders.layout, shaders.vertex);
    descriptor.fragment = Some(shaders.fragment);
    descriptor.color_attachments = vec![if blend {
        ColorAttachmentDescriptor::blended(format)
    } else {
        ColorAttachmentDescriptor::new(format)
    }];
    descriptor.depth_format = None;
    descriptor.depth_stencil = None;
    descriptor.rasterization.cull_mode = CullMode::None;
    descriptor.sample_count = samples;
    descriptor.label = Some(label);
    let mut attributes = vec![
        VertexAttribute::new(0, VertexFormat::Float2, 0),
        VertexAttribute::new(1, VertexFormat::Float2, 8),
    ];
    if !backdrop {
        attributes.push(VertexAttribute::new(2, VertexFormat::Unorm8x4, 16));
    }
    descriptor.vertex_layout = Some(VertexLayoutDescriptor::new(
        attributes,
        vec![VertexBufferLayout::new(if backdrop { 16 } else { 20 })],
    ));
    shaders.device.make_graphics_pipeline(descriptor)
}

/// Creates a module from the preferred artifact. A precompiled Metal library
/// built for a newer OS than the running one refuses to load ("deployment
/// target ... not supported on this device"); the MSL source it was built
/// from, carrying the same fingerprint and interface, compiles here instead.
fn make_module(
    device: &Device,
    artifact: &ShaderArtifact,
    api: GraphicsApi,
    name: &str,
    stage: ShaderStage,
) -> RhiResult<ShaderModule> {
    match artifact
        .module_descriptor(&[])
        .and_then(|descriptor| device.make_shader_module(descriptor))
    {
        Ok(module) => Ok(module),
        Err(error) if artifact.format == ShaderFormat::Metallib => {
            let source = super::resources::source_artifact(api, name, stage)?;
            source
                .module_descriptor(&[])
                .and_then(|descriptor| device.make_shader_module(descriptor))
                .map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "ios")))]
mod tests {
    use super::*;

    #[test]
    fn an_unloadable_library_falls_back_to_its_msl_source() {
        let Ok(device) = Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            enable_validation: false,
            ..DeviceConfig::default()
        }) else {
            eprintln!("no Metal device available; skipping");
            return;
        };
        let mut broken = super::super::resources::source_artifact(
            GraphicsApi::Metal,
            "ui",
            ShaderStage::Fragment,
        )
        .expect("the MSL source artifact");
        // Stands in for a library built for a newer OS than this one.
        broken.format = ShaderFormat::Metallib;
        broken.code = b"not a metallib".to_vec();
        let module = make_module(
            &device,
            &broken,
            GraphicsApi::Metal,
            "ui",
            ShaderStage::Fragment,
        )
        .expect("the MSL source builds the module");
        device.destroy_shader_module(module);
    }
}
