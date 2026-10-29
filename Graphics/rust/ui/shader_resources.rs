use std::sync::Arc;
use crate::rhi::*;
pub(super) struct NativeUIShaderResources{pub device:Arc<Device>,pub vertex:ShaderModule,pub fragment:ShaderModule,pub bindings:BindingLayout,pub layout:PipelineLayout,pub sampler:Sampler,pub constants:PushConstantRange}
impl NativeUIShaderResources{
    pub fn new(device:Arc<Device>,backdrop:bool)->RhiResult<Arc<Self>>{
        let name=if backdrop{"ui_backdrop"}else{"ui"};let va=super::resources::artifact(device.backend_api(),name,ShaderStage::Vertex)?;let fa=super::resources::artifact(device.backend_api(),name,ShaderStage::Fragment)?;
        let(stage,bytes)=if backdrop{(ShaderStage::Fragment,64)}else{(ShaderStage::Vertex,16)};
        let ranges:Vec<_>=va.interface.push_constants.iter().chain(&fa.interface.push_constants).copied().collect();
        if ranges.len()!=1||ranges[0].stage!=stage||ranges[0].byte_count!=bytes{return Err(RhiError::layout_mismatch(format!("{name} requires one {bytes}-byte {stage:?} constant block")));}
        let constants=ranges[0];let vertex=device.make_shader_module(va.module_descriptor(&[])?)?;
        let fragment=match fa.module_descriptor(&[]).and_then(|d|device.make_shader_module(d)){Ok(value)=>value,Err(error)=>{device.destroy_shader_module(vertex);return Err(error);}};
        let remainder=(||{let bindings=device.make_binding_layout(reflected_binding_layout(&[va,fa])?)?;let mut ld=PipelineLayoutDescriptor::new(vec![bindings]);ld.push_constants=vec![constants];let layout=device.make_pipeline_layout(ld)?;
            let sampler=device.make_sampler(SamplerDescriptor{min_filter:SamplerFilter::Linear,mag_filter:SamplerFilter::Linear,mip_filter:SamplerMipFilter::Nearest,address_mode_u:SamplerAddressMode::ClampToEdge,address_mode_v:SamplerAddressMode::ClampToEdge,..Default::default()})?;Ok((bindings,layout,sampler))})();
        let(bindings,layout,sampler)=match remainder{Ok(value)=>value,Err(error)=>{device.destroy_shader_module(vertex);device.destroy_shader_module(fragment);return Err(error);}};
        Ok(Arc::new(Self{device,vertex,fragment,bindings,layout,sampler,constants}))
    }
    pub fn binding(&self,texture:Texture)->RhiResult<BindingSet>{self.device.make_binding_set(self.bindings,BindingSetDescriptor::new(vec![BindingSetEntry::new(1,BindingResource::Texture(texture)),BindingSetEntry::new(2,BindingResource::Sampler(self.sampler))]))}
}
impl Drop for NativeUIShaderResources{fn drop(&mut self){self.device.destroy_shader_module(self.vertex);self.device.destroy_shader_module(self.fragment);self.device.destroy_sampler(self.sampler);}}
pub(super) struct NativeUIPipeline{pub shaders:Arc<NativeUIShaderResources>,pub format:TextureFormat,pub samples:usize,pub handle:GraphicsPipeline}
impl NativeUIPipeline{
    pub fn new(shaders:Arc<NativeUIShaderResources>,format:TextureFormat,samples:usize)->RhiResult<Arc<Self>>{
        if !matches!(format,TextureFormat::Rgba8Unorm|TextureFormat::Bgra8Unorm|TextureFormat::Rgba8UnormSrgb|TextureFormat::Bgra8UnormSrgb|TextureFormat::Rgba16Float){return Err(RhiError::unsupported_feature("unsupported UI color target format"));}
        let handle=make_pipeline(&shaders,format,samples,false,true,"native-ui")?;Ok(Arc::new(Self{shaders,format,samples,handle}))
    }
    pub fn srgb(&self)->f32{if matches!(self.format,TextureFormat::Rgba8UnormSrgb|TextureFormat::Bgra8UnormSrgb){1.0}else{0.0}}
}
impl Drop for NativeUIPipeline{fn drop(&mut self){self.shaders.device.destroy_graphics_pipeline(self.handle);}}
pub(super) fn make_pipeline(shaders:&NativeUIShaderResources,format:TextureFormat,samples:usize,backdrop:bool,blend:bool,label:&'static str)->RhiResult<GraphicsPipeline>{
    let mut descriptor=GraphicsPipelineDescriptor::new(shaders.layout,shaders.vertex);descriptor.fragment=Some(shaders.fragment);descriptor.color_attachments=vec![if blend{ColorAttachmentDescriptor::blended(format)}else{ColorAttachmentDescriptor::new(format)}];descriptor.depth_format=None;descriptor.depth_stencil=None;descriptor.rasterization.cull_mode=CullMode::None;descriptor.sample_count=samples;descriptor.label=Some(label);
    let mut attributes=vec![VertexAttribute::new(0,VertexFormat::Float2,0),VertexAttribute::new(1,VertexFormat::Float2,8)];if !backdrop{attributes.push(VertexAttribute::new(2,VertexFormat::Unorm8x4,16));}
    descriptor.vertex_layout=Some(VertexLayoutDescriptor::new(attributes,vec![VertexBufferLayout::new(if backdrop{16}else{20})]));shaders.device.make_graphics_pipeline(descriptor)
}
