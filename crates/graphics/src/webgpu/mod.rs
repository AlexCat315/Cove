//! Browser WebGPU backend. Rust owns RHI descriptors, resources and submission
//! planning. The WASI host translates resource IDs to JavaScript GPU objects.

mod bridge;
mod commands;
mod formats;

pub use bridge::complete;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};

use crate::rhi::*;
use bridge::Host;
use formats::*;

pub(crate) fn make_backend(config: &DeviceConfig) -> RhiResult<Box<dyn RhiBackend>> {
    Ok(Box::new(WebGpu {
        host: Host::open()?,
        buffers: HashMap::new(),
        textures: HashMap::new(),
        swapchains: HashMap::new(),
        signals: HashMap::new(),
        next_internal: Arc::new(std::sync::atomic::AtomicU32::new(u32::MAX - 1)),
        uploads: (0..config.frames_in_flight.max(1))
            .map(|_| Arc::default())
            .collect(),
    }))
}

struct Surface {
    descriptor: SurfaceDescriptor,
    texture: Texture,
    generation: u64,
}

struct WebGpu {
    host: Host,
    buffers: HashMap<u32, BufferDescriptor>,
    textures: HashMap<u32, TextureDescriptor>,
    swapchains: HashMap<u32, Surface>,
    signals: HashMap<u32, u64>,
    next_internal: Arc<std::sync::atomic::AtomicU32>,
    uploads: Vec<Arc<Mutex<UploadSlot>>>,
}

impl WebGpu {
    fn submit_impl(
        &mut self,
        submit: &PlannedSubmit,
        measure_gpu: bool,
        completion: TimedCompletion,
    ) -> RhiResult<()> {
        for wait in &submit.wait_semaphores {
            rhi_require(self.signals.get(&wait.id).copied().unwrap_or(0) >= wait.value,
                "WebGPU timeline waits require a signal already queued on the device")?;
        }
        self.host.queue_timed(
            "submit",
            json!({ "passes": commands::encode(submit)?, "measureGpu": measure_gpu }),
            completion,
        )?;
        for signal in &submit.signal_semaphores {
            let value = self.signals.entry(signal.id).or_default();
            *value = (*value).max(signal.value);
        }
        Ok(())
    }

    fn destroy(&self, kind: &str, id: u32) {
        let _ = self
            .host
            .call("destroy", json!({ "kind": kind, "id": id }), &[]);
    }
}

impl Drop for WebGpu {
    fn drop(&mut self) {
        let _ = self.host.call("close", Value::Null, &[]);
    }
}

impl RhiBackend for WebGpu {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::WebGpu
    }

    fn device_name(&self) -> &str {
        "Browser WebGPU"
    }

    fn query_capabilities(&self) -> Capabilities {
        Capabilities::configure(|caps| {
            caps.graphics = true;
            caps.compute = true;
            caps.indirect_draw = true;
            caps.max_queues = QueueLimits::default();
        })
    }

    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        rhi_require(
            descriptor.kind == NativeSurfaceKind::WebCanvas,
            "WebGPU requires a registered canvas",
        )?;
        rhi_require(
            matches!(
                descriptor.color_format,
                TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
            ),
            "WebGPU canvas needs a browser presentation format",
        )?;
        let id = handle.id();
        self.host.call("configure", json!({
            "id": id, "canvas": descriptor.native_handle,
            "width": descriptor.width, "height": descriptor.height,
            "format": texture_format(descriptor.color_format)?,
            "alphaMode": if descriptor.premultiplied_alpha { "premultiplied" } else { "opaque" },
        }), &[])?;
        let (texture, generation) = self.swapchains.get(&id).map_or_else(
            || (Texture::new(internal_id(&self.next_internal)), 1),
            |surface| (surface.texture, surface.generation + 1),
        );
        self.swapchains.insert(
            id,
            Surface {
                descriptor,
                texture,
                generation,
            },
        );
        Ok(())
    }

    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        rhi_require(
            self.swapchains.contains_key(&handle.id()),
            "unknown WebGPU swapchain",
        )?;
        Ok(PresentationMode::Compositor)
    }

    fn destroy_swapchain(&mut self, handle: Swapchain) {
        self.swapchains.remove(&handle.id());
        self.destroy("surface", handle.id());
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        let surface = self
            .swapchains
            .get(&handle.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown WebGPU swapchain"))?;
        self.host.call(
            "acquire",
            json!({ "id": handle.id(), "texture": surface.texture.id() }),
            &[],
        )?;
        Ok(SwapchainImage::new(
            handle,
            surface.generation,
            surface.texture,
            surface.descriptor.width,
            surface.descriptor.height,
        ))
    }

    fn present(&mut self, image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        self.host.queue(
            "present",
            json!({ "texture": image.texture.id() }),
            completion,
        )
    }

    fn create_buffer(&mut self, handle: Buffer, descriptor: BufferDescriptor) -> RhiResult<()> {
        make_buffer(&self.host, handle, descriptor)?;
        self.buffers.insert(handle.id(), descriptor);
        Ok(())
    }

    fn create_texture(&mut self, handle: Texture, descriptor: TextureDescriptor) -> RhiResult<()> {
        rhi_require(
            matches!(descriptor.sample_count, 1 | 4),
            "WebGPU texture sample count must be 1 or 4",
        )?;
        let dimension = match descriptor.dimension {
            TextureDimension::Texture2D
            | TextureDimension::Texture2DArray
            | TextureDimension::Cube => "2d",
            TextureDimension::Texture3D => "3d",
        };
        let depth = if dimension == "3d" {
            descriptor.depth
        } else {
            descriptor.layers
        };
        self.host.call("texture", json!({ "id": handle.id(), "descriptor": {
            "label": descriptor.label.unwrap_or_default(), "format": texture_format(descriptor.format)?,
            "size": [descriptor.width, descriptor.height, depth], "dimension": dimension,
            "mipLevelCount": descriptor.mip_levels, "sampleCount": descriptor.sample_count,
            "usage": texture_usage(descriptor.usage),
        }}), &[])?;
        self.textures.insert(handle.id(), descriptor);
        Ok(())
    }

    fn create_sampler(&mut self, handle: Sampler, descriptor: SamplerDescriptor) -> RhiResult<()> {
        self.host.call(
            "sampler",
            json!({ "id": handle.id(), "descriptor": sampler(descriptor)? }),
            &[],
        )
    }

    fn create_shader_module(
        &mut self,
        handle: ShaderModule,
        descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        rhi_require(
            descriptor.format == ShaderFormat::Wgsl,
            "WebGPU requires WGSL shader modules",
        )?;
        let code = std::str::from_utf8(&descriptor.code)
            .map_err(|_| RhiError::invalid_argument("WGSL source must be UTF-8"))?;
        let constants: Map<String, Value> = descriptor
            .specialization_constants
            .iter()
            .map(|constant| {
                let value = match constant.value {
                    ShaderConstantValue::Bool(value) => json!(u8::from(value)),
                    ShaderConstantValue::Int32(value) => json!(value),
                    ShaderConstantValue::Uint32(value) => json!(value),
                    ShaderConstantValue::Float32(value) => json!(value),
                };
                (constant.id.to_string(), value)
            })
            .collect();
        self.host.call("shader", json!({ "id": handle.id(), "entryPoint": descriptor.entry_point, "constants": constants }), code.as_bytes())
    }

    fn create_graphics_pipeline(
        &mut self,
        handle: GraphicsPipeline,
        descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        rhi_require(
            matches!(descriptor.sample_count, 1 | 4),
            "WebGPU pipeline sample count must be 1 or 4",
        )?;
        let targets = descriptor
            .color_attachments
            .iter()
            .map(color_target)
            .collect::<RhiResult<Vec<_>>>()?;
        let mut state = json!({
            "label": descriptor.label.unwrap_or_default(),
            "primitive": primitive(&descriptor)?, "multisample": { "count": descriptor.sample_count },
        });
        if let Some(format) = descriptor.depth_format.or(descriptor.stencil_format) {
            let depth = descriptor.depth_stencil.unwrap_or(DepthStencilState {
                depth_write_enabled: false,
                depth_compare: CompareOp::Always,
            });
            state["depthStencil"] = json!({ "format": texture_format(format)?, "depthWriteEnabled": depth.depth_write_enabled, "depthCompare": compare(depth.depth_compare) });
        }
        self.host.call("graphics", json!({
            "id": handle.id(), "layout": descriptor.layout.id(), "state": state,
            "vertex": descriptor.vertex.id(), "fragment": descriptor.fragment.map(ShaderModule::id),
            "targets": targets, "buffers": vertex_layout(descriptor.vertex_layout.as_ref()),
        }), &[])
    }

    fn create_compute_pipeline(
        &mut self,
        handle: ComputePipeline,
        descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        self.host.call("compute", json!({ "id": handle.id(), "layout": descriptor.layout.id(), "shader": descriptor.shader.id(), "label": descriptor.label.unwrap_or_default() }), &[])
    }

    fn register_binding_layout(
        &mut self,
        handle: BindingLayout,
        descriptor: BindingLayoutDescriptor,
    ) -> RhiResult<()> {
        let entries = descriptor
            .entries
            .iter()
            .map(binding_layout_entry)
            .collect::<RhiResult<Vec<_>>>()?;
        self.host.call(
            "bindingLayout",
            json!({ "id": handle.id(), "entries": entries }),
            &[],
        )
    }

    fn register_pipeline_layout(
        &mut self,
        handle: PipelineLayout,
        descriptor: PipelineLayoutDescriptor,
    ) -> RhiResult<()> {
        let sets: Vec<_> = descriptor.set_layouts.iter().map(|set| set.id()).collect();
        rhi_require(sets.len() + usize::from(!descriptor.push_constants.is_empty()) <= 4, "WebGPU supports four bind groups including the constants group")?;
        let constants: Vec<_> = descriptor.push_constants.iter().map(|range| json!({
            "slot": range.slot, "visibility": visibility(ShaderVisibility::from_stage(range.stage)), "size": range.byte_count,
        })).collect();
        self.host.call(
            "pipelineLayout",
            json!({ "id": handle.id(), "sets": sets, "constants": constants }),
            &[],
        )
    }

    fn register_binding_set(
        &mut self,
        handle: BindingSet,
        layout: BindingLayout,
        declarations: Vec<BindingLayoutEntry>,
        entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()> {
        let values = entries
            .iter()
            .map(|entry| {
                let resource = match entry.resource {
                    BindingResource::UniformBuffer {
                        buffer,
                        offset,
                        size,
                    } => json!({ "buffer": buffer.id(), "offset": offset, "size": size }),
                    BindingResource::StorageBuffer { buffer, offset } => {
                        json!({ "buffer": buffer.id(), "offset": offset })
                    }
                    BindingResource::Sampler(sampler) => json!({ "sampler": sampler.id() }),
                    BindingResource::Texture(texture)
                    | BindingResource::StorageTexture(texture) => {
                        let descriptor = self.textures.get(&texture.id()).ok_or_else(|| {
                            RhiError::invalid_argument("unknown WebGPU binding texture")
                        })?;
                        rhi_require(
                            descriptor.dimension == TextureDimension::Texture2D
                                && descriptor.sample_count == 1,
                            "WebGPU binding layout uses single-sample 2D textures",
                        )?;
                        if matches!(entry.resource, BindingResource::StorageTexture(_)) {
                            rhi_require(
                                descriptor.format == TextureFormat::Rgba8Unorm,
                                "WebGPU storage binding layout uses RGBA8Unorm",
                            )?;
                        }
                        json!({ "texture": texture.id() })
                    }
                    BindingResource::AccelerationStructure(_) => {
                        return Err(RhiError::unsupported_feature(
                            "WebGPU acceleration structures",
                        ));
                    }
                };
                Ok(json!({ "binding": entry.slot, "resource": resource }))
            })
            .collect::<RhiResult<Vec<_>>>()?;
        debug_assert_eq!(declarations.len(), entries.len());
        self.host.call(
            "bindingSet",
            json!({ "id": handle.id(), "layout": layout.id(), "entries": values }),
            &[],
        )
    }

    fn unregister_binding_set(&mut self, handle: BindingSet) {
        self.destroy("bindingSet", handle.id());
    }

    fn upload_buffer_data(&mut self, buffer: Buffer, offset: usize, data: &[u8]) -> RhiResult<()> {
        let descriptor = self
            .buffers
            .get(&buffer.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown WebGPU buffer"))?;
        rhi_byte_range(offset, data.len(), descriptor.size)?;
        rhi_require(
            offset.is_multiple_of(4) && data.len().is_multiple_of(4),
            "WebGPU buffer uploads require four-byte alignment",
        )?;
        self.host.call(
            "writeBuffer",
            json!({ "id": buffer.id(), "offset": offset }),
            data,
        )
    }

    fn upload_texture_data(
        &mut self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()> {
        let descriptor = self
            .textures
            .get(&texture.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown WebGPU texture"))?;
        let extent = [
            usize::try_from(region.width).unwrap_or(0),
            usize::try_from(region.height).unwrap_or(0),
        ];
        rhi_require(
            extent.iter().all(|&axis| axis > 0) && region.origin.iter().all(|&axis| axis >= 0),
            "invalid WebGPU texture upload extent",
        )?;
        let tight = extent[0]
            .checked_mul(descriptor.format.bytes_per_texel())
            .ok_or(RhiError::OutOfMemory)?;
        let required = bytes_per_row
            .checked_mul(extent[1] - 1)
            .and_then(|n| n.checked_add(tight))
            .ok_or(RhiError::OutOfMemory)?;
        rhi_require(
            bytes_per_row >= tight && required <= data.len(),
            "WebGPU texture upload data is shorter than its extent",
        )?;
        self.host.call("writeTexture", json!({
            "id": texture.id(), "extent": extent, "bytesPerRow": bytes_per_row, "mipLevel": subresource.mip_level,
            "origin": [region.origin[0], region.origin[1], subresource.layer as i64],
        }), &data[..required])
    }

    fn read_texture_data(
        &mut self,
        _: Texture,
        _: usize,
        _: usize,
        _: usize,
        _: TextureSubresource,
        _: &mut [u8],
    ) -> RhiResult<()> {
        Err(RhiError::unsupported_feature(
            "synchronous WebGPU texture readback",
        ))
    }

    fn make_frame_uploader(&mut self, slot: usize) -> RhiResult<Box<dyn FrameUploader>> {
        Ok(Box::new(Uploader {
            host: self.host.clone(),
            ids: self.next_internal.clone(),
            slot: self.uploads[slot].clone(),
        }))
    }

    fn submit(
        &mut self,
        submit: &PlannedSubmit,
        _profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        self.submit_impl(submit, false, Box::new(move |_| completion()))
    }

    fn submit_timed(
        &mut self,
        submit: &PlannedSubmit,
        _profile: Option<&mut SubmissionCpuProfile>,
        completion: TimedCompletion,
    ) -> RhiResult<()> {
        self.submit_impl(submit, true, completion)
    }

    fn destroy_buffer(&mut self, handle: Buffer) {
        self.buffers.remove(&handle.id());
        self.destroy("buffer", handle.id());
    }
    fn destroy_texture(&mut self, handle: Texture) {
        self.textures.remove(&handle.id());
        self.destroy("texture", handle.id());
    }
    fn destroy_sampler(&mut self, handle: Sampler) {
        self.destroy("sampler", handle.id());
    }
    fn destroy_shader_module(&mut self, handle: ShaderModule) {
        self.destroy("shader", handle.id());
    }
    fn destroy_graphics_pipeline(&mut self, handle: GraphicsPipeline) {
        self.destroy("graphics", handle.id());
    }
    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline) {
        self.destroy("compute", handle.id());
    }

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        if !self.host.idle() {
            return Err(RhiError::GpuBusy);
        }
        self.host.call("check", Value::Null, &[])
    }
}

fn internal_id(counter: &std::sync::atomic::AtomicU32) -> u32 {
    counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed)
}

fn visibility(stages: ShaderVisibility) -> u32 {
    u32::from(stages.contains(ShaderVisibility::VERTEX))
        | u32::from(stages.contains(ShaderVisibility::FRAGMENT)) << 1
        | u32::from(stages.contains(ShaderVisibility::COMPUTE)) << 2
}

fn binding_layout_entry(entry: &BindingLayoutEntry) -> RhiResult<Value> {
    rhi_require(
        entry.array_size == 1,
        "WebGPU core does not support descriptor binding arrays",
    )?;
    let mut value = json!({ "binding": entry.slot, "visibility": visibility(entry.visibility) });
    match entry.binding_type {
        BindingType::UniformBuffer => {
            value["buffer"] =
                json!({ "type": "uniform", "hasDynamicOffset": entry.buffer.has_dynamic_offset })
        }
        BindingType::StorageBuffer => {
            value["buffer"] = json!({ "type": if entry.buffer.read_only { "read-only-storage" } else { "storage" }, "hasDynamicOffset": entry.buffer.has_dynamic_offset })
        }
        BindingType::Sampler => value["sampler"] = json!({ "type": "filtering" }),
        BindingType::Texture => {
            value["texture"] = json!({ "sampleType": "float", "viewDimension": "2d" })
        }
        BindingType::StorageTexture => {
            value["storageTexture"] =
                json!({ "access": "write-only", "format": "rgba8unorm", "viewDimension": "2d" })
        }
        BindingType::AccelerationStructure => {
            return Err(RhiError::unsupported_feature(
                "WebGPU acceleration structures",
            ));
        }
    }
    Ok(value)
}

fn make_buffer(host: &Host, handle: Buffer, descriptor: BufferDescriptor) -> RhiResult<()> {
    rhi_require(descriptor.size > 0, "WebGPU buffer requires positive size")?;
    host.call("buffer", json!({ "id": handle.id(), "descriptor": {
        "size": descriptor.size, "usage": buffer_usage(descriptor.usage), "label": descriptor.label.unwrap_or_default(),
    }}), &[])
}

#[derive(Default)]
struct UploadSlot {
    chunks: Vec<(Buffer, usize, usize)>,
}

struct Uploader {
    host: Host,
    ids: Arc<std::sync::atomic::AtomicU32>,
    slot: Arc<Mutex<UploadSlot>>,
}

impl FrameUploader for Uploader {
    fn reset(&mut self) {
        for (_, _, used) in &mut self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .chunks
        {
            *used = 0;
        }
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadLocation> {
        rhi_require(
            alignment.is_power_of_two(),
            "invalid WebGPU upload alignment",
        )?;
        let alignment = alignment.max(4);
        let length = bytes.len().next_multiple_of(4);
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let chunk = slot.chunks.iter().position(|&(_, size, used)| {
            used.next_multiple_of(alignment)
                .checked_add(length)
                .is_some_and(|end| end <= size)
        });
        let index = if let Some(index) = chunk {
            index
        } else {
            let size = length
                .checked_next_power_of_two()
                .ok_or(RhiError::OutOfMemory)?
                .max(65536);
            let buffer = Buffer::new(internal_id(&self.ids));
            make_buffer(
                &self.host,
                buffer,
                BufferDescriptor::new(
                    size,
                    BufferUsage::TRANSFER_SOURCE
                        | BufferUsage::TRANSFER_DESTINATION
                        | BufferUsage::VERTEX
                        | BufferUsage::INDEX
                        | BufferUsage::INDIRECT
                        | BufferUsage::UNIFORM
                        | BufferUsage::STORAGE_READ,
                ),
            )?;
            slot.chunks.push((buffer, size, 0));
            slot.chunks.len() - 1
        };
        let (buffer, _, used) = &mut slot.chunks[index];
        let offset = used.next_multiple_of(alignment);
        let padded;
        let bytes = if bytes.len() == length {
            bytes
        } else {
            let mut copy = bytes.to_vec();
            copy.resize(length, 0);
            padded = copy;
            &padded
        };
        if !bytes.is_empty() {
            self.host.call(
                "writeBuffer",
                json!({ "id": buffer.id(), "offset": offset }),
                bytes,
            )?;
        }
        *used = offset + length;
        Ok(UploadLocation::new(*buffer, offset))
    }
}
